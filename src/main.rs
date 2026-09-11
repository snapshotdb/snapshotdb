//! anybranch: copy-on-write branches of any local database.
//!
//! A branch is a directory `$ANYBRANCH_HOME/<name>/` holding `engine`, optional
//! `parent`, optional `source` (a production URL this root replicates from),
//! `data/` (what the engine sees; this is what gets cloned) and `run/` (port, pid,
//! socket, log; never cloned). Cloning is `cp` with the platform's clone flag, so a
//! 50 GiB branch costs the same as a 50 MiB one.
use std::{
    env, fs,
    net::{TcpListener, TcpStream},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{self, Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const USAGE: &str = "usage:
  anybranch import <postgres|mysql|sqlite|mongodb> <name> <datadir|file|--new>
  anybranch sync   <postgres|mysql> <name> <production-url> [schema,schema]   root kept in sync with production
  anybranch create <name> --from <parent>
  anybranch reset  <name>          re-clone from parent
  anybranch status <name>          replication state of a synced root
  anybranch list
  anybranch url|start|stop|rm <name>";

type R<T> = Result<T, String>;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match a.as_slice() {
        ["import", engine, name, source] => import(engine, name, source),
        ["sync", engine, name, url] => sync(engine, name, url, "public"),
        ["sync", engine, name, url, schemas] => sync(engine, name, url, schemas),
        ["create", name, "--from", parent] => create(name, parent),
        ["reset", name] => reset(name),
        ["status", name] => Branch::load(name).and_then(|b| b.status()),
        ["list"] => list(),
        ["url", name] => Branch::load(name).and_then(|b| b.url()).map(|u| println!("{u}")),
        ["start", name] => Branch::load(name).and_then(|b| b.start().and(b.url())).map(|u| println!("{u}")),
        ["stop", name] => Branch::load(name).and_then(|b| b.stop()),
        ["rm", name] => Branch::load(name).and_then(|b| b.rm()),
        _ => Err(USAGE.to_string()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Engine {
    Postgres,
    Mysql,
    Sqlite,
    Mongodb,
}

impl Engine {
    fn parse(s: &str) -> R<Engine> {
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
            // mysqld leaves no lock file; look for a process pointed at this directory.
            Engine::Mysql => ok(Command::new("pgrep").arg("-f").arg(format!("--datadir={}", data.display()))),
            // mongod truncates mongod.lock to zero bytes on clean shutdown.
            Engine::Mongodb => fs::metadata(data.join("mongod.lock")).map(|m| m.len() > 0).unwrap_or(false),
            // ponytail: SQLite is cloned with its -wal; readers recover committed frames on open.
            Engine::Sqlite => false,
        };
        if busy {
            return Err(format!("a {} server is using {}; stop it first so the clone is consistent", self.name(), data.display()));
        }
        Ok(())
    }
}

struct Branch {
    name: String,
    dir: PathBuf,
    engine: Engine,
}

fn home() -> PathBuf {
    env::var_os("ANYBRANCH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").expect("HOME is set")).join(".anybranch"))
}

impl Branch {
    fn new(name: &str, engine: Engine) -> R<Branch> {
        if name.is_empty() || name.starts_with('.') || name.contains('/') {
            return Err(format!("invalid branch name {name:?}"));
        }
        let b = Branch { name: name.into(), dir: home().join(name), engine };
        if b.dir.exists() {
            return Err(format!("branch {name} already exists"));
        }
        io(fs::create_dir_all(b.run()))?;
        io(fs::write(b.dir.join("engine"), engine.name()))?;
        Ok(b)
    }

    fn load(name: &str) -> R<Branch> {
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
    fn source(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("source")).ok()
    }

    /// Does any ancestor replicate from production?
    fn synced_ancestry(&self) -> bool {
        let mut cur = self.parent();
        while let Some(name) = cur {
            match Branch::load(&name) {
                Ok(b) if b.source().is_some() => return true,
                Ok(b) => cur = b.parent(),
                Err(_) => return false,
            }
        }
        false
    }

    /// Replication object name (publication, subscription, slot) for this root.
    fn subname(&self) -> String {
        format!("anybranch_{}", self.name.replace(['-', '.'], "_"))
    }

    fn pid(&self) -> Option<u32> {
        match self.engine {
            Engine::Postgres => live_pid(&self.data().join("postmaster.pid")),
            Engine::Sqlite => None,
            _ => live_pid(&self.run().join("pid")),
        }
    }

    fn running(&self) -> bool {
        self.pid().is_some()
    }

    fn port(&self) -> Option<u16> {
        fs::read_to_string(self.run().join("port")).ok()?.trim().parse().ok()
    }

    fn url(&self) -> R<String> {
        if self.engine == Engine::Sqlite {
            return Ok(format!("sqlite:///{}", self.data().join("db.sqlite").display()));
        }
        let port = self.port().filter(|_| self.running());
        let port = port.ok_or_else(|| format!("{0} is stopped; run: anybranch start {0}", self.name))?;
        Ok(match self.engine {
            // ponytail: assumes the cluster's superuser is $USER (initdb's default); pass your own otherwise.
            Engine::Postgres => format!("postgresql://{}@127.0.0.1:{port}/postgres", env::var("USER").unwrap_or_default()),
            Engine::Mysql => format!("mysql://root@127.0.0.1:{port}/"),
            Engine::Mongodb => format!("mongodb://127.0.0.1:{port}/"),
            Engine::Sqlite => unreachable!(),
        })
    }

    /// The mysql client pointed at this branch's socket.
    fn mysql(&self) -> Command {
        let mut c = Command::new("mysql");
        c.args(["--no-defaults", "-u", "root", "-N", "-B", "--protocol=socket"])
            .arg(format!("--socket={}", self.run().join("sock").display()));
        c
    }

    /// Run SQL on this branch's own server; returns stdout.
    fn sql(&self, statement: &str) -> R<String> {
        match self.engine {
            Engine::Postgres => psql(&self.url()?, statement),
            Engine::Mysql => out(self.mysql().arg("-e").arg(statement)),
            _ => Err(format!("sql is not supported for {}", self.engine.name())),
        }
    }

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
            Engine::Mongodb => io(fs::create_dir(&data)).and_then(|_| self.start()).and_then(|_| self.stop()),
        }
    }

    fn start(&self) -> R<()> {
        if self.engine == Engine::Sqlite || self.running() {
            return Ok(());
        }
        let (data, run) = (self.data(), self.run());
        let port = self.port().filter(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok()).unwrap_or_else(free_port);
        io(fs::write(run.join("port"), port.to_string()))?;
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
                // work and a synced root can be tailed. A clone wakes up with its parent's port
                // in the set config; ensure_primary reconfigures it to its own.
                // Spawned detached like mysqld: mongod 8.3 dropped --fork on macOS.
                let mut child = Command::new("mongod")
                    .arg("--dbpath").arg(&data)
                    .args(["--port", &port.to_string(), "--bind_ip", "127.0.0.1", "--nounixsocket", "--logappend"])
                    .args(["--replSet", "anybranch"])
                    .arg("--logpath").arg(run.join("log"))
                    .arg("--pidfilepath").arg(run.join("pid"))
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                    .process_group(0)
                    .spawn()
                    .map_err(|e| format!("mongod: {e}"))?;
                wait_port(port, &mut child, &run.join("log"))?;
                mongosh(&self.url()?, &MONGO_ENSURE_PRIMARY.replace("PORT", &port.to_string()))?;
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
                    // Managed providers write heartbeats into `mysql`; we never mirror system schemas.
                    cmd.args(["--replicate-ignore-db=mysql", "--replicate-ignore-db=sys"]);
                }
                let mut child = cmd
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
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
        Ok(())
    }

    /// A fresh clone inherits its parent's replication config. Cut it loose so it never
    /// pulls from production, then move sequences past the replicated rows so inserts on
    /// the branch do not collide (logical replication does not carry sequence values).
    fn detach(&self) -> R<()> {
        match self.engine {
            Engine::Postgres => {
                self.sql(
                    "DO $$ DECLARE s name; BEGIN FOR s IN SELECT subname FROM pg_subscription LOOP \
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
            .args(["--quiet", &self.url()?, "--file"])
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

    fn stop(&self) -> R<()> {
        if let Some(pid) = live_pid(&self.run().join("tailpid")) {
            let _ = sh(Command::new("kill").args(["-TERM", &pid.to_string()]));
        }
        let Some(pid) = self.pid() else { return Ok(()) };
        match self.engine {
            Engine::Postgres => sh(Command::new("pg_ctl").arg("-D").arg(self.data()).args(["-m", "fast", "-w", "stop"])),
            // SIGTERM is the clean shutdown for both mysqld and mongod.
            _ => sh(Command::new("kill").args(["-TERM", &pid.to_string()]))
                .and_then(|_| wait(|| !alive(pid), 180, "server to shut down")),
        }
    }

    fn rm(&self) -> R<()> {
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
        io(fs::remove_dir_all(&self.dir))
    }

    fn status(&self) -> R<()> {
        let url = self.source().ok_or_else(|| format!("{} is not a synced root", self.name))?;
        if !self.running() {
            println!("stopped");
            return Ok(());
        }
        match self.engine {
            Engine::Postgres => {
                self.refresh();
                let tables = self.sql(
                    "SELECT coalesce(string_agg(n || ' ' || CASE s WHEN 'r' THEN 'ready' WHEN 's' THEN 'synced' \
                     WHEN 'd' THEN 'copying' WHEN 'f' THEN 'finishing' ELSE 'queued' END, ', ' ORDER BY s), 'none') \
                     FROM (SELECT srsubstate s, count(*) n FROM pg_subscription_rel GROUP BY 1) x",
                )?;
                let stream = self.sql(
                    "SELECT coalesce(latest_end_lsn::text, '0/0') || ' ' || coalesce(extract(epoch FROM now() - last_msg_receipt_time)::int::text, '?') \
                     FROM pg_stat_subscription WHERE relid IS NULL AND pid IS NOT NULL",
                )?;
                match stream.split_once(' ') {
                    None => println!("tables: {tables}\nstream: disconnected (see {}/log)", self.run().display()),
                    Some((lsn, age)) => {
                        let behind = psql(&url, &format!("SELECT pg_size_pretty(pg_wal_lsn_diff(pg_current_wal_lsn(), '{lsn}'))"))
                            .unwrap_or_else(|_| "unknown".into());
                        println!("tables: {tables}\nstream: connected, {behind} behind source, last message {age}s ago");
                    }
                }
                let sub = self.subname();
                let tracked = psql(&url, &format!("SELECT count(*) FROM pg_event_trigger WHERE evtname = '{sub}'")).unwrap_or_default() == "1";
                if tracked && self.sql(&format!("SELECT to_regclass('{sub}.ddl') IS NOT NULL"))? == "t" {
                    let ddl = self.sql(&format!(
                        "SELECT count(*) FILTER (WHERE error IS NULL) || ' applied, ' || count(*) FILTER (WHERE error IS NOT NULL) || ' failed' \
                         || coalesce(' (last: ' || (SELECT error FROM {sub}.ddl WHERE error IS NOT NULL ORDER BY id DESC LIMIT 1) || ')', '') \
                         FROM {sub}.ddl"
                    ))?;
                    println!("schema changes: {ddl}");
                } else {
                    println!("schema changes: not tracked (no event trigger on the source; rm and sync again after migrations)");
                }
            }
            Engine::Mongodb => {
                let run = self.run();
                let tailing = live_pid(&run.join("tailpid")).is_some();
                let applied = fs::read_to_string(run.join("applied")).unwrap_or_else(|_| "nothing yet".into());
                println!("stream: {}, last event applied {applied}", if tailing { "tailing" } else { "stopped (see run/tail.log)" });
            }
            Engine::Mysql => {
                let threads = self.sql(
                    "SELECT concat('io ', c.SERVICE_STATE, ', sql ', a.SERVICE_STATE, \
                     coalesce(concat(', io error: ', nullif(c.LAST_ERROR_MESSAGE, '')), ''), \
                     coalesce(concat(', sql error: ', nullif(k.LAST_ERROR_MESSAGE, '')), '')) \
                     FROM performance_schema.replication_connection_status c \
                     JOIN performance_schema.replication_applier_status a USING (CHANNEL_NAME) \
                     LEFT JOIN performance_schema.replication_applier_status_by_coordinator k USING (CHANNEL_NAME)",
                )?;
                // Transactions the source has committed that we have not applied yet.
                let (user, pass, host, port) = parse_url(&url)?;
                let pending = mysql_on(&host, port, &user, &pass, "SELECT @@gtid_executed")
                    .and_then(|src| self.sql(&format!("SELECT GTID_SUBTRACT('{src}', @@gtid_executed)")))
                    .map(|gap| if gap.is_empty() { "caught up".to_string() } else { format!("behind by {gap}") })
                    .unwrap_or_else(|_| "lag unknown (source unreachable)".into());
                println!("stream: {}, {pending}", if threads.is_empty() { "not configured".to_string() } else { threads });
            }
            _ => {}
        }
        Ok(())
    }
}

fn import(engine: &str, name: &str, source: &str) -> R<()> {
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
    finish(b, result, None)
}

/// Make a root that continuously replicates from a production database, using the
/// engine's own replication (Postgres logical replication, MySQL GTID replication).
/// Nothing is created on the source until the preflight passes; on the source we only
/// add a publication and slot (Postgres) or read the binlog (MySQL).
fn sync(engine: &str, name: &str, url: &str, schemas: &str) -> R<()> {
    let engine = Engine::parse(engine)?;
    let started = Instant::now();
    match engine {
        Engine::Postgres => {
            let info = psql(url, "SELECT current_setting('wal_level'), pg_is_in_recovery()")?;
            let (wal_level, in_recovery) = info.split_once('|').unwrap_or((&info, ""));
            if wal_level != "logical" {
                return Err(format!(
                    "source wal_level is {wal_level}, needs logical. RDS: set rds.logical_replication=1 and reboot; \
                     self-hosted: wal_level=logical and restart"
                ));
            }
            if in_recovery == "t" {
                return Err("source is a read replica (pg_is_in_recovery); point at the writable primary".into());
            }
            let list = schemas.split(',').map(|s| format!("'{s}'")).collect::<Vec<_>>().join(",");
            let replicable = format!(
                "c.relkind IN ('r', 'p') AND n.nspname IN ({list}) AND (c.relreplident IN ('f', 'i') \
                 OR EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = c.oid AND i.indisprimary))"
            );
            let from = "FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE";
            // Publishing a table without a replica identity makes UPDATE/DELETE fail on production
            // itself, so such tables are left out and the fix is printed instead of applied.
            let tables = psql(url, &format!("SELECT string_agg(format('%I.%I', n.nspname, c.relname), ', ') {from} {replicable}"))?;
            let skipped = psql(url, &format!(
                "SELECT string_agg(format('ALTER TABLE %I.%I REPLICA IDENTITY FULL;', n.nspname, c.relname), ' ') \
                 {from} c.relkind IN ('r', 'p') AND n.nspname IN ({list}) AND NOT ({replicable})"
            ))?;
            if tables.is_empty() {
                return Err(format!("no replicable tables in {schemas}: each needs a PRIMARY KEY or REPLICA IDENTITY FULL"));
            }
            if !skipped.is_empty() {
                eprintln!("skipping tables without a primary key; to include them run this on the source, then rm and sync again:\n  {skipped}");
            }
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
            let result = write_secret(&b.dir.join("source"), url).and_then(|_| b.init()).and_then(|_| b.start()).and_then(|_| {
                // Roles first so GRANTs in the schema dump resolve; roles that already exist error harmlessly.
                let _ = pipe(
                    Command::new("pg_dumpall").args(["--roles-only", "--no-role-passwords", "-d", url]),
                    Command::new("psql").arg(b.url()?).args(["-X", "-q"]),
                );
                // The dump recreates the schemas it covers; the fresh cluster's own `public` would collide.
                b.sql("DROP SCHEMA IF EXISTS public CASCADE")?;
                let mut dump = Command::new("pg_dump");
                dump.args(["--schema-only", "--no-owner", "--no-publications", "--no-subscriptions", "-d", url]);
                for schema in schemas.split(',').chain(ddl.then_some(sub.as_str())) {
                    dump.args(["-n", schema]);
                }
                let warnings = pipe(&mut dump, Command::new("psql").arg(b.url()?).args(["-X", "-q", "-o", "/dev/null"]))?;
                if !warnings.trim().is_empty() {
                    eprintln!("schema load warnings (usually extensions or roles missing locally):\n{warnings}");
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
                b.sql(&format!("CREATE SUBSCRIPTION {sub} CONNECTION '{}' PUBLICATION {sub}", url.replace('\'', "''")))
            });
            if let Err(e) = result {
                let _ = psql(url, &teardown);
                let _ = b.stop();
                let _ = fs::remove_dir_all(&b.dir);
                return Err(e);
            }
            println!("{}", b.url()?);
            eprintln!(
                "{name} replicates {} tables from the source; initial copy continues in the background ({:.1}s so far). Check: anybranch status {name}",
                tables.matches(", ").count() + 1,
                started.elapsed().as_secs_f64()
            );
            b.status()
        }
        Engine::Mysql => {
            let (user, pass, host, port) = parse_url(url)?;
            let on_source = |q: &str| mysql_on(&host, port, &user, &pass, q);
            let info = on_source("SELECT @@gtid_mode, @@log_bin")?;
            let (gtid, log_bin) = info.split_once('\t').unwrap_or((&info, ""));
            if gtid != "ON" || log_bin != "1" {
                return Err(format!(
                    "source needs gtid_mode=ON and log_bin=1 (has gtid_mode={gtid}, log_bin={log_bin}). \
                     RDS: gtid-mode=ON, enforce_gtid_consistency=ON, then reboot"
                ));
            }
            let dbs = on_source(
                "SELECT coalesce(GROUP_CONCAT(schema_name SEPARATOR ' '), '') FROM information_schema.schemata \
                 WHERE schema_name NOT IN ('mysql', 'sys', 'information_schema', 'performance_schema')",
            )?;
            if dbs.is_empty() {
                return Err("source has no user databases to replicate".into());
            }
            let b = Branch::new(name, engine)?;
            let result = write_secret(&b.dir.join("source"), url).and_then(|_| b.init()).and_then(|_| b.start()).and_then(|_| {
                let mut dump = Command::new("mysqldump");
                dump.args(["--no-defaults", "-h", &host, "-P", &port.to_string(), "-u", &user])
                    .args(["--single-transaction", "--set-gtid-purged=ON", "--routines", "--triggers", "--events", "--databases"])
                    .args(dbs.split(' '))
                    .env("MYSQL_PWD", &pass);
                pipe(&mut dump, &mut b.mysql())?;
                b.sql(&format!(
                    "CHANGE REPLICATION SOURCE TO SOURCE_HOST='{host}', SOURCE_PORT={port}, SOURCE_USER='{user}', \
                     SOURCE_PASSWORD='{}', SOURCE_AUTO_POSITION=1, GET_SOURCE_PUBLIC_KEY=1; START REPLICA",
                    pass.replace('\'', "''")
                ))
            });
            if let Err(e) = result {
                let _ = b.stop();
                let _ = fs::remove_dir_all(&b.dir);
                return Err(e);
            }
            println!("{}", b.url()?);
            eprintln!("{name} replicates [{dbs}] from the source ({:.1}s). Check: anybranch status {name}", started.elapsed().as_secs_f64());
            b.status()
        }
        Engine::Mongodb => {
            // Change streams need a replica set. Take a resume token before the dump: the tailer
            // starts there, and replaying the dump window is harmless (upserts by _id).
            let token = mongosh(
                url,
                "if (!db.hello().setName) throw 'source must be a replica set: change streams need one'; \
                 const s = db.watch([]); s.tryNext(); print(JSON.stringify(s.getResumeToken())); s.close()",
            )?;
            let b = Branch::new(name, engine)?;
            let result = b.init().and_then(|_| b.start()).and_then(|_| {
                pipe(
                    Command::new("mongodump").args(["--uri", url, "--archive", "--quiet"]),
                    Command::new("mongorestore").args(["--uri", &b.url()?, "--archive", "--drop", "--quiet", "--nsExclude", "admin.*", "--nsExclude", "config.*"]),
                )?;
                // `source` is written only now so start() did not launch the tailer before the restore.
                io(fs::write(b.run().join("token"), &token))?;
                write_secret(&b.dir.join("source"), url)?;
                b.spawn_tail()
            });
            if let Err(e) = result {
                let _ = b.stop();
                let _ = fs::remove_dir_all(&b.dir);
                return Err(e);
            }
            println!("{}", b.url()?);
            eprintln!("{name} replicates from the source via change streams ({:.1}s). Check: anybranch status {name}", started.elapsed().as_secs_f64());
            b.status()
        }
        _ => Err(format!("sync is not available for {}; use import", engine.name())),
    }
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
/// delete keyed by _id, so replaying the dump window or a restart overlap is harmless.
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
  fs.writeFileSync(run + '/token', JSON.stringify(e._id));
  fs.writeFileSync(run + '/applied', (e.wallTime || new Date()).toISOString());
}";

fn mongosh(url: &str, js: &str) -> R<String> {
    out(Command::new("mongosh").args(["--quiet", url, "--eval", js]))
}

fn create(name: &str, parent: &str) -> R<()> {
    let p = Branch::load(parent)?;
    let b = Branch::new(name, p.engine)?;
    let started = Instant::now();
    let was_running = p.running();
    p.refresh();
    if was_running {
        p.stop()?;
    }
    let cloned = clone(&p.data(), &b.data());
    if was_running {
        p.start()?;
    }
    io(fs::write(b.dir.join("parent"), parent))?;
    finish(b, cloned, Some(started))
}

fn reset(name: &str) -> R<()> {
    let b = Branch::load(name)?;
    let parent = b.parent().ok_or_else(|| format!("{name} is a root; only branches with a parent can be reset"))?;
    b.rm()?;
    create(name, &parent)
}

/// Start the new branch and print its URL, or remove the half-made directory.
fn finish(b: Branch, result: R<()>, started: Option<Instant>) -> R<()> {
    if let Err(e) = result.and_then(|_| b.start()) {
        let _ = fs::remove_dir_all(&b.dir);
        return Err(e);
    }
    println!("{}", b.url()?);
    if let Some(t) = started {
        eprintln!("branch {} ready in {:.2}s", b.name, t.elapsed().as_secs_f64());
    }
    Ok(())
}

fn list() -> R<()> {
    let Ok(entries) = fs::read_dir(home()) else { return Ok(()) };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    for name in names {
        let b = Branch::load(&name)?;
        let status = match (b.engine, b.running(), b.source().is_some()) {
            (Engine::Sqlite, _, _) => "file",
            (_, true, true) => "syncing",
            (_, true, false) => "running",
            (_, false, _) => "stopped",
        };
        println!("{name}\t{}\t{}\t{status}\t{}", b.engine.name(), b.parent().unwrap_or_else(|| "-".into()), b.url().unwrap_or_default());
    }
    Ok(())
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

fn psql(url: &str, statement: &str) -> R<String> {
    out(Command::new("psql").arg(url).args(["-X", "-At", "-v", "ON_ERROR_STOP=1", "-c", statement]))
}

fn mysql_on(host: &str, port: u16, user: &str, pass: &str, statement: &str) -> R<String> {
    out(Command::new("mysql")
        .args(["--no-defaults", "-N", "-B", "-h", host, "-P", &port.to_string(), "-u", user, "-e", statement])
        .env("MYSQL_PWD", pass))
}

/// `scheme://user[:pass]@host[:port]/...` → (user, pass, host, port), percent-decoded.
fn parse_url(url: &str) -> R<(String, String, String, u16)> {
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
        for name in ["dev", "main"] {
            let b = Branch::load(name).unwrap();
            fs::remove_dir_all(&b.dir).unwrap();
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn urls_parse() {
        let (u, p, h, port) = parse_url("mysql://repl:p%40ss@db.example.com:3307/app?ssl=1").unwrap();
        assert_eq!((u.as_str(), p.as_str(), h.as_str(), port), ("repl", "p@ss", "db.example.com", 3307));
        assert_eq!(parse_url("mysql://root@127.0.0.1/").unwrap().3, 3306);
    }
}
