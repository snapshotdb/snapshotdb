//! snapshotdb: copy-on-write branches of any local database, synced from production.
//!
//! A branch is a directory `$SNAPSHOTDB_HOME/<name>/` holding `engine`, optional `parent`,
//! optional `source` (a production URL this root replicates from), per-root settings
//! (`default_db`, `branch_sql`, `lock`), `data/` (what the engine sees; this is what gets
//! cloned) and `run/` (ports, pids, sockets, logs, tailer state; never cloned).
//!
//! Every server branch has a stable public port owned by a small proxy (proxy.rs) and an
//! engine port behind it. Idle engines are suspended and resumed on the next connection.
//! The client submits authenticated jobs; only the deployed server clones database files.
mod preflight;
mod hosted;
mod proxy;
mod remote;
mod jobs;
mod pool;
mod sqlite;
mod sandbox;
mod login;

use std::{
    collections::HashMap,
    env, fs,
    net::{TcpListener, TcpStream},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{self, Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const USAGE: &str = "usage:
  snapshotdb serve --bind <address:port> --public-host <hostname> [--db-bind <ip>]   deploy the database server
  snapshotdb clone <name> <connection-string>                  create a replica on the deployed server
  snapshotdb job <id>                                         wait for a previously submitted server job
  snapshotdb login [--web <url>]                              sign in with GitHub in the browser
  snapshotdb preflight <postgres|mysql|mongodb> <url> [--schemas a,b] [--format json]   check a source; creates nothing (exit 2 on failure)
  snapshotdb import <postgres|mysql|sqlite|mongodb> <name> <datadir|file|--new>
  snapshotdb sync   <postgres|mysql|mongodb> <name> <url> [--schemas a,b] [--fix-replica-identity]   root kept in sync with production
  snapshotdb create <name> --from <parent> [--print-url] [--format json]
  snapshotdb prepare <snapshot> --from <parent> --count <1-32>   freeze a snapshot and fill its ready branch pool
  snapshotdb info   [name] [--print-url] [--format json]      details of a branch (default: current)
  snapshotdb url    [name]
  snapshotdb switch <name>                                    make a branch current
  snapshotdb list   [--format json]
  snapshotdb status <name> [--format json]                    replication state of a synced root
  snapshotdb repair <name>                                    resume a paused replica (skip a poisoned transaction, or reconcile schema)
  snapshotdb reconcile <name>                                 add columns/tables the source gained (for sources without the event trigger)
  snapshotdb reset  <name>                                    re-clone from parent
  snapshotdb settings <root> [set <key> <value> [--hook name] | remove <key> [--hook name]]
                                                             keys: default_db, branch_sql (@file or SQL; several via --hook, run in name order), source
  snapshotdb lock|unlock <name>                               protect a branch from rm
  snapshotdb start|stop|rm <name>
client env: SNAPSHOTDB_SERVER (HTTPS API URL)   SNAPSHOTDB_TOKEN (server access token)
server env: SNAPSHOTDB_HOME (server storage)   SNAPSHOTDB_TOKEN   SNAPSHOTDB_IDLE_MINUTES
All database commands execute on the deployed server. --detach returns a server job ID.";

pub type R<T> = Result<T, String>;

struct Args {
    pos: Vec<String>,
    flags: HashMap<String, String>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Args {
        let (mut pos, mut flags) = (vec![], HashMap::new());
        let mut it = raw.into_iter().peekable();
        while let Some(a) = it.next() {
            match a.strip_prefix("--") {
                Some("new") | None => pos.push(a),
                Some(k) => {
                    if let Some((k, v)) = k.split_once('=') {
                        flags.insert(k.to_string(), v.to_string());
                    } else if matches!(k, "print-url" | "fix-replica-identity") || it.peek().is_none_or(|n| n.starts_with("--")) {
                        flags.insert(k.to_string(), "true".into());
                    } else {
                        flags.insert(k.to_string(), it.next().unwrap_or_default());
                    }
                }
            }
        }
        Args { pos, flags }
    }

    fn flag(&self, k: &str) -> bool {
        self.flags.contains_key(k)
    }

    fn json(&self) -> bool {
        self.flags.get("format").map(|f| f == "json").unwrap_or(false)
    }

    fn schemas(&self) -> String {
        self.flags.get("schemas").cloned().unwrap_or_else(|| "public".into())
    }
}

fn main() {
    if env::args().nth(1).as_deref() == Some("login") {
        let args: Vec<String> = env::args().skip(2).collect();
        if let Err(e) = login::run(&args) {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }
    let raw = match remote::route(env::args().skip(1).collect()) {
        Ok(Some(raw)) => raw,
        Ok(None) => return,
        Err(e) => { eprintln!("error: {e}"); process::exit(1); }
    };
    // Also serialize workers across a control-server restart with a surviving child.
    let _worker_lock = if env::var("SNAPSHOTDB_INTERNAL").as_deref() == Ok("1")
        && raw.first().map(String::as_str) != Some("_proxy") {
        match remote::worker_lock(false) {
            Ok(lock) => Some(lock),
            Err(e) => { eprintln!("error: {e}"); process::exit(1); }
        }
    } else { None };
    let a = Args::parse(raw);
    if a.pos.first().map(String::as_str) != Some("_proxy") {
        if let Err(e) = hosted::validate(&a) { eprintln!("{e}"); process::exit(1); }
    }
    if a.flag("version") {
        println!("snapshotdb {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if a.flag("help") || a.pos.is_empty() {
        println!("{USAGE}");
        process::exit(if a.flag("help") { 0 } else { 1 });
    }
    if hosted::enabled() && a.pos.first().map(String::as_str)==Some("rm") { env::set_var("SNAPSHOTDB_CLEANUP", "1"); }
    let p: Vec<&str> = a.pos.iter().map(String::as_str).collect();
    let json = a.json();
    let current = |name: Option<&&str>| -> R<Branch> {
        match name {
            Some(n) => Branch::load(n),
            None => Branch::load(&fs::read_to_string(home().join(".current")).map_err(|_| "no current branch; pass a name or run: snapshotdb switch <name>")?),
        }
    };
    let result: R<()> = match p.as_slice() {
        ["preflight", engine, url] => {
            let report = Engine::parse(engine).and_then(|e| preflight::run(e, url, &a.schemas()));
            match report {
                Ok(r) => {
                    print!("{}", r.render(json));
                    process::exit(if r.passed() { 0 } else { 2 });
                }
                Err(e) => Err(e),
            }
        }
        ["import", engine, name, source] => import(engine, name, source).and_then(|b| emit(&b, &a, None)),
        ["sync", engine, name, url] => sync(engine, name, url, &a.schemas(), a.flag("fix-replica-identity")).and_then(|b| emit(&b, &a, None)),
        ["create", name] => {
            let started = Instant::now();
            match a.flags.get("from") {
                Some(parent) => create(name, parent).and_then(|b| emit(&b, &a, Some(started))),
                None => Err(format!("create needs --from <parent>\n{USAGE}")),
            }
        }
        ["prepare", name] => pool::prepare(name, &a).map(|v| println!("{v}")),
        ["info"] | ["info", _] => current(p.get(1)).and_then(|b| emit(&b, &a, None)),
        ["url"] | ["url", _] => current(p.get(1)).and_then(|b| b.url()).map(|u| println!("{u}")),
        ["switch", name] => Branch::load(name).and_then(|b| b.set_current()).map(|_| println!("switched to {name}")),
        ["list"] => list(json),
        ["status", name] => Branch::load(name).and_then(|b| b.status()).map(|s| print!("{}", render_status(&s, json))),
        ["repair", name] => Branch::load(name).and_then(|b| b.repair()).map(|m| println!("{m}")),
        ["reconcile", name] => Branch::load(name).and_then(|b| b.reconcile()).map(|m| println!("{m}")),
        ["reset", name] => reset(name).and_then(|b| emit(&b, &a, None)),
        ["settings", root] => Branch::load(root).and_then(|b| b.settings_list()).map(|s| print!("{s}")),
        ["settings", root, "set", key, value] => Branch::load(root).and_then(|b| b.set_setting(key, value, a.flags.get("hook"))),
        ["settings", root, "remove", key] => Branch::load(root).and_then(|b| b.remove_setting(key, a.flags.get("hook"))),
        ["lock", name] => Branch::load(name).and_then(|b| io(fs::write(b.dir.join("lock"), ""))),
        ["unlock", name] => Branch::load(name).and_then(|b| io(fs::remove_file(b.dir.join("lock")).or(Ok(())))),
        ["start", name] => Branch::load(name).and_then(|b| b.start().and_then(|_| if hosted::enabled() && b.parent().is_none() && b.engine != Engine::Sqlite { Ok(format!("Source {} started", b.name)) } else { b.url() })).map(|u| println!("{u}")),
        ["stop", name] => Branch::load(name).and_then(|b| b.stop()),
        ["rm", name] => Branch::load(name).and_then(|b| b.rm()),
        ["_proxy", name] => proxy::serve(name),
        ["_hosted_up"] => hosted::recover().and_then(|_| up()),
        ["_hosted_down"] => down().and_then(|_| hosted::usage(&home()).map(|_| ())),
        _ => Err(USAGE.to_string()),
    };
    if let Err(e) = result {
        // 2 = the command itself was wrong (also preflight failure); 1 = something failed while running.
        let code = if e.starts_with("usage:") || e.contains(USAGE) { 2 } else { 1 };
        if json {
            println!("{{\"error\":{}}}", js(&e));
        } else {
            eprintln!("error: {e}");
        }
        process::exit(code);
    }
}

/// Print a branch the way the caller asked: URL only, JSON, or URL plus a timing line.
fn emit(b: &Branch, a: &Args, started: Option<Instant>) -> R<()> {
    if a.json() {
        println!("{}", b.info_json()?);
    } else if hosted::enabled() && b.parent().is_none() && b.engine != Engine::Sqlite {
        println!("Source {} ready; create a branch to connect", b.name);
    } else {
        println!("{}", b.url()?);
        if let (Some(t), false) = (started, a.flag("print-url")) {
            eprintln!("branch {} ready in {:.2}s", b.name, t.elapsed().as_secs_f64());
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
pub enum Engine {
    Postgres,
    Mysql,
    Sqlite,
    Mongodb,
}

impl Engine {
    pub fn parse(s: &str) -> R<Engine> {
        Ok(match s {
            "postgres" => Engine::Postgres,
            "mysql" => Engine::Mysql,
            "sqlite" => Engine::Sqlite,
            "mongodb" => Engine::Mongodb,
            _ => return Err(format!("unknown engine {s:?}\n{USAGE}")),
        })
    }

    fn name(self) -> &'static str {
        match self {
            Engine::Postgres => "postgres",
            Engine::Mysql => "mysql",
            Engine::Sqlite => "sqlite",
            Engine::Mongodb => "mongodb",
        }
    }

    /// Refuse to clone a data directory some server is still writing to:
    /// per-file clones are only a consistent snapshot when nothing moves.
    fn check_stopped(self, data: &Path) -> R<()> {
        let busy = match self {
            Engine::Postgres => live_pid(&data.join("postmaster.pid")).is_some(),
            Engine::Mysql => ok(Command::new("pgrep").arg("-f").arg(format!("--datadir={}", data.display()))),
            Engine::Mongodb => fs::metadata(data.join("mongod.lock")).map(|m| m.len() > 0).unwrap_or(false),
            Engine::Sqlite => false, // cloned with its -wal; readers recover committed frames on open
        };
        if busy {
            return Err(format!("a {} server is using {}; stop it first so the clone is consistent", self.name(), data.display()));
        }
        Ok(())
    }
}

pub struct Branch {
    pub name: String,
    pub dir: PathBuf,
    pub engine: Engine,
}

pub fn home() -> PathBuf {
    env::var_os("SNAPSHOTDB_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").expect("HOME is set")).join(".snapshotdb"))
}

/// Exclusive marker on a branch while its engine is being started or its data cloned, so
/// the proxy cannot wake the engine in the middle of a clone. Removed on drop.
struct Hold(PathBuf);

impl Drop for Hold {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl Branch {
    fn new(name: &str, engine: Engine) -> R<Branch> {
        remote::validate_name(name)?;
        let mut b = Branch { name: name.into(), dir: home().join(name), engine };
        if b.dir.exists() {
            return Err(format!("branch {name} already exists"));
        }
        io(fs::create_dir_all(b.run()))?;
        // The sandbox mounts canonical paths. New branches must use the same
        // physical path as loaded branches, including when the server home is
        // a symlink to a separately mounted BYOC data volume.
        b.dir = io(fs::canonicalize(&b.dir))?;
        io(fs::write(b.dir.join("engine"), engine.name()))?;
        Ok(b)
    }

    pub fn load(name: &str) -> R<Branch> {
        remote::validate_name(name)?;
        let dir = fs::canonicalize(home().join(name)).map_err(|_| format!("no branch named {name}"))?;
        let engine = fs::read_to_string(dir.join("engine")).map_err(|_| format!("no branch named {name}"))?;
        Ok(Branch { name: name.into(), dir, engine: Engine::parse(engine.trim())? })
    }

    fn data(&self) -> PathBuf {
        self.dir.join("data")
    }

    fn run(&self) -> PathBuf {
        self.dir.join("run")
    }

    fn parent(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("parent")).ok()
    }

    /// Production URL this root replicates from, if it was made with `sync`.
    pub fn source(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("source")).ok()
    }

    /// The root of this branch's ancestry (itself if it has no parent).
    fn root(&self) -> Branch {
        let mut cur = Branch { name: self.name.clone(), dir: self.dir.clone(), engine: self.engine };
        while let Some(p) = cur.parent().and_then(|p| Branch::load(&p).ok()) {
            if pool::is_snapshot(&cur) { break; }
            cur = p;
        }
        cur
    }

    fn synced_ancestry(&self) -> bool {
        self.root().source().is_some()
    }

    /// Replication object name (publication, subscription, slot, DDL schema) for this root.
    fn subname(&self) -> String {
        format!("snapshotdb_{}", self.name.replace(['-', '.'], "_"))
    }

    fn setting(&self, key: &str) -> Option<String> {
        fs::read_to_string(self.root().dir.join(key)).ok().filter(|s| !s.is_empty())
    }

    fn database(&self) -> String {
        self.setting("default_db").unwrap_or_else(|| match self.engine {
            Engine::Postgres => "postgres".into(),
            _ => String::new(),
        })
    }

    // --- credentials -----------------------------------------------------------------

    /// This branch's own admin password, once it has one.
    fn password(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("password")).ok().filter(|s| !s.is_empty())
    }

    /// The admin password the data currently accepts: this branch's own, or the parent's
    /// for a fresh clone that has not rotated yet.
    fn inherited_password(&self) -> Option<String> {
        self.password().or_else(|| self.parent().and_then(|p| Branch::load(&p).ok()).and_then(|p| p.inherited_password()))
    }

    /// snapshotdb manages credentials for roots it created (`--new`, `sync`) and their clones.
    /// Imported data directories keep whatever auth they came with.
    fn managed(&self) -> bool {
        self.dir.join("new").exists()
            || self.password().is_some()
            || self.parent().and_then(|p| Branch::load(&p).ok()).map(|p| p.managed()).unwrap_or(false)
    }

    fn admin_user(&self) -> String {
        match self.engine {
            Engine::Postgres => env::var("USER").unwrap_or_else(|_| "postgres".into()),
            Engine::Mysql => "root".into(),
            Engine::Mongodb => "snapshotdb".into(),
            Engine::Sqlite => String::new(),
        }
    }

    /// Give this branch its own admin password: a fresh clone still accepts its parent's,
    /// a fresh MySQL or MongoDB root none at all.
    fn ensure_credentials(&self) -> R<()> {
        let run = self.run();
        if (self.engine != Engine::Sqlite && !self.managed()) || run.join("creds").exists() {
            return Ok(());
        }
        if self.password().is_none() {
            let new = random_password()?;
            let current = self.inherited_password();
            let user = self.admin_user();
            match self.engine {
                Engine::Postgres => psql(&self.socket_url("postgres")?, &format!("ALTER ROLE \"{user}\" PASSWORD '{new}'"))?,
                Engine::Mysql => out(self.mysql_with(current.as_deref()).arg("-e").arg(format!("ALTER USER 'root'@'localhost' IDENTIFIED BY '{new}'")))?,
                Engine::Mongodb => {
                    // No users yet: the localhost exception lets us create the first one.
                    let script = match current {
                        None => format!("db.getSiblingDB('admin').createUser({{ user: '{user}', pwd: '{new}', roles: ['root'] }})"),
                        Some(_) => format!("db.getSiblingDB('admin').changeUserPassword('{user}', '{new}')"),
                    };
                    mongosh(&self.engine_url()?, &script)?
                }
                Engine::Sqlite => String::new(),
            };
            write_secret(&self.dir.join("password"), &new)?;
        }
        io(fs::write(run.join("creds"), ""))
    }

    fn set_current(&self) -> R<()> {
        io(fs::write(home().join(".current"), &self.name))
    }

    /// Public branch credentials never grant server administration or filesystem
    /// access. Maintenance keeps a separate password outside the engine sandbox.
    fn ensure_agent_credentials(&self) -> R<()> {
        if self.parent().is_none() || self.engine == Engine::Sqlite { return Ok(()); }
        if !self.managed() { return Err("agent branches require a managed root; sync the source instead of importing an unmanaged directory".into()); }
        if self.dir.join("agent-password").exists() { return Ok(()); }
        let password = random_password()?;
        let admin_password = random_password()?;
        match self.engine {
            Engine::Postgres => {
                let admin = self.admin_user().replace('"', "\"\"");
                self.sql(&format!(
                    "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='snapshotdb_agent') THEN CREATE ROLE snapshotdb_agent; END IF; END $$; \
                     ALTER ROLE snapshotdb_agent LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD '{password}'; \
                     ALTER ROLE snapshotdb_agent RESET ALL; \
                     DO $$ DECLARE r record; BEGIN FOR r IN SELECT roleid::regrole AS role FROM pg_auth_members WHERE member='snapshotdb_agent'::regrole LOOP \
                     EXECUTE format('REVOKE %s FROM snapshotdb_agent', r.role); END LOOP; \
                     IF EXISTS (SELECT FROM pg_auth_members WHERE member='snapshotdb_agent'::regrole) THEN RAISE EXCEPTION 'agent role still has inherited memberships'; END IF; \
                     FOR r IN SELECT d.datname FROM pg_db_role_setting s JOIN pg_database d ON d.oid=s.setdatabase WHERE s.setrole='snapshotdb_agent'::regrole LOOP \
                     EXECUTE format('ALTER ROLE snapshotdb_agent IN DATABASE %I RESET ALL',r.datname); END LOOP; END $$; \
                     DO $$ DECLARE r record; BEGIN \
                     EXECUTE format('ALTER DATABASE %I OWNER TO snapshotdb_agent', current_database()); \
                     FOR r IN SELECT nspname FROM pg_namespace WHERE nspname !~ '^pg_' AND nspname <> 'information_schema' LOOP \
                       EXECUTE format('ALTER SCHEMA %I OWNER TO snapshotdb_agent', r.nspname); END LOOP; \
                     FOR r IN SELECT c.oid::regclass AS obj, c.relkind FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace \
                       WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' AND c.relkind IN ('r','p','v','m','S','f') AND NOT (c.relkind='S' AND EXISTS (SELECT FROM pg_depend d WHERE d.objid=c.oid AND d.deptype IN ('a','i'))) LOOP \
                       EXECUTE format('ALTER %s %s OWNER TO snapshotdb_agent', CASE r.relkind WHEN 'v' THEN 'VIEW' WHEN 'm' THEN 'MATERIALIZED VIEW' WHEN 'S' THEN 'SEQUENCE' WHEN 'f' THEN 'FOREIGN TABLE' ELSE 'TABLE' END, r.obj); END LOOP; \
                     FOR r IN SELECT p.oid::regprocedure AS obj,p.prokind FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace \
                       WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' LOOP \
                       EXECUTE format('ALTER %s %s OWNER TO snapshotdb_agent', CASE r.prokind WHEN 'p' THEN 'PROCEDURE' WHEN 'a' THEN 'AGGREGATE' ELSE 'FUNCTION' END,r.obj); END LOOP; \
                     FOR r IN SELECT t.oid::regtype AS obj FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace \
                       WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' AND t.typtype IN ('e','d') LOOP \
                       EXECUTE format('ALTER TYPE %s OWNER TO snapshotdb_agent',r.obj); END LOOP; END $$; \
                     ALTER ROLE \"{admin}\" PASSWORD '{admin_password}';"
                ))?;
            }
            Engine::Mysql => {
                self.sql(&format!("DROP USER IF EXISTS 'snapshotdb_agent'@'localhost'; CREATE USER 'snapshotdb_agent'@'localhost' IDENTIFIED BY '{password}';"))?;
                let dbs = self.sql("SHOW DATABASES")?;
                for db in dbs.lines().filter(|d| !matches!(*d, "Database" | "mysql" | "sys" | "information_schema" | "performance_schema")) {
                    self.sql(&format!("GRANT ALL PRIVILEGES ON `{}`.* TO 'snapshotdb_agent'@'localhost'", db.replace('`', "``")))?;
                }
                self.sql(&format!("ALTER USER 'root'@'localhost' IDENTIFIED BY '{admin_password}'"))?;
            }
            Engine::Mongodb => {
                self.sql(&format!("const a=db.getSiblingDB('admin'); if(a.getUser('snapshotdb_agent')) a.dropUser('snapshotdb_agent'); a.createUser({{user:'snapshotdb_agent',pwd:'{password}',roles:['readWriteAnyDatabase','dbAdminAnyDatabase']}}); a.changeUserPassword('snapshotdb','{admin_password}');"))?;
            }
            Engine::Sqlite => unreachable!(),
        }
        write_secret(&self.dir.join("password"), &admin_password)?;
        write_secret(&self.dir.join("agent-password"), &password)
    }

    fn is_current(&self) -> bool {
        fs::read_to_string(home().join(".current")).map(|c| c == self.name).unwrap_or(false)
    }

    // --- ports and processes -------------------------------------------------------

    fn pid(&self) -> Option<u32> {
        match self.engine {
            Engine::Postgres => live_pid(&self.data().join("postmaster.pid")),
            Engine::Sqlite => None,
            _ => live_pid(&self.run().join("pid")),
        }
    }

    pub fn running(&self) -> bool {
        self.pid().is_some()
    }

    /// Stable public port, owned by the proxy.
    pub fn port(&self) -> Option<u16> {
        fs::read_to_string(self.run().join("port")).ok()?.trim().parse().ok()
    }

    /// The engine's own port behind the proxy; may change across restarts.
    pub fn eport(&self) -> Option<u16> {
        fs::read_to_string(self.run().join("eport")).ok()?.trim().parse().ok()
    }

    /// The proxy is alive and actually holding the public port (a pid alone can be stale after a reboot).
    fn proxy_alive(&self) -> bool {
        live_pid(&self.run().join("proxypid")).is_some() && self.port().map(|p| !port_free(p)).unwrap_or(false)
    }

    fn status_word(&self) -> &'static str {
        if pool::is_snapshot(self) { return "snapshot"; }
        match (self.engine, self.running(), self.proxy_alive(), self.source().is_some()) {
            (Engine::Sqlite, _, true, _) => "running",
            (Engine::Sqlite, ..) => "stopped",
            (_, true, _, true) => "syncing",
            (_, true, _, false) => "running",
            (_, false, true, _) => "suspended",
            (_, false, false, _) => "stopped",
        }
    }

    /// Public URL through the proxy. Valid while the branch is suspended: connecting resumes it.
    pub fn url(&self) -> R<String> {
        if hosted::enabled() && self.parent().is_none() && self.engine != Engine::Sqlite { return Err("Create a branch to get a connection URL. Hosted source replicas do not expose admin credentials.".into()); }
        if pool::is_snapshot(self) { return Err("prepared snapshots are immutable; create a branch from this snapshot".into()); }
        if self.parent().is_some() && !self.run().join("ready-v1").exists() {
            return Err("branch initialization has not completed; start the branch before requesting its URL".into());
        }
        if self.engine == Engine::Sqlite {
            return sqlite::url(self);
        }
        let port = self.port().filter(|_| self.proxy_alive() || self.running());
        let port = port.ok_or_else(|| format!("{0} is stopped; run: snapshotdb start {0}", self.name))?;
        Ok(self.url_on(port, true))
    }

    /// URL straight to the engine, for snapshotdb's own maintenance commands (MongoDB, and
    /// mongorestore). Carries the password the data currently accepts.
    fn engine_url(&self) -> R<String> {
        let port = self.eport().filter(|_| self.running()).ok_or_else(|| format!("{} is not running", self.name))?;
        Ok(self.url_on(port, false))
    }

    /// Postgres over its Unix socket, which pg_hba trusts locally, so it works before and
    /// after the password is set.
    fn socket_url(&self, db: &str) -> R<String> {
        let port = self.eport().filter(|_| self.running()).ok_or_else(|| format!("{} is not running", self.name))?;
        Ok(format!("postgresql:///{db}?host={}&port={port}&user={}", self.run().display(), self.admin_user()))
    }

    fn url_on(&self, port: u16, public: bool) -> String {
        let host = if public { env::var("SNAPSHOTDB_PUBLIC_HOST").unwrap_or_else(|_| "127.0.0.1".into()) } else { "127.0.0.1".into() };
        let db = self.database();
        let agent = public && self.parent().is_some() && self.managed();
        let user = if agent { "snapshotdb_agent".into() } else { self.admin_user() };
        let pw = if agent { fs::read_to_string(self.dir.join("agent-password")).ok() }
            else if public { self.password() } else { self.inherited_password() };
        let cred = match &pw {
            Some(pw) => format!("{user}:{pw}@"),
            None => format!("{user}@"),
        };
        match self.engine {
            // ponytail: assumes the cluster's superuser is $USER (initdb's default); set default_db for the database.
            Engine::Postgres => format!("postgresql://{cred}{host}:{port}/{db}"),
            Engine::Mysql => format!("mysql://{cred}{host}:{port}/{db}"),
            Engine::Mongodb => {
                // directConnection: drivers must not discover the engine port behind the proxy.
                let mut q = vec![];
                if public {
                    q.push("directConnection=true");
                    // mongosh otherwise uses a 2s selection timeout, shorter than a cold
                    // resume on a small server. Allow the proxy time to start the engine.
                    q.push("serverSelectionTimeoutMS=30000");
                }
                if pw.is_some() {
                    q.push("authSource=admin");
                }
                format!("mongodb://{}{host}:{port}/{db}{}{}", if pw.is_some() { cred } else { String::new() }, if q.is_empty() { "" } else { "?" }, q.join("&"))
            }
            Engine::Sqlite => unreachable!(),
        }
    }

    /// The mysql client pointed at this branch's socket, authenticating with `pw`.
    fn mysql_with(&self, pw: Option<&str>) -> Command {
        let mut c = Command::new("mysql");
        c.args(["--no-defaults", "-u", "root", "-N", "-B", "--protocol=socket"])
            .arg(format!("--socket={}", self.run().join("sock").display()));
        if let Some(pw) = pw {
            c.env("MYSQL_PWD", pw);
        }
        c
    }

    fn mysql(&self) -> Command {
        let mut c = self.mysql_with(self.inherited_password().as_deref());
        if !self.database().is_empty() { c.args(["-D", &self.database()]); }
        c
    }

    /// Run SQL (or JavaScript for MongoDB) on this branch's own server; returns stdout.
    fn sql(&self, statement: &str) -> R<String> {
        match self.engine {
            Engine::Postgres => psql(&self.socket_url(&self.database())?, statement),
            Engine::Mysql => out(self.mysql().arg("-e").arg(statement)),
            Engine::Mongodb => mongosh(&self.engine_url()?, statement),
            Engine::Sqlite => Err("sqlite has no server; open the file directly".into()),
        }
    }

    // --- lifecycle -----------------------------------------------------------------

    /// Create an empty database in data/, cleanly shut down, with credentials snapshotdb manages.
    fn init(&self) -> R<()> {
        let data = self.data();
        io(fs::write(self.dir.join("new"), ""))?;
        match self.engine {
            Engine::Postgres => {
                // Password auth over TCP, trust on the Unix socket that snapshotdb itself uses.
                let pw = random_password()?;
                let pwfile = self.run().join("pwfile");
                write_secret(&pwfile, &pw)?;
                let result = sh(Command::new("initdb")
                    .arg("-D").arg(&data)
                    .args(["--auth-host=scram-sha-256", "--auth-local=trust", "-U", &self.admin_user(), "--pwfile"])
                    .arg(&pwfile));
                let _ = fs::remove_file(&pwfile);
                result.and_then(|_| write_secret(&self.dir.join("password"), &pw))
            }
            Engine::Mysql => sh(Command::new("mysqld")
                .args(["--no-defaults", "--initialize-insecure"])
                .arg(format!("--datadir={}", data.display()))
                .arg(format!("--log-error={}", self.run().join("init.log").display()))),
            // ponytail: a zero-byte file is a valid empty SQLite database.
            Engine::Sqlite => io(fs::create_dir(&data)).and_then(|_| io(fs::write(data.join("db.sqlite"), ""))),
            // WiredTiger lays down its files on first start.
            Engine::Mongodb => io(fs::create_dir(&data)).and_then(|_| self.start_engine()).and_then(|_| self.stop()),
        }
    }

    fn hold(&self) -> R<Hold> {
        let marker = self.run().join("starting");
        let t = Instant::now();
        loop {
            match fs::OpenOptions::new().write(true).create_new(true).open(&marker) {
                Ok(_) => return Ok(Hold(marker)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                // A branch whose run/ was never created: make it (not recursively, so a
                // deleted branch still errors) rather than spin on a marker that can't exist.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    match fs::create_dir(self.run()) {
                        Ok(()) => continue,
                        Err(d) if d.kind() == std::io::ErrorKind::AlreadyExists => continue,
                        Err(d) => return Err(format!("{}: {d}", self.name)),
                    }
                }
                Err(e) => return Err(format!("{}: {e}", self.name)),
            }
            let stale = marker.metadata().and_then(|m| m.modified()).map(|m| m.elapsed().unwrap_or_default() > Duration::from_secs(300)).unwrap_or(true);
            if stale {
                let _ = fs::remove_file(&marker);
                continue;
            }
            if t.elapsed() > Duration::from_secs(300) {
                return Err(format!("{} is busy starting or cloning; try again", self.name));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Start proxy and engine; the branch's public URL is usable afterwards.
    pub fn start(&self) -> R<()> {
        pool::writable(self)?;
        if self.engine == Engine::Sqlite {
            let _quota = hosted::start_lock()?;
            hosted::before_start(self)?;
            let _hold = self.hold()?;
            self.ensure_credentials()?;
            self.initialize_hooks()?;
            io(fs::write(self.run().join("ready-v1"), ""))?;
            return self.ensure_proxy();
        }
        self.ensure_proxy()?;
        self.start_engine()
    }

    fn ensure_proxy(&self) -> R<()> {
        if self.proxy_alive() {
            return Ok(());
        }
        let run = self.run();
        let port = match self.port() {
            Some(p) if proxy::port_free(p) => p,
            Some(p) => {
                let n = free_port();
                eprintln!("warning: port {p} is taken; {} moves to {n}", self.name);
                n
            }
            None => free_port(),
        };
        io(fs::write(run.join("port"), port.to_string()))?;
        let log = io(fs::OpenOptions::new().create(true).append(true).open(run.join("proxy.log")))?;
        let mut proxy_command = Command::new(io(env::current_exe())?);
        if self.engine == Engine::Sqlite { hosted::limit_process(&mut proxy_command, self)?; }
        let child = proxy_command.env_remove("SNAPSHOTDB_CLEANUP").args(["_proxy", &self.name])
            .stdin(Stdio::null())
            .stdout(Stdio::from(io(log.try_clone())?))
            .stderr(Stdio::from(log))
            .process_group(0)
            .spawn()
            .map_err(|e| format!("spawn proxy: {e}"))?;
        io(fs::write(run.join("proxypid"), child.id().to_string()))?;
        wait(|| !proxy::port_free(port), 10, "proxy to listen")
    }

    /// Start the engine behind the proxy (also what the proxy calls to resume).
    pub fn start_engine(&self) -> R<()> {
        let _quota = hosted::start_lock()?;
        hosted::before_start(self)?;
        pool::writable(self)?;
        if self.engine == Engine::Sqlite || (self.running() && self.run().join("ready-v1").exists()) {
            return Ok(());
        }
        let _hold = self.hold()?;
        if self.running() && self.run().join("ready-v1").exists() {
            return Ok(()); // someone else completed initialization while we waited
        }
        let (data, run) = (self.data(), self.run());
        let clone = self.parent().is_some();
        // Existing deployments must not label an old, unsandboxed process ready.
        if clone && self.running() && !run.join("sandbox-v1").exists() {
            self.stop()?;
        }
        if !self.running() {
        let port = free_port();
        io(fs::write(run.join("eport"), port.to_string()))?;
        let _ = fs::remove_file(run.join("ready-v1"));
        match self.engine {
            Engine::Postgres => {
                // wal_level=logical lets any branch serve as a replication source. Clones get no
                // apply workers, so an inherited subscription can never race the root for its slot.
                // %b tags each log line with the backend type, so status can tell a replication
                // worker's error from snapshotdb's own statements.
                // wal_receiver_timeout: replaying a long DDL on a big table must not look like a dead link.
                let mut opts = format!(
                    "-c listen_addresses=127.0.0.1 -c port={port} -c unix_socket_directories='{}' -c wal_level=logical -c log_line_prefix='%m [%p] %b: ' -c wal_receiver_timeout={REPLICATION_TIMEOUT_MS}",
                    run.display()
                );
                if clone {
                    opts += " -c max_logical_replication_workers=0";
                }
                // Without a locale in the environment (launchd, cron) macOS Postgres dies with
                // "postmaster became multithreaded during startup"; the cluster's own locale is unaffected.
                let locale = env::var("LC_ALL").or_else(|_| env::var("LANG")).unwrap_or_else(|_| "C".into());
                sh(sandbox::command(self, "pg_ctl")?.env("LC_ALL", locale).arg("-D").arg(&data).arg("-l").arg(run.join("log")).args(["-w", "-o", &opts, "start"]))?
            }
            Engine::Mongodb => {
                // Every mongod is a single-node replica set, so change streams and transactions
                // work and a synced root can be tailed. Spawned detached: 8.3 dropped --fork on macOS.
                if self.managed() && !self.dir.join("keyfile").exists() {
                    write_secret(&self.dir.join("keyfile"), &format!("{}{}", random_password()?, random_password()?))?;
                }
                let mut cmd = sandbox::command(self, "mongod")?;
                cmd.arg("--dbpath").arg(&data)
                    .args(["--port", &port.to_string(), "--bind_ip", "127.0.0.1", "--nounixsocket", "--logappend", "--replSet", "snapshotdb"])
                    .arg("--logpath").arg(run.join("log"))
                    .arg("--pidfilepath").arg(run.join("pid"));
                if hosted::enabled() { cmd.args(["--wiredTigerCacheSizeGB", "0.5"]); }
                if self.managed() {
                    // A keyFile turns authentication on; for a single-node set its content is arbitrary.
                    let keyfile = self.dir.join("keyfile");
                    if !keyfile.exists() {
                        write_secret(&keyfile, &format!("{}{}", random_password()?, random_password()?))?;
                    }
                    cmd.arg("--keyFile").arg(&keyfile);
                }
                let mut child = cmd
                    .stdin(Stdio::null()).stdout(Stdio::null())
                    .stderr(Stdio::from(io(fs::OpenOptions::new().create(true).append(true).open(run.join("log")))?))
                    .process_group(0)
                    .spawn()
                    .map_err(|e| format!("mongod: {e}"))?;
                wait_port(port, &mut child, &run.join("log"))?;
                mongosh(&self.url_on(port, false), &MONGO_ENSURE_PRIMARY.replace("PORT", &port.to_string()))?;
            }
            Engine::Mysql => {
                let mut cmd = sandbox::command(self, "mysqld")?;
                cmd.arg("--no-defaults")
                    .arg(format!("--datadir={}", data.display()))
                    .arg(format!("--port={port}"))
                    .arg("--bind-address=127.0.0.1")
                    .arg(format!("--socket={}", run.join("sock").display()))
                    .arg(format!("--pid-file={}", run.join("pid").display()))
                    .arg(format!("--log-error={}", run.join("log").display()))
                    // GTIDs so any branch can be a replication source or replica; the port doubles as server-id.
                    .arg(format!("--server-id={port}"))
                    .args(["--mysqlx=OFF", "--gtid-mode=ON", "--enforce-gtid-consistency=ON", "--relay-log=relay-bin"]);
                if clone {
                    cmd.args(["--skip-replica-start", "--secure-file-priv=NULL"]);
                }
                if self.source().is_some() {
                    cmd.args(["--replicate-ignore-db=mysql", "--replicate-ignore-db=sys"]);
                }
                let mut child = cmd
                    .stdin(Stdio::null()).stdout(Stdio::null())
                    .stderr(Stdio::from(io(fs::OpenOptions::new().create(true).append(true).open(run.join("log")))?))
                    .process_group(0)
                    .spawn()
                    .map_err(|e| format!("mysqld: {e}"))?;
                wait_port(port, &mut child, &run.join("log"))?
            }
            Engine::Sqlite => unreachable!(),
        }
        if clone { io(fs::write(run.join("sandbox-v1"), ""))?; }
        }
        self.ensure_credentials()?;
        if self.engine == Engine::Postgres && self.source().is_some() && !run.join("ddl-guard-v1").exists() && self.sql(&format!("SELECT to_regclass('{}.ddl') IS NOT NULL", self.subname()))? == "t" {
            self.sql(&ddl_replica_sql(&self.subname()))?;
            io(fs::write(run.join("ddl-guard-v1"), ""))?;
        }
        if clone && !run.join("detached").exists() {
            self.detach()?;
            io(fs::write(run.join("detached"), ""))?;
        }
        if self.engine == Engine::Mongodb && self.source().is_some() {
            self.spawn_tail()?;
        }
        self.initialize_hooks()?;
        self.ensure_agent_credentials()?;
        io(fs::write(run.join("ready-v1"), ""))?;
        Ok(())
    }

    fn initialize_hooks(&self) -> R<()> {
        if self.parent().is_none() || self.run().join("branch_sql.done-v1").exists() { return Ok(()); }
        if self.run().join("branch_sql.done").exists() && !self.hooks().is_empty() {
            return Err("legacy hook completion cannot be verified; reset this branch or create a new branch before handing out its URL".into());
        }
        // A prepared snapshot already contains its successfully initialized data.
        // Replaying hooks into every pool slot would duplicate fixtures/masking.
        if let Some(parent) = self.parent().and_then(|p| Branch::load(&p).ok()).filter(pool::is_snapshot) {
            if !parent.run().join("branch_sql.done-v1").exists() && !self.hooks().is_empty() {
                return Err("legacy snapshot hook completion cannot be verified; prepare a new snapshot".into());
            }
            return io(fs::write(self.run().join("branch_sql.done-v1"), ""));
        }
        for (label, script) in self.hooks() {
            // Preserve successful hooks on retry, but rerun if their content changes.
            let done = self.run().join(format!("hook-done.{label}"));
            if fs::read_to_string(&done).ok().as_deref() == Some(script.as_str()) { continue; }
            let result = if self.engine == Engine::Sqlite {
                let _lock = sqlite::lock(self)?;
                rusqlite::Connection::open(self.data().join("db.sqlite"))
                    .and_then(|db| db.execute_batch(&script)).map_err(|e| e.to_string())
            } else { self.sql(&script).map(drop) };
            result.map_err(|e| format!("{label} failed on {}; branch is not ready: {e}", self.name))?;
            write_secret(&done, &script)?;
        }
        io(fs::write(self.run().join("branch_sql.done-v1"), ""))
    }

    /// A fresh clone inherits its parent's replication config. Cut it loose so it never
    /// pulls from production, then move sequences past the replicated rows so inserts on
    /// the branch do not collide (logical replication does not carry sequence values).
    fn detach(&self) -> R<()> {
        match self.engine {
            Engine::Postgres => {
                self.sql(
                    "DO $$ DECLARE s name; BEGIN FOR s IN SELECT subname FROM pg_subscription WHERE subdbid = (SELECT oid FROM pg_database WHERE datname = current_database()) LOOP \
                     EXECUTE format('ALTER SUBSCRIPTION %I DISABLE', s); \
                     EXECUTE format('ALTER SUBSCRIPTION %I SET (slot_name = NONE)', s); \
                     EXECUTE format('DROP SUBSCRIPTION %I', s); END LOOP; END $$",
                )?;
                if self.synced_ancestry() {
                    self.sql(
                        "DO $$ DECLARE r record; BEGIN FOR r IN \
                         SELECT s.oid::regclass AS seq, a.attname, c.oid::regclass AS tbl FROM pg_class s \
                         JOIN pg_depend d ON d.objid = s.oid AND d.deptype IN ('a', 'i') \
                         JOIN pg_class c ON c.oid = d.refobjid \
                         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = d.refobjsubid \
                         WHERE s.relkind = 'S' LOOP \
                         EXECUTE format('SELECT setval(%L, COALESCE((SELECT max(%I) FROM %s), 0) + 1, false)', r.seq, r.attname, r.tbl); \
                         END LOOP; END $$",
                    )?;
                }
                Ok(())
            }
            Engine::Mysql if self.synced_ancestry() => self.sql("STOP REPLICA; RESET REPLICA ALL").map(drop),
            _ => Ok(()),
        }
    }

    /// Pick up tables that joined the publication since last time (new tables on production).
    /// Only once the initial copy is done: a REFRESH during a large copy can stall the apply
    /// worker past wal_receiver_timeout, which disable_on_error then turns into a pause.
    fn refresh(&self) {
        let _ = self.refresh_checked();
    }

    fn refresh_checked(&self) -> R<()> {
        if self.engine == Engine::Postgres && self.source().is_some() && self.running() {
            let sub = self.subname();
            let ready = self.sql(&format!(
                "SELECT subenabled AND NOT EXISTS (SELECT 1 FROM pg_subscription_rel WHERE srsubstate <> 'r') FROM pg_subscription WHERE subname = '{sub}'"
            ));
            if ready.as_deref() == Ok("t") {
                // REFRESH takes locks and can restart apply. Do not interrupt a
                // large transaction on every status/branch request when the
                // publication membership has not changed.
                let published = psql(&self.source().unwrap(), &format!(
                    "SELECT coalesce(string_agg(format('%I.%I', schemaname, tablename), ',' ORDER BY schemaname, tablename), '') FROM pg_publication_tables WHERE pubname='{sub}'"
                ))?;
                let subscribed = self.sql(&format!(
                    "SELECT coalesce(string_agg(format('%I.%I', n.nspname, c.relname), ',' ORDER BY n.nspname, c.relname), '') FROM pg_subscription_rel r JOIN pg_subscription s ON s.oid=r.srsubid JOIN pg_class c ON c.oid=r.srrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE s.subname='{sub}'"
                ))?;
                if published != subscribed {
                    self.sql(&format!("ALTER SUBSCRIPTION {sub} REFRESH PUBLICATION"))?;
                }
            }
        }
        Ok(())
    }

    /// Long-running mongosh that applies production change events to this replica.
    fn spawn_tail(&self) -> R<()> {
        let run = self.run();
        if live_pid(&run.join("tailpid")).is_some() {
            return Ok(());
        }
        let _ = fs::remove_file(run.join("tail.failed"));
        io(fs::write(run.join("tail.js"), MONGO_TAIL_JS))?;
        let log = io(fs::OpenOptions::new().create(true).append(true).open(run.join("tail.log")))?;
        let child = Command::new("mongosh")
            .args(["--quiet", &self.engine_url()?, "--file"])
            .arg(run.join("tail.js"))
            .env("SRC", self.source().unwrap_or_default())
            .env("RUN", &run)
            .stdin(Stdio::null())
            .stdout(Stdio::from(io(log.try_clone())?))
            .stderr(Stdio::from(log))
            .process_group(0)
            .spawn()
            .map_err(|e| format!("mongosh: {e}"))?;
        io(fs::write(run.join("tailpid"), child.id().to_string()))
    }

    fn kill_tail(&self) {
        if let Some(pid) = live_pid(&self.run().join("tailpid")) {
            let _ = sh(Command::new("kill").args(["-TERM", &pid.to_string()]));
        }
    }

    /// Suspend: stop the engine (and tailer). The proxy stays, so the URL keeps working and
    /// the next connection resumes the engine.
    pub fn stop(&self) -> R<()> {
        // SQLite has no engine process to suspend; stop its HTTP endpoint instead.
        if self.engine == Engine::Sqlite {
            let _lock = sqlite::lock(self)?;
            if let Some(pid) = live_pid(&self.run().join("proxypid")) {
                sh(Command::new("kill").args(["-TERM", &pid.to_string()]))?;
                wait(|| !alive(pid), 10, "SQLite endpoint to stop")?;
            }
            return Ok(());
        }
        self.kill_tail();
        let Some(pid) = self.pid() else { return Ok(()) };
        if self.engine == Engine::Postgres && self.source().is_some() {
            eprintln!("note: while {} is stopped its replication slot retains WAL on the source; start it again soon or rm it", self.name);
        }
        match self.engine {
            Engine::Postgres => sh(Command::new("pg_ctl").arg("-D").arg(self.data()).args(["-m", "fast", "-w", "stop"])),
            // SIGTERM is the clean shutdown for both mysqld and mongod.
            _ => sh(Command::new("kill").args(["-TERM", &pid.to_string()]))
                .and_then(|_| wait(|| !alive(pid), 180, "server to shut down")),
        }
    }

    fn rm(&self) -> R<()> {
        pool::before_remove(self)?;
        if self.dir.join("lock").exists() {
            return Err(format!("{0} is locked; run: snapshotdb unlock {0}", self.name));
        }
        if let Some(url) = self.source() {
            // Release production-side resources first: an orphaned Postgres slot retains WAL forever.
            let released = self.start().and_then(|_| match self.engine {
                Engine::Postgres => self
                    .sql(&format!("DROP SUBSCRIPTION IF EXISTS {}", self.subname()))
                    .and_then(|_| psql(&url, &format!(
                        "DROP EVENT TRIGGER IF EXISTS {0}; DROP SCHEMA IF EXISTS {0} CASCADE; DROP PUBLICATION IF EXISTS {0}",
                        self.subname()
                    ))),
                Engine::Mysql => self.sql("STOP REPLICA; RESET REPLICA ALL"),
                _ => Ok(String::new()),
            });
            if let Err(e) = released {
                eprintln!(
                    "warning: could not release replication on the source: {e}\nrun on production:\n  \
                     SELECT pg_drop_replication_slot('{0}'); DROP EVENT TRIGGER IF EXISTS {0}; \
                     DROP SCHEMA IF EXISTS {0} CASCADE; DROP PUBLICATION IF EXISTS {0};",
                    self.subname()
                );
            }
        }
        let _hold = self.hold()?;
        if let Some(pid) = live_pid(&self.run().join("proxypid")) {
            sh(Command::new("kill").args(["-TERM", &pid.to_string()]))?;
            wait(|| !alive(pid), 10, "branch proxy to stop")?;
        }
        self.stop()?;
        if self.is_current() {
            let _ = fs::remove_file(home().join(".current"));
        }
        io(fs::remove_dir_all(&self.dir))?;
        let alias = home().join(&self.name);
        if fs::symlink_metadata(&alias).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            io(fs::remove_file(alias))?;
        }
        Ok(())
    }

    // --- introspection -------------------------------------------------------------

    fn info_json(&self) -> R<String> {
        Ok(format!(
            "{{\"schema_version\":1,\"name\":{},\"engine\":{},\"parent\":{},\"status\":{},\"url\":{},\"port\":{},\"synced\":{},\"locked\":{},\"current\":{}}}",
            js(&self.name),
            js(self.engine.name()),
            self.parent().map(|p| js(&p)).unwrap_or_else(|| "null".into()),
            js(self.status_word()),
            self.url().map(|u| js(&u)).unwrap_or_else(|_| "null".into()),
            self.port().map(|p| p.to_string()).unwrap_or_else(|| "null".into()),
            self.source().is_some(),
            self.dir.join("lock").exists(),
            self.is_current(),
        ))
    }

    /// Replication state of a synced root as (label, value) lines.
    fn status(&self) -> R<Vec<(&'static str, String)>> {
        let url = self.source().ok_or_else(|| format!("{} is not a synced root", self.name))?;
        let mut lines = vec![("state", self.status_word().to_string())];
        if !self.running() {
            return Ok(lines);
        }
        match self.engine {
            Engine::Postgres => {
                self.refresh();
                let sub = self.subname();
                lines.push(("tables", self.sql(
                    "SELECT coalesce(string_agg(n || ' ' || CASE s WHEN 'r' THEN 'ready' WHEN 's' THEN 'synced' \
                     WHEN 'd' THEN 'copying' WHEN 'f' THEN 'finishing' ELSE 'queued' END, ', ' ORDER BY s), 'none') \
                     FROM (SELECT srsubstate s, count(*) n FROM pg_subscription_rel GROUP BY 1) x",
                )?));
                if self.sql("SELECT count(*) FROM pg_subscription_rel WHERE srsubstate <> 'r'")? != "0" {
                    let local: u64 = self.sql("SELECT coalesce(sum(pg_table_size(srrelid)), 0)::bigint FROM pg_subscription_rel")?.parse().unwrap_or(0);
                    let remote: u64 = psql(&url, &format!(
                        "SELECT coalesce(sum(pg_table_size(pr.prrelid)), 0)::bigint FROM pg_publication_rel pr JOIN pg_publication p ON p.oid = pr.prpubid WHERE p.pubname = '{sub}'"
                    )).ok().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let pct = if remote > 0 { local * 100 / remote } else { 0 };
                    lines.push(("initial copy", format!("{} of {} ({pct}%)", human(local), human(remote))));
                }
                let enabled = self.sql(&format!("SELECT subenabled FROM pg_subscription WHERE subname = '{sub}'"))?;
                if enabled == "f" {
                    let (err, lsn) = self.last_apply_error();
                    lines.push(("stream", format!("PAUSED after an error: {err}")));
                    lines.push(("repair", match lsn {
                        _ if err.contains("missing replicated column") || err.contains("does not exist") => format!("snapshotdb repair {} reconciles the schema from the source and resumes", self.name),
                        Some(l) => format!("snapshotdb repair {} skips the transaction at {l} and resumes", self.name),
                        None => format!("snapshotdb repair {} resumes (the error was not tied to a transaction)", self.name),
                    }));
                } else {
                    let stream = self.sql(
                        "SELECT coalesce(latest_end_lsn::text, '0/0') || ' ' || coalesce(extract(epoch FROM now() - last_msg_receipt_time)::int::text, '?') \
                         FROM pg_stat_subscription WHERE relid IS NULL AND pid IS NOT NULL",
                    )?;
                    lines.push(("stream", match stream.split_once(' ') {
                        None => "disconnected (see run/log)".to_string(),
                        Some((lsn, age)) => {
                            let behind = psql(&url, &format!("SELECT pg_size_pretty(pg_wal_lsn_diff(pg_current_wal_lsn(), '{lsn}'))")).unwrap_or_else(|_| "unknown".into());
                            format!("connected, {behind} behind source, last message {age}s ago")
                        }
                    }));
                }
                lines.push(("slot", psql(&url, &format!(
                    "SELECT coalesce(wal_status || ', retaining ' || pg_size_pretty(pg_wal_lsn_diff(pg_current_wal_lsn(), restart_lsn)) || ' of WAL on the source', 'missing on source') \
                     FROM pg_replication_slots WHERE slot_name = '{sub}'"
                )).unwrap_or_else(|_| "source unreachable".into())));
                let tracked = psql(&url, &format!("SELECT count(*) FROM pg_event_trigger WHERE evtname = '{sub}'")).unwrap_or_default() == "1";
                lines.push(("schema changes", if tracked && self.sql(&format!("SELECT to_regclass('{sub}.ddl') IS NOT NULL"))? == "t" {
                    self.sql(&format!(
                        "SELECT count(*) FILTER (WHERE error IS NULL) || ' applied, ' || count(*) FILTER (WHERE error IS NOT NULL) || ' failed' \
                         || coalesce(' (last: ' || (SELECT error FROM {sub}.ddl WHERE error IS NOT NULL ORDER BY id DESC LIMIT 1) || ')', '') FROM {sub}.ddl"
                    ))?
                } else {
                    format!("not tracked (no event trigger on the source); after migrations run: snapshotdb reconcile {}", self.name)
                }));
            }
            Engine::Mysql => {
                let threads = self.sql(
                    "SELECT concat('io ', c.SERVICE_STATE, ', sql ', a.SERVICE_STATE, \
                     coalesce(concat(', io error: ', nullif(c.LAST_ERROR_MESSAGE, '')), ''), \
                     coalesce(concat(', sql error: ', nullif(k.LAST_ERROR_MESSAGE, '')), ''), \
                     coalesce(concat(', worker error: ', nullif((SELECT max(LAST_ERROR_MESSAGE) FROM performance_schema.replication_applier_status_by_worker w WHERE w.LAST_ERROR_NUMBER <> 0), '')), '')) \
                     FROM performance_schema.replication_connection_status c \
                     JOIN performance_schema.replication_applier_status a USING (CHANNEL_NAME) \
                     LEFT JOIN performance_schema.replication_applier_status_by_coordinator k USING (CHANNEL_NAME)",
                )?;
                let (user, pass, host, port) = parse_url(&url)?;
                let pending = mysql_on(&host, port, &user, &pass, "SELECT @@gtid_executed")
                    .and_then(|src| self.sql(&format!("SELECT GTID_SUBTRACT('{src}', @@gtid_executed)")))
                    .map(|gap| if gap.is_empty() { "caught up".to_string() } else { format!("behind by {gap}") })
                    .unwrap_or_else(|_| "lag unknown (source unreachable)".into());
                let paused = threads.contains("error:");
                lines.push(("stream", format!("{}{}, {pending}", if paused { "PAUSED: " } else { "" }, if threads.is_empty() { "not configured".to_string() } else { threads })));
                if paused {
                    lines.push(("repair", format!("snapshotdb repair {} skips the failing transaction", self.name)));
                }
            }
            Engine::Mongodb => {
                let run = self.run();
                let tailing = live_pid(&run.join("tailpid")).is_some();
                let applied = fs::read_to_string(run.join("applied")).unwrap_or_else(|_| "nothing yet".into());
                let failed = fs::read_to_string(run.join("tail.errors")).map(|s| s.lines().count()).unwrap_or(0);
                lines.push(("stream", format!("{}, last event applied {applied}", if tailing { "tailing" } else { "stopped (snapshotdb repair restarts it; see run/tail.log)" })));
                lines.push(("failed event attempts", format!("{failed}{}", if failed > 0 { " (checkpoint retained; see run/tail.errors)" } else { "" })));
            }
            Engine::Sqlite => {}
        }
        Ok(lines)
    }

    /// Last replication-worker error in the Postgres log and, when the error came from
    /// applying one remote transaction, the LSN that ends it.
    fn last_apply_error(&self) -> (String, Option<String>) {
        let log = fs::read_to_string(self.run().join("log")).unwrap_or_default();
        let lines: Vec<&str> = log.lines().collect();
        let at = lines
            .iter()
            .rposition(|l| l.contains("ERROR:") && l.contains("logical replication"))
            .or_else(|| lines.iter().rposition(|l| l.contains("ERROR:") && !l.contains("ALTER SUBSCRIPTION") && !l.contains("already exists")));
        let Some(at) = at else { return ("unknown".into(), None) };
        let err = lines[at].split("ERROR:").nth(1).unwrap_or("").trim().to_string();
        let lsn = lines[at..].iter().take(6).find_map(|l| l.split("finished at ").nth(1)).map(|s| s.trim().trim_end_matches('.').to_string());
        (err, lsn)
    }

    /// Skip the transaction that paused replication and resume. That transaction is lost
    /// on the replica; the message says which.
    fn repair(&self) -> R<String> {
        self.source().ok_or_else(|| format!("{} is not a synced root", self.name))?;
        self.start()?;
        match self.engine {
            Engine::Postgres => {
                let sub = self.subname();
                let failures = self.sql(&format!("SELECT count(*) FROM {sub}.ddl WHERE error IS NOT NULL")).unwrap_or_default();
                if !failures.is_empty() && failures != "0" {
                    self.sql(&format!(
                        "DO $$ DECLARE r record; BEGIN FOR r IN SELECT id,sql,search_path FROM {sub}.ddl WHERE error IS NOT NULL ORDER BY id LOOP \
                         BEGIN PERFORM set_config('search_path',r.search_path,true); \
                         EXECUTE {sub}.ddl_only(r.sql); \
                         UPDATE {sub}.ddl SET error=NULL WHERE id=r.id; \
                         EXCEPTION WHEN OTHERS THEN UPDATE {sub}.ddl SET error=SQLERRM WHERE id=r.id; END; END LOOP; END $$;"
                    ))?;
                    let remaining = self.sql(&format!("SELECT count(*) FROM {sub}.ddl WHERE error IS NOT NULL"))?;
                    if remaining != "0" { return Err(format!("{remaining} schema changes still fail; correct their prerequisites and run repair again")); }
                    if self.sql(&format!("SELECT subenabled FROM pg_subscription WHERE subname='{sub}'"))? == "t" {
                        return Ok(format!("replayed {failures} failed schema changes; replication is enabled"));
                    }
                }
                if self.sql(&format!("SELECT subenabled FROM pg_subscription WHERE subname = '{sub}'"))? == "t" {
                    return Ok("replication is not paused; nothing to repair".into());
                }
                let (err, lsn) = self.last_apply_error();
                // A migration on a source without the event trigger: bring the schema up, then resume.
                if err.contains("missing replicated column") || err.contains("does not exist") {
                    let did = self.reconcile()?;
                    self.sql(&format!("ALTER SUBSCRIPTION {sub} ENABLE"))?;
                    return Ok(format!("{did}; resumed ({err})"));
                }
                match lsn {
                    // A transaction the replica cannot apply: skip exactly that one.
                    Some(lsn) => {
                        self.sql(&format!("ALTER SUBSCRIPTION {sub} SKIP (lsn = '{lsn}')"))?;
                        self.sql(&format!("ALTER SUBSCRIPTION {sub} ENABLE"))?;
                        Ok(format!("skipped the transaction finishing at {lsn} ({err}) and resumed"))
                    }
                    // A timeout or connection error: nothing to skip, just resume.
                    None => {
                        self.sql(&format!("ALTER SUBSCRIPTION {sub} ENABLE"))?;
                        Ok(format!("resumed; the pause was not tied to a transaction ({err})"))
                    }
                }
            }
            Engine::Mysql => {
                let gtid = self.sql(
                    "SELECT coalesce(nullif((SELECT APPLYING_TRANSACTION FROM performance_schema.replication_applier_status_by_worker WHERE LAST_ERROR_NUMBER <> 0 LIMIT 1), ''), \
                     (SELECT GTID_SUBTRACT(RECEIVED_TRANSACTION_SET, @@gtid_executed) FROM performance_schema.replication_connection_status LIMIT 1), '')",
                )?;
                // A set like uuid:5-9 → its first transaction uuid:5 (the uuid itself has hyphens).
                let first = match gtid.split(',').next().unwrap_or("").trim().rsplit_once(':') {
                    Some((uuid, range)) => format!("{uuid}:{}", range.split('-').next().unwrap_or(range)),
                    None => return Ok("replication is not paused; nothing to repair".into()),
                };
                self.sql(&format!("STOP REPLICA; SET GTID_NEXT = '{first}'; BEGIN; COMMIT; SET GTID_NEXT = 'AUTOMATIC'; START REPLICA"))?;
                Ok(format!("skipped transaction {first} with an empty commit and resumed"))
            }
            Engine::Mongodb => {
                self.kill_tail();
                wait(|| live_pid(&self.run().join("tailpid")).is_none(), 30, "tailer to exit")?;
                self.spawn_tail()?;
                Ok("tailer restarted from its last successful resume token; failed events will be retried".into())
            }
            Engine::Sqlite => Err("sqlite does not replicate".into()),
        }
    }

    /// Without the event trigger, schema changes on the source do not replay. Reconcile adds
    /// the columns the source gained to the replica's published tables, and puts new keyed
    /// tables into the publication and onto the replica. Drops and renames are left alone.
    fn reconcile(&self) -> R<String> {
        let url = self.source().ok_or_else(|| format!("{} is not a synced root", self.name))?;
        if self.engine != Engine::Postgres {
            return Err("reconcile applies to Postgres roots; MySQL and MongoDB replicate schema natively".into());
        }
        self.start()?;
        let sub = self.subname();
        let schemas = self.setting("schemas").unwrap_or_else(|| "public".into());
        let list = schemas.split(',').map(|s| format!("'{}'", s.trim().replace('\'', "''"))).collect::<Vec<_>>().join(",");
        let columns = |where_tables: &str| format!(
            "SELECT format('%I.%I', n.nspname, c.relname) || '|' || quote_ident(a.attname) || '|' || format_type(a.atttypid, a.atttypmod) \
             FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE a.attnum > 0 AND NOT a.attisdropped AND c.oid IN ({where_tables}) ORDER BY 1, a.attnum"
        );
        let source_cols = psql(&url, &columns(&format!("SELECT prrelid FROM pg_publication_rel pr JOIN pg_publication p ON p.oid = pr.prpubid WHERE p.pubname = '{sub}'")))?;
        let replica_cols = self.sql(&columns("SELECT srrelid FROM pg_subscription_rel"))?;
        let have: std::collections::HashSet<String> = replica_cols.lines().map(|l| l.rsplit_once('|').map(|x| x.0.to_string()).unwrap_or_default()).collect();
        let mut added_cols = 0;
        for line in source_cols.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 3 && !have.contains(&format!("{}|{}", parts[0], parts[1])) && !parts[0].starts_with(&format!("{sub}.")) {
                self.sql(&format!("ALTER TABLE {} ADD COLUMN {} {}", parts[0], parts[1], parts[2]))?;
                added_cols += 1;
            }
        }
        // New keyed tables in the synced schemas that are not published yet.
        let new_tables = psql(&url, &format!(
            "SELECT string_agg(format('%I.%I', n.nspname, c.relname), ' ') FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r', 'p') AND n.nspname IN ({list}) \
             AND (c.relreplident IN ('f', 'i') OR EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = c.oid AND i.indisprimary)) \
             AND NOT EXISTS (SELECT 1 FROM pg_publication_rel pr JOIN pg_publication p ON p.oid = pr.prpubid WHERE p.pubname = '{sub}' AND pr.prrelid = c.oid)"
        ))?;
        let mut added_tables = 0;
        for table in new_tables.split_whitespace() {
            let mut dump = Command::new("pg_dump");
            dump.args(["--schema-only", "--no-owner", "--no-publications", "--no-subscriptions", "-t", table, "-d", &url]);
            pipe(&mut dump, Command::new("psql").arg(self.socket_url(&self.database())?).args(["-X", "-q", "-o", "/dev/null"]))?;
            psql(&url, &format!("ALTER PUBLICATION {sub} ADD TABLE {table}"))?;
            added_tables += 1;
        }
        if added_tables > 0 {
            if self.sql(&format!("SELECT subenabled FROM pg_subscription WHERE subname = '{sub}'"))? == "f" {
                self.sql(&format!("ALTER SUBSCRIPTION {sub} ENABLE"))?;
            }
            self.sql(&format!("ALTER SUBSCRIPTION {sub} REFRESH PUBLICATION"))?;
        }
        Ok(format!("added {added_cols} columns and {added_tables} tables from the source"))
    }

    // --- settings ------------------------------------------------------------------

    /// branch_sql scripts on the root: `branch_sql` plus `branch_sql.<hook>`, in file-name order.
    fn hooks(&self) -> Vec<(String, String)> {
        let root = self.root();
        let mut names: Vec<String> = fs::read_dir(&root.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n == "branch_sql" || n.starts_with("branch_sql."))
            .collect();
        names.sort();
        names.into_iter().filter_map(|n| fs::read_to_string(root.dir.join(&n)).ok().map(|sql| (n, sql))).collect()
    }

    fn hook_file(key: &str, hook: Option<&String>) -> R<String> {
        match hook {
            None => Ok(key.to_string()),
            Some(_) if key != "branch_sql" => Err("--hook only applies to branch_sql".into()),
            Some(h) if h.is_empty() || !h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') => Err(format!("hook name {h:?} must be [a-z0-9-]")),
            Some(h) => Ok(format!("branch_sql.{h}")),
        }
    }

    fn settings_list(&self) -> R<String> {
        let mut s = String::new();
        for key in ["default_db", "source"] {
            let v = match (key, self.setting(key)) {
                (_, None) => "(unset)".to_string(),
                ("source", Some(u)) => redact(&u),
                (_, Some(v)) => v,
            };
            s += &format!("{key:<11} {v}\n");
        }
        let hooks = self.hooks();
        if hooks.is_empty() {
            s += &format!("{:<11} (unset)\n", "branch_sql");
        }
        for (label, sql) in hooks {
            s += &format!("{label:<11} {} bytes, {} lines\n", sql.len(), sql.lines().count());
        }
        s += &format!("{:<11} {}\n", "lock", if self.dir.join("lock").exists() { "locked" } else { "unlocked" });
        Ok(s)
    }

    fn set_setting(&self, key: &str, value: &str, hook: Option<&String>) -> R<()> {
        if self.parent().is_some() {
            return Err(format!("settings live on roots; {} is a branch of {}", self.name, self.parent().unwrap_or_default()));
        }
        match key {
            "default_db" => io(fs::write(self.dir.join(key), value)),
            "branch_sql" => {
                let sql = match value.strip_prefix('@') {
                    Some(path) => fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
                    None => value.to_string(),
                };
                io(fs::write(self.dir.join(Self::hook_file(key, hook)?), sql))
            }
            "source" => self.set_source(value),
            _ => Err(format!("unknown setting {key:?}; keys: default_db, branch_sql, source")),
        }
    }

    fn remove_setting(&self, key: &str, hook: Option<&String>) -> R<()> {
        pool::writable(self)?;
        match key {
            "default_db" | "branch_sql" => io(fs::remove_file(self.dir.join(Self::hook_file(key, hook)?)).or(Ok(()))),
            _ => Err(format!("cannot remove {key:?}; only default_db and branch_sql")),
        }
    }

    /// Rotate the production URL (credentials, host) without re-syncing.
    fn set_source(&self, url: &str) -> R<()> {
        self.source().ok_or_else(|| format!("{} is not a synced root", self.name))?;
        self.start()?;
        match self.engine {
            Engine::Postgres => self.sql(&format!("ALTER SUBSCRIPTION {} CONNECTION '{}'", self.subname(), replication_conninfo(url).replace('\'', "''")))?,
            Engine::Mysql => {
                let (user, pass, host, port) = parse_url(url)?;
                self.sql(&format!(
                    "STOP REPLICA; CHANGE REPLICATION SOURCE TO SOURCE_HOST='{host}', SOURCE_PORT={port}, SOURCE_USER='{user}', \
                     SOURCE_PASSWORD='{}', SOURCE_AUTO_POSITION=1, GET_SOURCE_PUBLIC_KEY=1; START REPLICA",
                    pass.replace('\'', "''")
                ))?
            }
            Engine::Mongodb => {
                self.kill_tail();
                wait(|| live_pid(&self.run().join("tailpid")).is_none(), 30, "tailer to exit")?;
                write_secret(&self.dir.join("source"), url)?;
                self.spawn_tail()?;
                String::new()
            }
            Engine::Sqlite => unreachable!(),
        };
        write_secret(&self.dir.join("source"), url)
    }
}

fn import(engine: &str, name: &str, source: &str) -> R<Branch> {
    let b = Branch::new(name, Engine::parse(engine)?)?;
    let src = Path::new(source);
    let result = if source == "--new" {
        b.init()
    } else if b.engine == Engine::Sqlite && src.is_file() {
        let wal = PathBuf::from(format!("{source}-wal"));
        io(fs::create_dir(b.data()))
            .and_then(|_| clone(src, &b.data().join("db.sqlite")))
            .and_then(|_| if wal.exists() { clone(&wal, &b.data().join("db.sqlite-wal")) } else { Ok(()) })
    } else {
        b.engine.check_stopped(src).and_then(|_| clone(src, &b.data()))
    };
    finish(b, result)
}

/// Make a root that continuously replicates from a production database, using the
/// engine's own replication (Postgres logical replication, MySQL GTID replication,
/// MongoDB change streams). Preflight first; nothing is created on the source if it fails.
fn sync(engine: &str, name: &str, url: &str, schemas: &str, fix_identity: bool) -> R<Branch> {
    let engine = Engine::parse(engine)?;
    let started = Instant::now();
    let mut report = preflight::run(engine, url, schemas)?;
    if fix_identity && !report.unkeyed.is_empty() {
        psql(url, &report.unkeyed.join(" "))?;
        eprintln!("applied REPLICA IDENTITY FULL to {} tables on the source", report.unkeyed.len());
        report = preflight::run(engine, url, schemas)?;
    }
    eprint!("{}", report.render(false));
    if !report.passed() {
        return Err("preflight failed; nothing was created".into());
    }
    let source_db = url.split_once("://").map(|x| x.1).and_then(|r| r.split(['?', '#']).next()).and_then(|r| r.split_once('/')).map(|x| x.1.to_string()).unwrap_or_default();
    match engine {
        Engine::Postgres => {
            let list = schemas.split(',').map(|s| format!("'{}'", s.trim().replace('\'', "''"))).collect::<Vec<_>>().join(",");
            let tables = report.tables.clone();
            let b = Branch::new(name, engine)?;
            let sub = b.subname();
            let teardown = format!("DROP EVENT TRIGGER IF EXISTS {sub}; DROP SCHEMA IF EXISTS {sub} CASCADE; DROP PUBLICATION IF EXISTS {sub}");
            // Schema changes: an event trigger on the source logs each DDL statement into a table
            // that is itself replicated, and a trigger on the replica replays it. The table and
            // function need only CREATE on the database; the event trigger itself needs superuser.
            let ddl = match psql(url, &teardown).and_then(|_| psql(url, &ddl_source_sql(&sub, &list))) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("schema changes will not replicate (could not create {sub} on the source): {e}");
                    let _ = psql(url, &teardown);
                    false
                }
            };
            let db = if source_db.is_empty() { "postgres".to_string() } else { source_db };
            let result = write_secret(&b.dir.join("source"), url)
                .and_then(|_| io(fs::write(b.dir.join("default_db"), &db)))
                .and_then(|_| io(fs::write(b.dir.join("schemas"), schemas)))
                .and_then(|_| b.init())
                .and_then(|_| b.start())
                .and_then(|_| {
                    if db != "postgres" {
                        psql(&b.socket_url("postgres")?, &format!("CREATE DATABASE \"{}\"", db.replace('"', "\"\"")))?;
                    }
                    // Roles first so GRANTs in the schema dump resolve; roles that already exist error harmlessly.
                    let _ = pipe(
                        Command::new("pg_dumpall").args(["--roles-only", "--no-role-passwords", "-d", url]),
                        Command::new("psql").arg(b.socket_url(&db)?).args(["-X", "-q"]),
                    );
                    // The dump recreates the schemas it covers; the fresh database's own `public` would collide.
                    b.sql("DROP SCHEMA IF EXISTS public CASCADE")?;
                    let mut dump = Command::new("pg_dump");
                    dump.args(["--schema-only", "--no-owner", "--no-publications", "--no-subscriptions", "-d", url]);
                    for schema in schemas.split(',').chain(ddl.then_some(sub.as_str())) {
                        dump.args(["-n", schema.trim()]);
                    }
                    // pg_dump locks every table it dumps; a restricted role cannot lock tables it
                    // may not read, so leave those out (they are not published anyway).
                    let unreadable = psql(url, &format!(
                        "SELECT coalesce(string_agg(format('%I.%I', n.nspname, c.relname), ' '), '') FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                         WHERE c.relkind IN ('r', 'p', 'm', 'f') AND n.nspname IN ({list}) AND NOT has_table_privilege(c.oid, 'SELECT')"
                    ))?;
                    for table in unreadable.split_whitespace() {
                        dump.args(["--exclude-table", table]);
                    }
                    let warnings = pipe(&mut dump, Command::new("psql").arg(b.socket_url(&db)?).args(["-X", "-q", "-o", "/dev/null"]))?;
                    if !warnings.trim().is_empty() {
                        io(fs::write(b.run().join("schema.log"), &warnings))?;
                        eprintln!("schema load warnings (usually extensions or roles missing locally; saved to run/schema.log):\n{warnings}");
                    }
                    let mut published = tables.clone();
                    if ddl {
                        b.sql(&ddl_replica_sql(&sub))?;
                        published += &format!(", {sub}.ddl");
                    }
                    // Install the freshness barrier during initial setup, so
                    // the first branch needs no publication change or copy.
                    if !ddl { psql(url, &format!("CREATE SCHEMA {sub}"))?; }
                    let freshness_table = format!("CREATE TABLE {sub}._freshness_v1 (id integer PRIMARY KEY, token text NOT NULL)");
                    psql(url, &freshness_table)?;
                    if !ddl { b.sql(&format!("CREATE SCHEMA IF NOT EXISTS {sub}"))?; }
                    b.sql(&freshness_table)?;
                    published += &format!(", {sub}._freshness_v1");
                    psql(url, &format!("DROP PUBLICATION IF EXISTS {sub}"))?;
                    psql(url, &format!("CREATE PUBLICATION {sub} FOR TABLE {published}"))?;
                    if ddl {
                        // Last, so our own publication statements are not logged and replayed.
                        if let Err(e) = psql(url, &format!("CREATE EVENT TRIGGER {sub} ON ddl_command_end EXECUTE FUNCTION {sub}.log_ddl()")) {
                            eprintln!("schema changes will not replay automatically (an event trigger on the source needs superuser): {}\n  after production migrations run: snapshotdb reconcile {name}", e.lines().last().unwrap_or(""));
                        }
                    }
                    // disable_on_error: a poisoned transaction pauses the stream instead of retrying forever;
                    // `status` shows it and `repair` skips it.
                    b.sql(&format!(
                        "CREATE SUBSCRIPTION {sub} CONNECTION '{}' PUBLICATION {sub} WITH (disable_on_error = true)",
                        replication_conninfo(url).replace('\'', "''")
                    ))?;
                    io(fs::write(b.dir.join("freshness-v1"), ""))
                });
            if let Err(e) = result {
                let _ = psql(url, &teardown);
                let _ = b.rm_quiet();
                return Err(e);
            }
            eprintln!("{name} replicates {} tables from the source; initial copy continues in the background ({:.1}s so far). Check: snapshotdb status {name}", tables.matches(", ").count() + 1, started.elapsed().as_secs_f64());
            b.set_current()?;
            Ok(b)
        }
        Engine::Mysql => {
            let (user, pass, host, port) = parse_url(url)?;
            let dbs = report.databases.clone();
            let b = Branch::new(name, engine)?;
            let result = write_secret(&b.dir.join("source"), url)
                .and_then(|_| if source_db.is_empty() { Ok(()) } else { io(fs::write(b.dir.join("default_db"), &source_db)) })
                .and_then(|_| b.init())
                .and_then(|_| b.start())
                .and_then(|_| {
                    let mut dump = Command::new("mysqldump");
                    dump.args(["--no-defaults", "-h", &host, "-P", &port.to_string(), "-u", &user])
                        .args(["--single-transaction", "--set-gtid-purged=ON", "--routines", "--triggers", "--events", "--databases"])
                        .args(dbs.split(' '))
                        .env("MYSQL_PWD", &pass);
                    let mut restore = Command::new("mysql");
                    restore.args(["--no-defaults", "-u", "root", "--protocol=socket"]).arg(format!("--socket={}", b.run().join("sock").display()));
                    if let Some(pw) = b.password() {
                        restore.env("MYSQL_PWD", pw);
                    }
                    pipe(&mut dump, &mut restore)?;
                    b.sql(&format!(
                        "CHANGE REPLICATION SOURCE TO SOURCE_HOST='{host}', SOURCE_PORT={port}, SOURCE_USER='{user}', \
                         SOURCE_PASSWORD='{}', SOURCE_AUTO_POSITION=1, GET_SOURCE_PUBLIC_KEY=1; START REPLICA",
                        pass.replace('\'', "''")
                    ))
                });
            if let Err(e) = result {
                let _ = b.rm_quiet();
                return Err(e);
            }
            eprintln!("{name} replicates [{dbs}] from the source ({:.1}s). Check: snapshotdb status {name}", started.elapsed().as_secs_f64());
            b.set_current()?;
            Ok(b)
        }
        Engine::Mongodb => {
            // Take a resume token before the dump: the tailer starts there, and replaying the
            // dump window is harmless (upserts by _id).
            let token = mongosh(url, "const s = db.watch([]); s.tryNext(); print(JSON.stringify(s.getResumeToken())); s.close()")?;
            let b = Branch::new(name, engine)?;
            let result = (if source_db.is_empty() { Ok(()) } else { io(fs::write(b.dir.join("default_db"), &source_db)) })
                .and_then(|_| b.init())
                .and_then(|_| b.start())
                .and_then(|_| {
                    pipe(
                        Command::new("mongodump").args(["--uri", url, "--archive", "--quiet"]),
                        Command::new("mongorestore").args(["--uri", &b.engine_url()?, "--archive", "--drop", "--quiet", "--nsExclude", "admin.*", "--nsExclude", "config.*"]),
                    )?;
                    // `source` is written only now so start() did not launch the tailer before the restore.
                    io(fs::write(b.run().join("token"), &token))?;
                    write_secret(&b.dir.join("source"), url)?;
                    b.spawn_tail()
                });
            if let Err(e) = result {
                let _ = b.rm_quiet();
                return Err(e);
            }
            eprintln!("{name} replicates from the source via change streams ({:.1}s). Check: snapshotdb status {name}", started.elapsed().as_secs_f64());
            b.set_current()?;
            Ok(b)
        }
        Engine::Sqlite => Err("sqlite has nothing to sync from; use import".into()),
    }
}

