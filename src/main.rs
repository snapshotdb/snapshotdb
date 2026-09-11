//! anybranch: copy-on-write branches of any local database, synced from production.
//!
//! A branch is a directory `$ANYBRANCH_HOME/<name>/` holding `engine`, optional `parent`,
//! optional `source` (a production URL this root replicates from), per-root settings
//! (`default_db`, `branch_sql`, `lock`), `data/` (what the engine sees; this is what gets
//! cloned) and `run/` (ports, pids, sockets, logs, tailer state; never cloned).
//!
//! Every server branch has a stable public port owned by a small proxy (proxy.rs) and an
//! engine port behind it. Idle engines are suspended and resumed on the next connection.
//! Cloning is `cp` with the platform's clone flag, so size never matters.
mod preflight;
mod proxy;

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
  anybranch preflight <postgres|mysql|mongodb> <url> [--schemas a,b] [--format json]   check a source; creates nothing (exit 2 on failure)
  anybranch import <postgres|mysql|sqlite|mongodb> <name> <datadir|file|--new>
  anybranch sync   <postgres|mysql|mongodb> <name> <url> [--schemas a,b] [--fix-replica-identity]   root kept in sync with production
  anybranch create <name> --from <parent> [--print-url] [--format json]
  anybranch info   [name] [--print-url] [--format json]      details of a branch (default: current)
  anybranch url    [name]
  anybranch switch <name>                                    make a branch current
  anybranch list   [--format json]
  anybranch status <name> [--format json]                    replication state of a synced root
  anybranch repair <name>                                    skip the transaction that paused replication
  anybranch reset  <name>                                    re-clone from parent
  anybranch settings <root> [set <key> <value> | remove <key>]   keys: default_db, branch_sql (@file or SQL), source
  anybranch lock|unlock <name>                               protect a branch from rm
  anybranch start|stop|rm <name>
env: ANYBRANCH_HOME (default ~/.anybranch)   ANYBRANCH_IDLE_MINUTES (default 5; 0 never suspends)";

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
                    } else if matches!(k, "print-url" | "fix-replica-identity") || it.peek().map_or(true, |n| n.starts_with("--")) {
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
    let a = Args::parse(env::args().skip(1).collect());
    if a.flag("version") {
        println!("anybranch {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if a.flag("help") || a.pos.is_empty() {
        println!("{USAGE}");
        process::exit(if a.flag("help") { 0 } else { 1 });
    }
    let p: Vec<&str> = a.pos.iter().map(String::as_str).collect();
    let json = a.json();
    let current = |name: Option<&&str>| -> R<Branch> {
        match name {
            Some(n) => Branch::load(n),
            None => Branch::load(&fs::read_to_string(home().join(".current")).map_err(|_| "no current branch; pass a name or run: anybranch switch <name>")?),
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
        ["info"] | ["info", _] => current(p.get(1)).and_then(|b| emit(&b, &a, None)),
        ["url"] | ["url", _] => current(p.get(1)).and_then(|b| b.url()).map(|u| println!("{u}")),
        ["switch", name] => Branch::load(name).and_then(|b| b.set_current()).map(|_| println!("switched to {name}")),
        ["list"] => list(json),
        ["status", name] => Branch::load(name).and_then(|b| b.status()).map(|s| print!("{}", render_status(&s, json))),
        ["repair", name] => Branch::load(name).and_then(|b| b.repair()).map(|m| println!("{m}")),
        ["reset", name] => reset(name).and_then(|b| emit(&b, &a, None)),
        ["settings", root] => Branch::load(root).and_then(|b| b.settings_list()).map(|s| print!("{s}")),
        ["settings", root, "set", key, value] => Branch::load(root).and_then(|b| b.set_setting(key, value)),
        ["settings", root, "remove", key] => Branch::load(root).and_then(|b| b.remove_setting(key)),
        ["lock", name] => Branch::load(name).and_then(|b| io(fs::write(b.dir.join("lock"), ""))),
        ["unlock", name] => Branch::load(name).and_then(|b| io(fs::remove_file(b.dir.join("lock")).or(Ok(())))),
        ["start", name] => Branch::load(name).and_then(|b| b.start().and(b.url())).map(|u| println!("{u}")),
        ["stop", name] => Branch::load(name).and_then(|b| b.stop()),
        ["rm", name] => Branch::load(name).and_then(|b| b.rm()),
        ["_proxy", name] => proxy::serve(name),
        _ => Err(USAGE.to_string()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

/// Print a branch the way the caller asked: URL only, JSON, or URL plus a timing line.
fn emit(b: &Branch, a: &Args, started: Option<Instant>) -> R<()> {
    if a.json() {
        println!("{}", b.info_json()?);
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
    env::var_os("ANYBRANCH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").expect("HOME is set")).join(".anybranch"))
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
        if name.is_empty() || name.starts_with(['.', '_']) || name.contains('/') || name.len() > 40 {
            return Err(format!("invalid branch name {name:?}: 1-40 chars, no '/', not starting with '.' or '_'"));
        }
        let b = Branch { name: name.into(), dir: home().join(name), engine };
        if b.dir.exists() {
            return Err(format!("branch {name} already exists"));
        }
        io(fs::create_dir_all(b.run()))?;
        io(fs::write(b.dir.join("engine"), engine.name()))?;
        Ok(b)
    }

    pub fn load(name: &str) -> R<Branch> {
        let dir = home().join(name);
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
            cur = p;
        }
        cur
    }

    fn synced_ancestry(&self) -> bool {
        self.root().source().is_some()
    }

    /// Replication object name (publication, subscription, slot, DDL schema) for this root.
    fn subname(&self) -> String {
        format!("anybranch_{}", self.name.replace(['-', '.'], "_"))
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

    fn set_current(&self) -> R<()> {
        io(fs::write(home().join(".current"), &self.name))
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

    fn proxy_pid(&self) -> Option<u32> {
        live_pid(&self.run().join("proxypid"))
    }

    fn status_word(&self) -> &'static str {
        match (self.engine, self.running(), self.proxy_pid().is_some(), self.source().is_some()) {
            (Engine::Sqlite, ..) => "file",
            (_, true, _, true) => "syncing",
            (_, true, _, false) => "running",
            (_, false, true, _) => "suspended",
            (_, false, false, _) => "stopped",
        }
    }

    /// Public URL through the proxy. Valid while the branch is suspended: connecting resumes it.
    pub fn url(&self) -> R<String> {
        if self.engine == Engine::Sqlite {
            return Ok(format!("sqlite:///{}", self.data().join("db.sqlite").display()));
        }
        let port = self.port().filter(|_| self.proxy_pid().is_some() || self.running());
        let port = port.ok_or_else(|| format!("{0} is stopped; run: anybranch start {0}", self.name))?;
        Ok(self.url_on(port, true))
    }

    /// URL straight to the engine, for anybranch's own maintenance commands.
    fn engine_url(&self) -> R<String> {
        let port = self.eport().filter(|_| self.running()).ok_or_else(|| format!("{} is not running", self.name))?;
        Ok(self.url_on(port, false))
    }

    fn url_on(&self, port: u16, public: bool) -> String {
        let db = self.database();
        match self.engine {
            // ponytail: assumes the cluster's superuser is $USER (initdb's default); set default_db for the database.
            Engine::Postgres => format!("postgresql://{}@127.0.0.1:{port}/{db}", env::var("USER").unwrap_or_default()),
            Engine::Mysql => format!("mysql://root@127.0.0.1:{port}/{db}"),
            // directConnection: drivers must not discover the engine port behind the proxy.
            Engine::Mongodb => format!("mongodb://127.0.0.1:{port}/{db}{}", if public { "?directConnection=true" } else { "" }),
            Engine::Sqlite => unreachable!(),
        }
    }

    /// The mysql client pointed at this branch's socket.
    fn mysql(&self) -> Command {
        let mut c = Command::new("mysql");
        c.args(["--no-defaults", "-u", "root", "-N", "-B", "--protocol=socket"])
            .arg(format!("--socket={}", self.run().join("sock").display()));
        if !self.database().is_empty() {
            c.args(["-D", &self.database()]);
        }
        c
    }

    /// Run SQL (or JavaScript for MongoDB) on this branch's own server; returns stdout.
    fn sql(&self, statement: &str) -> R<String> {
        match self.engine {
            Engine::Postgres => psql(&self.engine_url()?, statement),
            Engine::Mysql => out(self.mysql().arg("-e").arg(statement)),
            Engine::Mongodb => mongosh(&self.engine_url()?, statement),
            Engine::Sqlite => Err("sqlite has no server; open the file directly".into()),
        }
    }

    // --- lifecycle -----------------------------------------------------------------

    /// Create an empty database in data/, cleanly shut down.
    fn init(&self) -> R<()> {
        let data = self.data();
        match self.engine {
            Engine::Postgres => sh(Command::new("initdb").arg("-D").arg(&data)),
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
            if fs::OpenOptions::new().write(true).create_new(true).open(&marker).is_ok() {
                return Ok(Hold(marker));
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
        if self.engine == Engine::Sqlite {
            return Ok(());
        }
        self.ensure_proxy()?;
        self.start_engine()
    }

    fn ensure_proxy(&self) -> R<()> {
        if self.engine == Engine::Sqlite || self.proxy_pid().is_some() {
            return Ok(());
        }
        let run = self.run();
        let port = match self.port() {
            Some(p) if port_free(p) => p,
            Some(p) => {
                let n = free_port();
                eprintln!("warning: port {p} is taken; {} moves to {n}", self.name);
                n
            }
            None => free_port(),
        };
        io(fs::write(run.join("port"), port.to_string()))?;
        let log = io(fs::OpenOptions::new().create(true).append(true).open(run.join("proxy.log")))?;
        let child = Command::new(io(env::current_exe())?)
            .args(["_proxy", &self.name])
            .stdin(Stdio::null())
            .stdout(Stdio::from(io(log.try_clone())?))
            .stderr(Stdio::from(log))
            .process_group(0)
            .spawn()
            .map_err(|e| format!("spawn proxy: {e}"))?;
        io(fs::write(run.join("proxypid"), child.id().to_string()))?;
        wait(|| !port_free(port), 10, "proxy to listen")
    }

    /// Start the engine behind the proxy (also what the proxy calls to resume).
    pub fn start_engine(&self) -> R<()> {
        if self.engine == Engine::Sqlite || self.running() {
            return Ok(());
        }
        let _hold = self.hold()?;
        if self.running() {
            return Ok(()); // someone else started it while we waited
        }
        let (data, run) = (self.data(), self.run());
        let port = free_port();
        io(fs::write(run.join("eport"), port.to_string()))?;
        let clone = self.parent().is_some();
        match self.engine {
            Engine::Postgres => {
                // wal_level=logical lets any branch serve as a replication source. Clones get no
                // apply workers, so an inherited subscription can never race the root for its slot.
                let mut opts = format!(
                    "-c listen_addresses=127.0.0.1 -c port={port} -c unix_socket_directories='{}' -c wal_level=logical",
                    run.display()
                );
                if clone {
                    opts += " -c max_logical_replication_workers=0";
                }
                sh(Command::new("pg_ctl").arg("-D").arg(&data).arg("-l").arg(run.join("log")).args(["-w", "-o", &opts, "start"]))?
            }
            Engine::Mongodb => {
                // Every mongod is a single-node replica set, so change streams and transactions
                // work and a synced root can be tailed. Spawned detached: 8.3 dropped --fork on macOS.
                let mut child = Command::new("mongod")
                    .arg("--dbpath").arg(&data)
                    .args(["--port", &port.to_string(), "--bind_ip", "127.0.0.1", "--nounixsocket", "--logappend", "--replSet", "anybranch"])
                    .arg("--logpath").arg(run.join("log"))
                    .arg("--pidfilepath").arg(run.join("pid"))
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                    .process_group(0)
                    .spawn()
                    .map_err(|e| format!("mongod: {e}"))?;
                wait_port(port, &mut child, &run.join("log"))?;
                mongosh(&format!("mongodb://127.0.0.1:{port}/"), &MONGO_ENSURE_PRIMARY.replace("PORT", &port.to_string()))?;
                if self.source().is_some() {
                    self.spawn_tail()?;
                }
            }
            Engine::Mysql => {
                let mut cmd = Command::new("mysqld");
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
                    cmd.arg("--skip-replica-start");
                }
                if self.source().is_some() {
                    cmd.args(["--replicate-ignore-db=mysql", "--replicate-ignore-db=sys"]);
                }
                let mut child = cmd
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                    .process_group(0)
                    .spawn()
                    .map_err(|e| format!("mysqld: {e}"))?;
                wait_port(port, &mut child, &run.join("log"))?
            }
            Engine::Sqlite => unreachable!(),
        }
        if clone && !run.join("detached").exists() {
            self.detach()?;
            io(fs::write(run.join("detached"), ""))?;
        }
        if clone && !run.join("branch_sql.done").exists() {
            if let Some(script) = self.setting("branch_sql") {
                if let Err(e) = self.sql(&script) {
                    eprintln!("warning: branch_sql failed on {}: {e}", self.name);
                }
            }
            io(fs::write(run.join("branch_sql.done"), ""))?;
        }
        Ok(())
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
    fn refresh(&self) {
        if self.engine == Engine::Postgres && self.source().is_some() && self.running() {
            let _ = self.sql(&format!("ALTER SUBSCRIPTION {} REFRESH PUBLICATION", self.subname()));
        }
    }

    /// Long-running mongosh that applies production change events to this replica.
    fn spawn_tail(&self) -> R<()> {
        let run = self.run();
        if live_pid(&run.join("tailpid")).is_some() {
            return Ok(());
        }
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
        if self.dir.join("lock").exists() {
            return Err(format!("{0} is locked; run: anybranch unlock {0}", self.name));
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
        self.stop()?;
        if let Some(pid) = self.proxy_pid() {
            let _ = sh(Command::new("kill").args(["-TERM", &pid.to_string()]));
        }
        if self.is_current() {
            let _ = fs::remove_file(home().join(".current"));
        }
        io(fs::remove_dir_all(&self.dir))
    }

    // --- introspection -------------------------------------------------------------

    fn info_json(&self) -> R<String> {
        Ok(format!(
            "{{\"name\":{},\"engine\":{},\"parent\":{},\"status\":{},\"url\":{},\"port\":{},\"synced\":{},\"locked\":{},\"current\":{}}}",
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
                let enabled = self.sql(&format!("SELECT subenabled FROM pg_subscription WHERE subname = '{sub}'"))?;
                if enabled == "f" {
                    let (err, lsn) = self.last_apply_error();
                    lines.push(("stream", format!("PAUSED after an apply error: {err}")));
                    lines.push(("repair", format!("anybranch repair {} skips that transaction{}", self.name, lsn.map(|l| format!(" (at {l})")).unwrap_or_default())));
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
                    "not tracked (no event trigger on the source; rm and sync again after migrations)".into()
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
                    lines.push(("repair", format!("anybranch repair {} skips the failing transaction", self.name)));
                }
            }
            Engine::Mongodb => {
                let run = self.run();
                let tailing = live_pid(&run.join("tailpid")).is_some();
                let applied = fs::read_to_string(run.join("applied")).unwrap_or_else(|_| "nothing yet".into());
                let failed = fs::read_to_string(run.join("tail.errors")).map(|s| s.lines().count()).unwrap_or(0);
                lines.push(("stream", format!("{}, last event applied {applied}", if tailing { "tailing" } else { "stopped (anybranch repair restarts it; see run/tail.log)" })));
                lines.push(("skipped events", format!("{failed}{}", if failed > 0 { " (see run/tail.errors)" } else { "" })));
            }
            Engine::Sqlite => {}
        }
        Ok(lines)
    }

    /// Last apply-worker error in the Postgres log and the LSN that ends the failing transaction.
    fn last_apply_error(&self) -> (String, Option<String>) {
        let log = fs::read_to_string(self.run().join("log")).unwrap_or_default();
        let err = log.lines().rev().find(|l| l.contains("ERROR:")).map(|l| l.split("ERROR:").nth(1).unwrap_or("").trim().to_string()).unwrap_or_else(|| "unknown".into());
        let lsn = log.lines().rev().find_map(|l| l.split("finished at ").nth(1)).map(|s| s.trim().to_string());
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
                if self.sql(&format!("SELECT subenabled FROM pg_subscription WHERE subname = '{sub}'"))? == "t" {
                    return Ok("replication is not paused; nothing to repair".into());
                }
                let (err, lsn) = self.last_apply_error();
                let lsn = lsn.ok_or("paused, but no failing transaction found in run/log; check the log and ALTER SUBSCRIPTION ... ENABLE by hand")?;
                self.sql(&format!("ALTER SUBSCRIPTION {sub} SKIP (lsn = '{lsn}')"))?;
                self.sql(&format!("ALTER SUBSCRIPTION {sub} ENABLE"))?;
                Ok(format!("skipped the transaction finishing at {lsn} ({err}) and resumed"))
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
                Ok("tailer restarted from its last resume token; failed events are logged in run/tail.errors and skipped".into())
            }
            Engine::Sqlite => Err("sqlite does not replicate".into()),
        }
    }

    // --- settings ------------------------------------------------------------------

    fn settings_list(&self) -> R<String> {
        let mut s = String::new();
        for key in ["default_db", "branch_sql", "source"] {
            let v = match (key, self.setting(key)) {
                (_, None) => "(unset)".to_string(),
                ("source", Some(u)) => redact(&u),
                ("branch_sql", Some(sql)) => format!("{} bytes, {} lines", sql.len(), sql.lines().count()),
                (_, Some(v)) => v,
            };
            s += &format!("{key:<11} {v}\n");
        }
        s += &format!("{:<11} {}\n", "lock", if self.dir.join("lock").exists() { "locked" } else { "unlocked" });
        Ok(s)
    }

    fn set_setting(&self, key: &str, value: &str) -> R<()> {
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
                io(fs::write(self.dir.join(key), sql))
            }
            "source" => self.set_source(value),
            _ => Err(format!("unknown setting {key:?}; keys: default_db, branch_sql, source")),
        }
    }

    fn remove_setting(&self, key: &str) -> R<()> {
        match key {
            "default_db" | "branch_sql" => io(fs::remove_file(self.dir.join(key)).or(Ok(()))),
            _ => Err(format!("cannot remove {key:?}; only default_db and branch_sql")),
        }
    }

    /// Rotate the production URL (credentials, host) without re-syncing.
    fn set_source(&self, url: &str) -> R<()> {
        self.source().ok_or_else(|| format!("{} is not a synced root", self.name))?;
        self.start()?;
        match self.engine {
            Engine::Postgres => self.sql(&format!("ALTER SUBSCRIPTION {} CONNECTION '{}'", self.subname(), url.replace('\'', "''")))?,
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
            let list = schemas.split(',').map(|s| format!("'{}'", s.trim())).collect::<Vec<_>>().join(",");
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
                .and_then(|_| b.init())
                .and_then(|_| b.start())
                .and_then(|_| {
                    if db != "postgres" {
                        psql(&b.url_on(b.eport().unwrap_or_default(), false).replace(&format!("/{db}"), "/postgres"), &format!("CREATE DATABASE \"{db}\""))?;
                    }
                    // Roles first so GRANTs in the schema dump resolve; roles that already exist error harmlessly.
                    let _ = pipe(
                        Command::new("pg_dumpall").args(["--roles-only", "--no-role-passwords", "-d", url]),
                        Command::new("psql").arg(b.engine_url()?).args(["-X", "-q"]),
                    );
                    // The dump recreates the schemas it covers; the fresh database's own `public` would collide.
                    b.sql("DROP SCHEMA IF EXISTS public CASCADE")?;
                    let mut dump = Command::new("pg_dump");
                    dump.args(["--schema-only", "--no-owner", "--no-publications", "--no-subscriptions", "-d", url]);
                    for schema in schemas.split(',').chain(ddl.then_some(sub.as_str())) {
                        dump.args(["-n", schema.trim()]);
                    }
                    let warnings = pipe(&mut dump, Command::new("psql").arg(b.engine_url()?).args(["-X", "-q", "-o", "/dev/null"]))?;
                    if !warnings.trim().is_empty() {
                        io(fs::write(b.run().join("schema.log"), &warnings))?;
                        eprintln!("schema load warnings (usually extensions or roles missing locally; saved to run/schema.log):\n{warnings}");
                    }
                    let mut published = tables.clone();
                    if ddl {
                        b.sql(&ddl_replica_sql(&sub))?;
                        published += &format!(", {sub}.ddl");
                    }
                    psql(url, &format!("DROP PUBLICATION IF EXISTS {sub}"))?;
                    psql(url, &format!("CREATE PUBLICATION {sub} FOR TABLE {published}"))?;
                    if ddl {
                        // Last, so our own publication statements are not logged and replayed.
                        if let Err(e) = psql(url, &format!("CREATE EVENT TRIGGER {sub} ON ddl_command_end EXECUTE FUNCTION {sub}.log_ddl()")) {
                            eprintln!("schema changes will not replicate (an event trigger on the source needs superuser): {e}\n  after production migrations: anybranch rm {name} and sync again");
                        }
                    }
                    // disable_on_error: a poisoned transaction pauses the stream instead of retrying forever;
                    // `status` shows it and `repair` skips it.
                    b.sql(&format!(
                        "CREATE SUBSCRIPTION {sub} CONNECTION '{}' PUBLICATION {sub} WITH (disable_on_error = true)",
                        url.replace('\'', "''")
                    ))
                });
            if let Err(e) = result {
                let _ = psql(url, &teardown);
                let _ = b.rm_quiet();
                return Err(e);
            }
            eprintln!("{name} replicates {} tables from the source; initial copy continues in the background ({:.1}s so far). Check: anybranch status {name}", tables.matches(", ").count() + 1, started.elapsed().as_secs_f64());
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
            eprintln!("{name} replicates [{dbs}] from the source ({:.1}s). Check: anybranch status {name}", started.elapsed().as_secs_f64());
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
                        Command::new("mongorestore").args(["--uri", &b.url_on(b.eport().unwrap_or_default(), false), "--archive", "--drop", "--quiet", "--nsExclude", "admin.*", "--nsExclude", "config.*"]),
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
            eprintln!("{name} replicates from the source via change streams ({:.1}s). Check: anybranch status {name}", started.elapsed().as_secs_f64());
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
    let b = Branch::new(name, p.engine)?;
    let was_running = p.running();
    p.refresh();
    let cloned = {
        // Hold the parent so its proxy cannot wake the engine while the files are cloned.
        let _hold = p.hold()?;
        if was_running {
            p.stop()?;
        }
        clone(&p.data(), &b.data())
    };
    if was_running {
        p.start()?;
    }
    io(fs::write(b.dir.join("parent"), parent))?;
    let b = finish(b, cloned)?;
    b.set_current()?;
    Ok(b)
}

fn reset(name: &str) -> R<Branch> {
    let b = Branch::load(name)?;
    let parent = b.parent().ok_or_else(|| format!("{name} is a root; only branches with a parent can be reset"))?;
    b.rm()?;
    create(name, &parent)
}

/// Start the new branch, or remove the half-made directory.
fn finish(b: Branch, result: R<()>) -> R<Branch> {
    if let Err(e) = result.and_then(|_| b.start()) {
        let _ = b.rm_quiet();
        return Err(e);
    }
    Ok(b)
}

fn list(json: bool) -> R<()> {
    let Ok(entries) = fs::read_dir(home()) else { return Ok(()) };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    let branches: Vec<Branch> = names.iter().filter_map(|n| Branch::load(n).ok()).collect();
    if json {
        let items: R<Vec<String>> = branches.iter().map(|b| b.info_json()).collect();
        println!("[{}]", items?.join(","));
        return Ok(());
    }
    for b in &branches {
        println!(
            "{}{}\t{}\t{}\t{}\t{}",
            if b.is_current() { "* " } else { "  " },
            b.name,
            b.engine.name(),
            b.parent().unwrap_or_else(|| "-".into()),
            b.status_word(),
            b.url().unwrap_or_default()
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
    match (url.split_once("://"), url.rsplit_once('@')) {
        (Some((scheme, rest)), Some((_, host))) if rest.contains('@') => {
            let user = rest.split(['@', ':']).next().unwrap_or("");
            format!("{scheme}://{user}:***@{host}")
        }
        _ => url.to_string(),
    }
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
    let c = consumer.stdin(p.stdout.take().unwrap()).output().map_err(|e| format!("{cname}: {e}"))?;
    let status = p.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("{pname} failed ({status})"));
    }
    let text = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    if !c.status.success() {
        return Err(format!("{cname} failed:\n{text}"));
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

// ponytail: replays current_query(), the whole client string. Migration tools send one
// statement per query, so this is exact for them; a hand-written `psql -c "ddl; dml"` batch
// would replay its DML too and can collide with the row stream.
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
    format!(
        "DROP EVENT TRIGGER IF EXISTS {sub};
CREATE FUNCTION {sub}.apply_ddl() RETURNS trigger LANGUAGE plpgsql AS $f$
BEGIN
  -- The apply worker runs with an empty search_path; replay with the one the statement was written under.
  BEGIN
    PERFORM set_config('search_path', NEW.search_path, true);
    EXECUTE NEW.sql;
  EXCEPTION WHEN OTHERS THEN NEW.error := SQLERRM;
  END;
  RETURN NEW;
END $f$;
CREATE TRIGGER apply_ddl BEFORE INSERT ON {sub}.ddl FOR EACH ROW EXECUTE FUNCTION {sub}.apply_ddl();
ALTER TABLE {sub}.ddl ENABLE ALWAYS TRIGGER apply_ddl;"
    )
}

/// Make this mongod the sole writable member of its set: initiate a fresh one, or force a
/// clone (which inherited its parent's host:port) onto its own port.
const MONGO_ENSURE_PRIMARY: &str = "
const me = '127.0.0.1:PORT';
const cfg = { _id: 'anybranch', version: 1, members: [{ _id: 0, host: me }] };
let status; try { status = rs.status(); } catch (e) { status = e; }
if (status.codeName === 'NotYetInitialized') rs.initiate(cfg);
else if (!(status.members || []).some(m => m.self && m.name === me)) rs.reconfig(cfg, { force: true });
while (!db.hello().isWritablePrimary) sleep(100);";

/// Applies production change events to this replica, forever. Every event is an upsert or
/// delete keyed by _id, so replaying the dump window or a restart overlap is harmless. An
/// event that fails to apply is logged to run/tail.errors and skipped, never fatal.
const MONGO_TAIL_JS: &str = "
const fs = require('fs');
const src = new Mongo(process.env.SRC);
const run = process.env.RUN;
const opts = { fullDocument: 'updateLookup' };
try { opts.resumeAfter = JSON.parse(fs.readFileSync(run + '/token', 'utf8')); } catch (e) {}
const stream = src.watch([], opts);
while (true) {
  const e = stream.tryNext();
  if (!e) { sleep(200); continue; }
  try {
    const coll = e.ns && e.ns.coll ? db.getSiblingDB(e.ns.db).getCollection(e.ns.coll) : null;
    const id = e.documentKey ? e.documentKey._id : null;
    switch (e.operationType) {
      case 'insert': case 'update': case 'replace':
        if (e.fullDocument) coll.replaceOne({ _id: id }, e.fullDocument, { upsert: true });
        else coll.deleteOne({ _id: id });
        break;
      case 'delete': coll.deleteOne({ _id: id }); break;
      case 'drop': coll.drop(); break;
      case 'rename': coll.renameCollection(e.to.coll, true); break;
      case 'dropDatabase': db.getSiblingDB(e.ns.db).dropDatabase(); break;
    }
  } catch (err) {
    fs.appendFileSync(run + '/tail.errors', JSON.stringify({ at: new Date().toISOString(), op: e.operationType, ns: e.ns, id: e.documentKey, error: String(err) }) + '\\n');
  }
  fs.writeFileSync(run + '/token', JSON.stringify(e._id));
  fs.writeFileSync(run + '/applied', (e.wallTime || new Date()).toISOString());
}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_branches_are_isolated() {
        let tmp = env::temp_dir().join(format!("anybranch-test-{}", process::id()));
        env::set_var("ANYBRANCH_HOME", &tmp);
        let q = |branch: &str, sql: &str| {
            let out = Command::new("sqlite3").arg(home().join(branch).join("data/db.sqlite")).arg(sql).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        import("sqlite", "main", "--new").unwrap();
        q("main", "create table t(x); insert into t values(1)");
        create("dev", "main").unwrap();
        q("dev", "insert into t values(2)");
        assert_eq!(q("main", "select count(*) from t"), "1");
        assert_eq!(q("dev", "select count(*) from t"), "2");
        assert!(create("dev", "main").is_err(), "duplicate names must be refused");
        assert!(Branch::load("dev").unwrap().is_current());
        for name in ["dev", "main"] {
            Branch::load(name).unwrap().rm().unwrap();
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn urls_parse_and_redact() {
        let (u, p, h, port) = parse_url("mysql://repl:p%40ss@db.example.com:3307/app?ssl=1").unwrap();
        assert_eq!((u.as_str(), p.as_str(), h.as_str(), port), ("repl", "p@ss", "db.example.com", 3307));
        assert_eq!(parse_url("mysql://root@127.0.0.1/").unwrap().3, 3306);
        assert_eq!(redact("postgresql://u:secret@h:5432/db"), "postgresql://u:***@h:5432/db");
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