impl Branch {
    /// Tear down a half-made branch without touching any source.
    fn rm_quiet(&self) -> R<()> {
        let _ = fs::remove_file(self.dir.join("source"));
        self.rm()
    }
}

fn create(name: &str, parent: &str) -> R<Branch> {
    let p = Branch::load(parent)?;
    if pool::is_snapshot(&p) { return pool::claim(name, &p); }
    create_cold(name, parent)
}

fn create_cold(name: &str, parent: &str) -> R<Branch> {
    let p = Branch::load(parent)?;
    if let Ok(existing) = Branch::load(name) {
        // Re-running create is safe: same branch, same URL. Scripts and agents rely on that.
        if existing.parent().as_deref() == Some(parent) {
            existing.start()?;
            existing.set_current()?;
            return Ok(existing);
        }
        return Err(format!("branch {name} already exists and was not created from {parent}"));
    }
    ensure_branch_ready(&p)?;
    let b = Branch::new(name, p.engine)?;
    let was_running = p.running();
    let cloned = {
        let _sqlite_lock = sqlite::lock(&p)?;
        // Hold the parent so its proxy cannot wake the engine while the files are cloned.
        let _hold = p.hold()?;
        if was_running {
            p.stop()?;
        }
        clone(&p.data(), &b.data())
    };
    let restarted = if was_running { p.start() } else { Ok(()) };
    io(fs::write(b.dir.join("parent"), parent))?;
    let b = finish(b, cloned.and(restarted))?;
    b.set_current()?;
    Ok(b)
}

fn ensure_branch_ready(p: &Branch) -> R<()> {
    if p.source().is_some() && p.engine == Engine::Mongodb && (p.run().join("tail.failed").exists() || live_pid(&p.run().join("tailpid")).is_none()) {
        return Err(format!("replica {} is not tailing successfully; inspect status and repair before branching", p.name));
    }
    if p.source().is_some() && p.engine == Engine::Mysql {
        let healthy = p.sql("SELECT COUNT(*) FROM performance_schema.replication_connection_status c JOIN performance_schema.replication_applier_status a USING (CHANNEL_NAME) WHERE c.SERVICE_STATE='ON' AND a.SERVICE_STATE='ON'")?;
        if healthy != "1" { return Err(format!("replica {} is paused or disconnected; inspect status and repair before branching", p.name)); }
    }
    if p.engine == Engine::Postgres && p.source().is_some() {
        p.start()?;
        p.refresh_checked()?;
        let ready = p.sql(&format!(
            "SELECT subenabled AND EXISTS (SELECT 1 FROM pg_subscription_rel WHERE srsubid = s.oid) \
             AND NOT EXISTS (SELECT 1 FROM pg_subscription_rel WHERE srsubid = s.oid AND srsubstate <> 'r') \
             FROM pg_subscription s WHERE subname = '{}'", p.subname()
        ))?;
        if let Ok(errors) = p.sql(&format!("SELECT count(*) FROM {}.ddl WHERE error IS NOT NULL", p.subname())) {
            if errors != "0" { return Err(format!("replica {} has failed schema changes; repair the recorded DDL errors before branching", p.name)); }
        }
        if ready != "t" {
            return Err(format!("replica {0} is still copying or replication is paused; check: snapshotdb status {0}", p.name));
        }
        wait_postgres_freshness(p)?;
    }
    Ok(())
}

/// A received WAL position is not proof that a transaction was applied. Emit a
/// row on the same publication and wait for its committed value locally, before
/// stopping the parent. The worker lock serializes marker writers for this root.
fn wait_postgres_freshness(p: &Branch) -> R<()> {
    let source = p.source().ok_or("freshness requires a synced root")?;
    let local = p.socket_url(&p.database())?;
    let sub = p.subname();
    let timeout = env::var("SNAPSHOTDB_FRESHNESS_TIMEOUT_SECONDS")
        .unwrap_or_else(|_| "600".into()).parse::<u64>()
        .map_err(|_| "SNAPSHOTDB_FRESHNESS_TIMEOUT_SECONDS must be an integer from 1 to 3600")?;
    if !(1..=3600).contains(&timeout) {
        return Err("SNAPSHOTDB_FRESHNESS_TIMEOUT_SECONDS must be from 1 to 3600".into());
    }
    let started = Instant::now();
    let query = |url: &str, sql: &str| -> R<String> {
        out(Command::new("psql").arg(url).env("PGCONNECT_TIMEOUT", "10")
            .args(["-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-c",
                "SET statement_timeout = '10s'", "-c", sql]))
    };
    let installed = p.dir.join("freshness-v1");
    if !installed.exists() {
        // Do not batch CREATE SCHEMA with CREATE TABLE on the publisher: its
        // event trigger logs current_query(), and replay could race local setup.
        if query(&source, &format!("SELECT to_regnamespace('{sub}') IS NOT NULL"))? != "t" {
            query(&source, &format!("CREATE SCHEMA {sub}"))?;
        }
        let table = format!("CREATE TABLE IF NOT EXISTS {sub}._freshness_v1 (id integer PRIMARY KEY, token text NOT NULL)");
        query(&source, &table)?;
        query(&local, &format!("CREATE SCHEMA IF NOT EXISTS {sub}; {table}"))?;
        query(&source, &format!(
            "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_publication_tables WHERE pubname='{sub}' AND schemaname='{sub}' AND tablename='_freshness_v1') THEN ALTER PUBLICATION {sub} ADD TABLE {sub}._freshness_v1; END IF; END $$"
        ))?;
        query(&local, &format!("ALTER SUBSCRIPTION {sub} REFRESH PUBLICATION"))?;
        // A table-copy worker could otherwise copy a new marker ahead of the
        // main apply worker. Only emit it after this table is fully synchronized.
        loop {
            let ready = query(&local, &format!(
                "SELECT EXISTS (SELECT 1 FROM pg_subscription_rel r JOIN pg_subscription s ON s.oid=r.srsubid WHERE s.subname='{sub}' AND s.subenabled AND r.srrelid='{sub}._freshness_v1'::regclass AND r.srsubstate='r')"
            ))?;
            if ready == "t" { break; }
            if started.elapsed().as_secs() >= timeout {
                return Err(format!("freshness initialization timed out for {}; parent left running", p.name));
            }
            thread::sleep(Duration::from_millis(100));
        }
        io(fs::write(installed, ""))?;
    }
    let token = random_password()?;
    query(&source, &format!(
        "INSERT INTO {sub}._freshness_v1 VALUES (1, '{token}') ON CONFLICT (id) DO UPDATE SET token=excluded.token"
    ))?;
    loop {
        let applied = query(&local, &format!(
            "SELECT EXISTS (SELECT 1 FROM {sub}._freshness_v1 WHERE id=1 AND token='{token}') AND EXISTS (SELECT 1 FROM pg_subscription WHERE subname='{sub}' AND subenabled)"
        ))?;
        if applied == "t" {
            if query(&local, &format!("SELECT to_regclass('{sub}.ddl') IS NOT NULL"))? == "t"
                && query(&local, &format!("SELECT count(*) FROM {sub}.ddl WHERE error IS NOT NULL"))? != "0" {
                return Err(format!("replica {} has failed schema changes; no branch created", p.name));
            }
            return Ok(());
        }
        if started.elapsed().as_secs() >= timeout {
            return Err(format!("replica {} has not applied the source commit within {timeout}s; no branch created and parent left running", p.name));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn reset(name: &str) -> R<Branch> {
    let b = Branch::load(name)?;
    pool::writable(&b)?;
    let parent = b.parent().ok_or_else(|| format!("{name} is a root; only branches with a parent can be reset"))?;
    if pool::is_snapshot(&Branch::load(&parent)?) { return Err("create a new prepared branch before removing this one; reset is unavailable for prepared branches".into()); }
    ensure_branch_ready(&Branch::load(&parent)?)?;
    b.rm()?;
    create(name, &parent)
}

/// Start the new branch, or remove the half-made directory.
fn finish(b: Branch, result: R<()>) -> R<Branch> {
    if let Err(e) = result.and_then(|_| b.start()) {
        // Cleanup must not erase the only explanation for a startup failure.
        let diagnostic = fs::File::open(b.run().join("log")).ok().and_then(|mut log| {
            use std::io::{Read, Seek, SeekFrom};
            let length = log.metadata().ok()?.len();
            log.seek(SeekFrom::Start(length.saturating_sub(8192))).ok()?;
            let mut bytes = Vec::new();
            log.take(8192).read_to_end(&mut bytes).ok()?;
            let dir = home().join(".failed");
            fs::create_dir_all(&dir).ok()?;
            let path = dir.join(format!("{}-{}.log", b.name, random_password().ok()?));
            let tail = String::from_utf8_lossy(&bytes);
            write_secret(&path, &tail).ok()?;
            Some(path)
        });
        let _ = b.rm_quiet();
        return Err(match diagnostic {
            Some(path) => format!("{e}\nstartup diagnostic saved on server: {}", path.display()),
            None => e,
        });
    }
    Ok(b)
}

fn all_branches() -> Vec<Branch> {
    let mut names: Vec<String> = fs::read_dir(home())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names.iter().filter_map(|n| Branch::load(n).ok()).collect()
}

/// After a reboot: every branch gets its proxy back so URLs work again, and synced roots
/// restart so replication resumes. Everything else resumes on its first connection.
fn up() -> R<()> {
    let branches = all_branches();
    // Restore every source proxy before starting subscribers, regardless of name order.
    for b in &branches {
        if pool::is_snapshot(b) { continue; }
        b.ensure_proxy()?;
    }
    for b in branches {
        if pool::is_snapshot(&b) { continue; }
        if b.engine == Engine::Sqlite {
            b.start()?;
            continue;
        }
        let upgrade = b.parent().is_some() && !b.run().join("ready-v1").exists();
        let result = if upgrade || b.source().is_some() || b.dir.join("pool-ready").exists() { b.start() } else { b.ensure_proxy() };
        match result {
            Ok(()) => println!("{}\t{}", b.name, b.status_word()),
            Err(e) => eprintln!("{}\tfailed: {e}", b.name),
        }
    }
    Ok(())
}

/// Stop consumers before their sources so disable_on_error cannot turn a planned
/// service restart into a permanently paused subscription. Proxies must not wake
/// an engine during the ordered shutdown. systemd must use KillMode=mixed.
fn down() -> R<()> {
    let mut branches = all_branches();
    branches.sort_by(|a, b| a.dir.cmp(&b.dir));
    branches.dedup_by(|a, b| a.dir == b.dir);
    branches.sort_by_key(|b| b.source().is_none());
    let _holds: Vec<Hold> = branches.iter().map(|b| b.hold()).collect::<R<_>>()?;
    for b in &branches { b.stop()?; }
    for b in branches {
        if let Some(pid) = live_pid(&b.run().join("proxypid")) {
            sh(Command::new("kill").args(["-TERM", &pid.to_string()]))?;
        }
    }
    Ok(())
}

fn list(json: bool) -> R<()> {
    let branches: Vec<_> = all_branches().into_iter().filter(|b| !b.name.starts_with(pool::PREFIX)).collect();
    if json {
        let items: R<Vec<String>> = branches.iter().map(|b| b.info_json()).collect();
        println!("[{}]", items?.join(","));
        return Ok(());
    }
    // Passwords are redacted here; `url` and `info --print-url` give the full string.
    for b in &branches {
        println!(
            "{}{}\t{}\t{}\t{}\t{}",
            if b.is_current() { "* " } else { "  " },
            b.name,
            b.engine.name(),
            b.parent().unwrap_or_else(|| "-".into()),
            b.status_word(),
            b.url().map(|u| redact(&u)).unwrap_or_default()
        );
    }
    Ok(())
}

fn render_status(lines: &[(&str, String)], json: bool) -> String {
    if json {
        let fields: Vec<String> = lines.iter().map(|(k, v)| format!("{}:{}", js(&k.replace(' ', "_")), js(v))).collect();
        return format!("{{{}}}\n", fields.join(","));
    }
    lines.iter().map(|(k, v)| format!("{k}: {v}\n")).collect()
}

// ponytail: shell out to cp. macOS -c clones via clonefile(2) but silently falls back to a
// full copy off APFS; Linux --reflink=always fails loudly off Btrfs/XFS. Upgrade path:
// call clonefile/FICLONE directly and refuse to copy.
fn clone(src: &Path, dst: &Path) -> R<()> {
    let mut cp = Command::new("cp");
    if cfg!(target_os = "macos") {
        cp.arg("-cpR");
    } else {
        cp.args(["-a", "--reflink=always"]);
    }
    sh(cp.arg(src).arg(dst))
}

/// Replication link timeouts, both sides. Postgres defaults to 60 s, which a single long
/// DDL replay or lock wait on a large table exceeds; the walsender then drops the link and
/// disable_on_error pauses the stream. A dead peer is still noticed when TCP fails.
const REPLICATION_TIMEOUT_MS: u32 = 1_800_000;

/// The source URL with wal_sender_timeout set for this connection (a USERSET parameter).
fn replication_conninfo(url: &str) -> String {
    format!("{url}{}options=-c%20wal_sender_timeout%3D{REPLICATION_TIMEOUT_MS}", if url.contains('?') { "&" } else { "?" })
}

pub fn psql(url: &str, statement: &str) -> R<String> {
    out(Command::new("psql").arg(url).args(["-X", "-At", "-v", "ON_ERROR_STOP=1", "-c", statement]))
}

pub fn mysql_on(host: &str, port: u16, user: &str, pass: &str, statement: &str) -> R<String> {
    out(Command::new("mysql")
        .args(["--no-defaults", "-N", "-B", "-h", host, "-P", &port.to_string(), "-u", user, "-e", statement])
        .env("MYSQL_PWD", pass))
}

pub fn mongosh(url: &str, script: &str) -> R<String> {
    out(Command::new("mongosh").args(["--quiet", url, "--eval", script]))
}

/// `scheme://user[:pass]@host[:port]/...` → (user, pass, host, port), percent-decoded.
pub fn parse_url(url: &str) -> R<(String, String, String, u16)> {
    let rest = url.split_once("://").map(|x| x.1).ok_or("url must look like mysql://user:pass@host:3306/")?;
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    let (auth, hostport) = authority.rsplit_once('@').ok_or("url needs user@host")?;
    let (user, pass) = auth.split_once(':').unwrap_or((auth, ""));
    let (host, port) = hostport.rsplit_once(':').unwrap_or((hostport, "3306"));
    Ok((decode(user), decode(pass), host.into(), port.parse().map_err(|_| format!("bad port {port:?}"))?))
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut outb = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                outb.push(v);
                i += 3;
                continue;
            }
        }
        outb.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&outb).into_owned()
}

/// Hide the password in a URL for display.
fn redact(url: &str) -> String {
    if let Some((base, _)) = url.split_once("?token=") { return format!("{base}?token=***"); }
    match (url.split_once("://"), url.rsplit_once('@')) {
        (Some((scheme, rest)), Some((_, host))) if rest.contains('@') => {
            let auth = rest.split('@').next().unwrap_or("");
            match auth.split_once(':') {
                Some((user, _)) => format!("{scheme}://{user}:***@{host}"),
                None => url.to_string(),
            }
        }
        _ => url.to_string(),
    }
}

fn human(bytes: u64) -> String {
    let mut v = bytes as f64;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if v < 1024.0 || unit == "TB" {
            return if unit == "B" { format!("{bytes} B") } else { format!("{v:.1} {unit}") };
        }
        v /= 1024.0;
    }
    unreachable!()
}

/// JSON string literal.
pub fn js(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// 32 hex characters from /dev/urandom: URL-safe, no encoding needed.
fn random_password() -> R<String> {
    use std::io::Read;
    let mut buf = [0u8; 16];
    io(fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

fn write_secret(path: &Path, content: &str) -> R<()> {
    io(fs::write(path, content))?;
    io(fs::set_permissions(path, fs::Permissions::from_mode(0o600)))
}

/// `producer | consumer`; the producer's stderr streams to the terminal, the consumer's
/// combined output is returned so callers can show it as warnings.
fn pipe(producer: &mut Command, consumer: &mut Command) -> R<String> {
    let pname = producer.get_program().to_string_lossy().into_owned();
    let cname = consumer.get_program().to_string_lossy().into_owned();
    let mut p = producer.stdout(Stdio::piped()).spawn().map_err(|e| format!("{pname}: {e}"))?;
    let c = consumer.stdin(p.stdout.take().unwrap()).output();
    // Command keeps the pipe's read end until it is dropped. Release it now, or a consumer
    // that exits early leaves the producer blocked on a full pipe and p.wait() never returns.
    consumer.stdin(Stdio::null());
    let c = match c {
        Ok(c) => c,
        Err(e) => { let _ = p.kill(); let _ = p.wait(); return Err(format!("{cname}: {e}")); }
    };
    let status = p.wait().map_err(|e| e.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    // The consumer's failure is the cause; the producer then only reports a broken pipe.
    if !c.status.success() {
        return Err(format!("{cname} failed:\n{text}"));
    }
    if !status.success() {
        return Err(format!("{pname} failed ({status})"));
    }
    Ok(text)
}

fn out(cmd: &mut Command) -> R<String> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    let o = cmd.output().map_err(|e| format!("{name}: {e}"))?;
    if !o.status.success() {
        return Err(format!("{name} failed:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn sh(cmd: &mut Command) -> R<()> {
    out(cmd).map(drop)
}

fn ok(cmd: &mut Command) -> bool {
    cmd.stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn io<T>(r: std::io::Result<T>) -> R<T> {
    r.map_err(|e| e.to_string())
}

/// ps rather than kill -0: a server we spawned ourselves lingers as a zombie (state Z)
/// after it exits until we are reaped, and kill -0 would still call that alive.
fn alive(pid: u32) -> bool {
    Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .map(|o| {
            let state = String::from_utf8_lossy(&o.stdout).trim().to_string();
            !state.is_empty() && !state.starts_with('Z')
        })
        .unwrap_or(false)
}

fn live_pid(pidfile: &Path) -> Option<u32> {
    let pid: u32 = fs::read_to_string(pidfile).ok()?.lines().next()?.trim().parse().ok()?;
    alive(pid).then_some(pid)
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port").local_addr().unwrap().port()
}

fn port_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn wait(mut done: impl FnMut() -> bool, secs: u64, what: &str) -> R<()> {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(secs) {
        if done() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("timed out after {secs}s waiting for {what}"))
}

fn wait_port(port: u16, child: &mut Child, log: &Path) -> R<()> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    wait(
        || matches!(child.try_wait(), Ok(Some(_))) || TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok(),
        120,
        "server to accept connections",
    )?;
    if let Ok(Some(status)) = child.try_wait() {
        return Err(format!("server exited ({status}); see {}", log.display()));
    }
    Ok(())
}

// Event triggers log the full client query. ddl_only extracts its DDL before replay;
// row changes in the same batch arrive separately through logical replication.
fn ddl_source_sql(sub: &str, schemas: &str) -> String {
    format!(
        "CREATE SCHEMA {sub};
CREATE TABLE {sub}.ddl (id bigserial PRIMARY KEY, xid xid8 NOT NULL, sql text NOT NULL, search_path text NOT NULL DEFAULT 'public', at timestamptz NOT NULL DEFAULT now(), error text, UNIQUE (xid, sql));
GRANT USAGE ON SCHEMA {sub} TO PUBLIC;
GRANT INSERT ON {sub}.ddl TO PUBLIC;
GRANT USAGE ON SEQUENCE {sub}.ddl_id_seq TO PUBLIC;
CREATE FUNCTION {sub}.log_ddl() RETURNS event_trigger LANGUAGE plpgsql SECURITY DEFINER AS $f$
DECLARE r record;
BEGIN
  -- Each block swallows its own errors: replication bookkeeping must never fail production DDL.
  BEGIN
    -- Replication plumbing (publications, subscriptions, event triggers, our schema) must not be replayed.
    IF NOT EXISTS (SELECT 1 FROM pg_event_trigger_ddl_commands()
                   WHERE command_tag ~ 'PUBLICATION|SUBSCRIPTION|EVENT TRIGGER' OR schema_name = '{sub}') THEN
      INSERT INTO {sub}.ddl (xid, sql, search_path) VALUES (pg_current_xact_id(), current_query(), current_setting('search_path'))
      ON CONFLICT DO NOTHING;
    END IF;
  EXCEPTION WHEN OTHERS THEN NULL;
  END;
  -- New or newly keyed tables join the publication; the replica picks them up on its next refresh.
  FOR r IN SELECT DISTINCT c.oid::regclass AS t FROM pg_event_trigger_ddl_commands() e JOIN pg_class c ON c.oid = e.objid
           WHERE e.command_tag IN ('CREATE TABLE', 'ALTER TABLE') AND c.relkind IN ('r', 'p')
             AND c.relnamespace::regnamespace::text IN ({schemas})
             AND (c.relreplident IN ('f', 'i') OR EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = c.oid AND i.indisprimary))
             AND NOT EXISTS (SELECT 1 FROM pg_publication_rel pr JOIN pg_publication p ON p.oid = pr.prpubid WHERE p.pubname = '{sub}' AND pr.prrelid = c.oid)
  LOOP
    BEGIN EXECUTE format('ALTER PUBLICATION {sub} ADD TABLE %s', r.t); EXCEPTION WHEN OTHERS THEN NULL; END;
  END LOOP;
END $f$;"
    )
}

fn ddl_replica_sql(sub: &str) -> String {
    let lexer = include_str!("ddl.sql").replace("@SCHEMA@", sub);
    format!(
        "{lexer}
DROP EVENT TRIGGER IF EXISTS {sub};
CREATE OR REPLACE FUNCTION {sub}.apply_ddl() RETURNS trigger LANGUAGE plpgsql AS $f$
BEGIN
  -- The apply worker runs with an empty search_path; replay with the one the statement was written under.
  -- CONCURRENTLY cannot run inside the apply transaction; on a replica the plain form is fine.
  BEGIN
    PERFORM set_config('search_path', NEW.search_path, true);
    EXECUTE {sub}.ddl_only(NEW.sql);
  EXCEPTION WHEN OTHERS THEN NEW.error := SQLERRM;
  END;
  RETURN NEW;
END $f$;
DROP TRIGGER IF EXISTS apply_ddl ON {sub}.ddl;
CREATE TRIGGER apply_ddl BEFORE INSERT ON {sub}.ddl FOR EACH ROW EXECUTE FUNCTION {sub}.apply_ddl();
ALTER TABLE {sub}.ddl ENABLE ALWAYS TRIGGER apply_ddl;"
    )
}

/// Make this mongod the sole writable member of its set: initiate a fresh one, or force a
/// clone (which inherited its parent's host:port) onto its own port.
const MONGO_ENSURE_PRIMARY: &str = "
const me = '127.0.0.1:PORT';
const cfg = { _id: 'snapshotdb', version: 1, members: [{ _id: 0, host: me }] };
let status; try { status = rs.status(); } catch (e) { status = e; }
if (status.codeName === 'NotYetInitialized') rs.initiate(cfg);
else if (!(status.members || []).some(m => m.self && m.name === me)) rs.reconfig(cfg, { force: true });
while (!db.hello().isWritablePrimary) sleep(100);";

/// Applies production change events to this replica, forever. Every event is an upsert or
/// delete keyed by _id, so replaying the dump window or a restart overlap is harmless. An
/// event that fails to apply stops the tailer without advancing its checkpoint.
const MONGO_TAIL_JS: &str = "
const fs = require('fs');
const src = new Mongo(process.env.SRC);
const run = process.env.RUN;
const opts = { fullDocument: 'updateLookup', showExpandedEvents: true };
try { opts.resumeAfter = JSON.parse(fs.readFileSync(run + '/token', 'utf8')); } catch (e) {}
const stream = src.watch([], opts);
while (true) {
  const e = stream.tryNext();
  if (!e) { sleep(200); continue; }
  try {
    const dbh = e.ns ? db.getSiblingDB(e.ns.db) : null;
    const coll = e.ns && e.ns.coll ? dbh.getCollection(e.ns.coll) : null;
    const id = e.documentKey ? e.documentKey._id : null;
    const desc = e.operationDescription || {};
    switch (e.operationType) {
      case 'insert': case 'update': case 'replace':
        if (e.fullDocument) coll.replaceOne({ _id: id }, e.fullDocument, { upsert: true });
        else coll.deleteOne({ _id: id });
        break;
      case 'delete': coll.deleteOne({ _id: id }); break;
      case 'drop': coll.drop(); break;
      case 'rename': coll.renameCollection(e.to.coll, true); break;
      case 'dropDatabase': dbh.dropDatabase(); break;
      case 'create': { const { idIndex, ...options } = desc; dbh.createCollection(e.ns.coll, options); break; }
      case 'createIndexes': { const r=dbh.runCommand({ createIndexes: e.ns.coll, indexes: desc.indexes }); if(!r.ok) throw new Error(JSON.stringify(r)); break; }
      case 'dropIndexes': for (const ix of desc.indexes || []) coll.dropIndex(ix.name); break;
      case 'modify': { const r=dbh.runCommand(Object.assign({ collMod: e.ns.coll }, desc)); if(!r.ok) throw new Error(JSON.stringify(r)); break; }
      case 'invalidate': throw new Error('change stream invalidated; resync the source');
    }
  } catch (err) {
    fs.appendFileSync(run + '/tail.errors', JSON.stringify({ at: new Date().toISOString(), op: e.operationType, ns: e.ns, id: e.documentKey, error: String(err) }) + '\\n');
    fs.writeFileSync(run + '/tail.failed', String(err));
    throw err;
  }
  fs.writeFileSync(run + '/token', JSON.stringify(e._id));
  fs.writeFileSync(run + '/applied', (e.wallTime || new Date()).toISOString());
}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_returns_when_the_consumer_exits_without_reading() {
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || { let _ = tx.send(pipe(&mut Command::new("yes"), &mut Command::new("false"))); });
        let result = rx.recv_timeout(Duration::from_secs(10)).expect("pipe deadlocked on an early-exiting consumer");
        assert!(result.unwrap_err().starts_with("false failed"));
    }

    #[test]
    fn public_urls_use_server_host_and_maintenance_stays_on_loopback() {
        let previous = env::var_os("SNAPSHOTDB_PUBLIC_HOST");
        env::set_var("SNAPSHOTDB_PUBLIC_HOST", "branches.internal");
        for engine in [Engine::Postgres, Engine::Mysql, Engine::Mongodb] {
            let branch = Branch { name: "url-only".into(), dir: PathBuf::from("/nonexistent-snapshotdb-url-test"), engine };
            assert!(branch.url_on(54321, true).contains("branches.internal:54321"));
            assert!(branch.url_on(54321, false).contains("127.0.0.1:54321"));
        }
        match previous {
            Some(value) => env::set_var("SNAPSHOTDB_PUBLIC_HOST", value),
            None => env::remove_var("SNAPSHOTDB_PUBLIC_HOST"),
        }
    }

    #[test]
    fn urls_parse_and_redact() {
        let (u, p, h, port) = parse_url("mysql://repl:p%40ss@db.example.com:3307/app?ssl=1").unwrap();
        assert_eq!((u.as_str(), p.as_str(), h.as_str(), port), ("repl", "p@ss", "db.example.com", 3307));
        assert_eq!(parse_url("mysql://root@127.0.0.1/").unwrap().3, 3306);
        assert_eq!(redact("postgresql://u:secret@h:5432/db"), "postgresql://u:***@h:5432/db");
        assert_eq!(redact("postgresql://u@h:5432/db"), "postgresql://u@h:5432/db");
        assert_eq!(js("a\"b\n"), "\"a\\\"b\\n\"");
    }

    #[test]
    fn args_parse() {
        let a = Args::parse(["create", "x", "--from", "main", "--print-url", "--format=json"].map(String::from).to_vec());
        assert_eq!(a.pos, ["create", "x"]);
        assert_eq!(a.flags["from"], "main");
        assert!(a.flag("print-url") && a.json());
        let b = Args::parse(["import", "sqlite", "m", "--new"].map(String::from).to_vec());
        assert_eq!(b.pos, ["import", "sqlite", "m", "--new"]);
    }
}
