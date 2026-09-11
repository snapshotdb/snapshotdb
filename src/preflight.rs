//! `anybranch preflight`: can this source be branched safely? Creates nothing, stores
//! nothing. `sync` runs the same checks first and refuses on any failure.
use std::fmt::Write as _;

use crate::{home, js, mongosh, mysql_on, parse_url, psql, Engine, R};

#[derive(PartialEq, Clone, Copy)]
pub enum State {
    Pass,
    Warn,
    Fail,
}

pub struct Check {
    pub name: &'static str,
    pub state: State,
    pub detail: String,
    pub fix: String,
}

pub struct Report {
    pub checks: Vec<Check>,
    pub grant_script: String,
    /// Postgres: `schema.table, ...` with a replica identity, ready for CREATE PUBLICATION.
    pub tables: String,
    /// Postgres: tables left out, as `ALTER TABLE ... REPLICA IDENTITY FULL;` statements.
    pub unkeyed: Vec<String>,
    /// MySQL: user databases, space separated.
    pub databases: String,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.state != State::Fail)
    }

    pub fn render(&self, json: bool) -> String {
        if json {
            let checks: Vec<String> = self.checks.iter().map(|c| format!(
                "{{\"name\":{},\"state\":{},\"detail\":{},\"fix\":{}}}",
                js(c.name), js(match c.state { State::Pass => "pass", State::Warn => "warn", State::Fail => "fail" }), js(&c.detail), js(&c.fix)
            )).collect();
            return format!("{{\"passed\":{},\"checks\":[{}],\"grant_script\":{}}}\n", self.passed(), checks.join(","), js(&self.grant_script));
        }
        let mut out = String::new();
        for c in &self.checks {
            let mark = match c.state { State::Pass => "✓", State::Warn => "!", State::Fail => "✗" };
            let _ = writeln!(out, "  {mark} {}{}", c.name, if c.detail.is_empty() { String::new() } else { format!(" ({})", c.detail) });
            if !c.fix.is_empty() && c.state != State::Pass {
                let _ = writeln!(out, "      → {}", c.fix.replace('\n', "\n        "));
            }
        }
        if !self.grant_script.is_empty() {
            let _ = write!(out, "\n--- Grant script (run on the source as an admin) ---\n{}\n", self.grant_script);
        }
        let _ = writeln!(out, "\n{}", if self.passed() { "✓ Preflight passed." } else { "✗ Preflight did NOT pass. Fix the failures above." });
        out
    }
}

fn check(name: &'static str, ok: bool, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { name, state: if ok { State::Pass } else { State::Fail }, detail: detail.into(), fix: fix.into() }
}

fn warn(name: &'static str, ok: bool, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { name, state: if ok { State::Pass } else { State::Warn }, detail: detail.into(), fix: fix.into() }
}

/// Another root in this workspace already replicating from the same URL.
fn duplicate_source(url: &str) -> Check {
    let dup = std::fs::read_dir(home())
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| std::fs::read_to_string(e.path().join("source")).map(|s| s == url).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned());
    warn("duplicate source", dup.is_none(), dup.as_deref().map(|n| format!("already synced as {n}")).unwrap_or_default(), if dup.is_some() { "branch from the existing root instead of syncing twice" } else { "" })
}

pub fn run(engine: Engine, url: &str, schemas: &str) -> R<Report> {
    match engine {
        Engine::Postgres => postgres(url, schemas),
        Engine::Mysql => mysql(url),
        Engine::Mongodb => mongodb(url),
        Engine::Sqlite => Err("sqlite has nothing to preflight; use import".into()),
    }
}

fn unreachable_report(name: &'static str, err: String, fix: &str) -> Report {
    Report {
        checks: vec![check(name, false, err.lines().last().unwrap_or("").to_string(), fix)],
        grant_script: String::new(),
        tables: String::new(),
        unkeyed: vec![],
        databases: String::new(),
    }
}

fn postgres(url: &str, schemas: &str) -> R<Report> {
    let list = schemas.split(',').map(|s| format!("'{}'", s.trim())).collect::<Vec<_>>().join(",");
    let server = match psql(url, "SELECT current_setting('server_version_num')::int, pg_is_in_recovery(), current_setting('wal_level'), \
        (SELECT count(*) FROM pg_replication_slots), current_setting('max_replication_slots')::int, \
        (SELECT count(*) FROM pg_stat_replication), current_setting('max_wal_senders')::int, \
        (SELECT rolsuper FROM pg_roles WHERE rolname = current_user), \
        EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'rds_superuser') AND pg_has_role(current_user, 'rds_superuser', 'MEMBER'), \
        has_database_privilege(current_database(), 'CREATE'), current_user") {
        Ok(row) => row,
        Err(e) => return Ok(unreachable_report("connection", e, "check host, port, credentials and that the server allows this client (pg_hba.conf)")),
    };
    let f: Vec<&str> = server.split('|').collect();
    let version: i64 = f[0].parse().unwrap_or(0);
    let (slots_used, slots_max): (i64, i64) = (f[3].parse().unwrap_or(0), f[4].parse().unwrap_or(0));
    let (senders_used, senders_max): (i64, i64) = (f[5].parse().unwrap_or(0), f[6].parse().unwrap_or(0));
    let user = f[10];
    let superish = f[7] == "t" || f[8] == "t";

    let tables = psql(url, &format!(
        "SELECT format('%I.%I', n.nspname, c.relname) || '|' || has_table_privilege(c.oid, 'SELECT') || '|' || \
         (pg_has_role(c.relowner, 'USAGE') OR (SELECT rolsuper FROM pg_roles WHERE rolname = current_user)) || '|' || \
         (c.relreplident IN ('f', 'i') OR EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = c.oid AND i.indisprimary)) \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ('r', 'p') AND n.nspname IN ({list}) ORDER BY 1"
    ))?;
    let (mut unreadable, mut unowned, mut unkeyed, mut keyed) = (vec![], vec![], vec![], vec![]);
    for line in tables.lines() {
        let t: Vec<&str> = line.split('|').collect();
        if t.len() < 4 {
            continue;
        }
        // booleans concatenated with || come out as true/false, not psql's t/f
        if t[1] != "true" {
            unreadable.push(t[0].to_string());
        }
        if t[2] != "true" {
            unowned.push(t[0].to_string());
        }
        if t[3] == "true" {
            keyed.push(t[0].to_string());
        } else {
            unkeyed.push(format!("ALTER TABLE {} REPLICA IDENTITY FULL;", t[0]));
        }
    }
    let mut grant = String::new();
    if !unreadable.is_empty() {
        for schema in schemas.split(',') {
            let _ = writeln!(grant, "GRANT USAGE ON SCHEMA {0} TO \"{user}\";\nGRANT SELECT ON ALL TABLES IN SCHEMA {0} TO \"{user}\";", schema.trim());
        }
    }
    let checks = vec![
        check("connection", true, format!("PostgreSQL {}.{}", version / 10000, version % 10000), ""),
        check("postgres version", version >= 130000, "needs 13+", "upgrade the source or branch from a pg_dump instead"),
        check("is writer", f[1] == "f", if f[1] == "f" { "primary" } else { "read replica" }, if f[1] == "f" { "" } else { "point at the writable primary; replicas cannot publish" }),
        check("wal level", f[2] == "logical", format!("actual: {}", f[2]), if f[2] == "logical" { "" } else { "RDS: rds.logical_replication=1 and reboot; self-hosted: wal_level=logical and restart" }),
        check("replication slots", slots_used < slots_max, format!("{slots_used}/{slots_max} used"), if slots_used < slots_max { "" } else { "raise max_replication_slots or drop an unused slot" }),
        check("wal senders", senders_used < senders_max, format!("{senders_used}/{senders_max} used"), if senders_used < senders_max { "" } else { "raise max_wal_senders" }),
        check("tables found", !keyed.is_empty() || !unkeyed.is_empty(), format!("{} in {schemas}", keyed.len() + unkeyed.len()), "check --schemas"),
        check("can read tables", unreadable.is_empty(), if unreadable.is_empty() { String::new() } else { format!("missing SELECT on {}", unreadable.join(", ")) }, if unreadable.is_empty() { "" } else { "run the grant script below, then re-run preflight" }),
        check("can publish tables", unowned.is_empty(), if unowned.is_empty() { String::new() } else { format!("not owner of {}", unowned.join(", ")) }, if unowned.is_empty() { String::new() } else { format!("a publication needs the table owner: run as the owner, or ALTER TABLE ... OWNER TO \"{user}\"") }),
        check("can create schema", f[9] == "t", "", "GRANT CREATE ON DATABASE <db> TO the user (for the DDL log table)"),
        warn("can create event trigger", superish, if superish { "superuser" } else { "not superuser" }, if superish { "" } else { "schema changes will not replicate; use a superuser (RDS: rds_superuser) or rm and sync again after migrations" }),
        warn("replica identity", unkeyed.is_empty(), if unkeyed.is_empty() { String::new() } else { format!("{} tables without a primary key are skipped", unkeyed.len()) }, if unkeyed.is_empty() { String::new() } else { format!("include them with --fix-replica-identity, or run on the source:\n{}", unkeyed.join("\n")) }),
        duplicate_source(url),
    ];
    Ok(Report { checks, grant_script: grant, tables: keyed.join(", "), unkeyed, databases: String::new() })
}

fn mysql(url: &str) -> R<Report> {
    let (user, pass, host, port) = parse_url(url)?;
    let on = |q: &str| mysql_on(&host, port, &user, &pass, q);
    let row = match on("SELECT @@version, @@gtid_mode, @@enforce_gtid_consistency, @@log_bin, @@binlog_format, @@server_id") {
        Ok(r) => r,
        Err(e) => return Ok(unreachable_report("connection", e, "check host, port, credentials and that the user may connect from this host")),
    };
    let f: Vec<&str> = row.split('\t').collect();
    let grants = on("SHOW GRANTS").unwrap_or_default().to_uppercase();
    let can_replicate = grants.contains("REPLICATION SLAVE") || grants.contains("ALL PRIVILEGES ON *.*");
    let can_read = grants.contains("SELECT") || grants.contains("ALL PRIVILEGES");
    let databases = on("SELECT coalesce(GROUP_CONCAT(schema_name SEPARATOR ' '), '') FROM information_schema.schemata WHERE schema_name NOT IN ('mysql', 'sys', 'information_schema', 'performance_schema')")?;
    let grant = if can_replicate && can_read { String::new() } else {
        format!("GRANT REPLICATION SLAVE, REPLICATION CLIENT, SELECT, SHOW VIEW, TRIGGER, EVENT ON *.* TO '{user}'@'%';")
    };
    let checks = vec![
        check("connection", true, format!("MySQL {}", f[0]), ""),
        check("mysql version", f[0].split('.').next().and_then(|m| m.parse::<u32>().ok()).unwrap_or(0) >= 8, "needs 8.0+", "upgrade the source"),
        check("gtid mode", f[1] == "ON", format!("actual: {}", f[1]), if f[1] == "ON" { "" } else { "set gtid_mode=ON and enforce_gtid_consistency=ON (RDS: parameter group, then reboot)" }),
        check("gtid consistency", f[2] == "ON", format!("actual: {}", f[2]), if f[2] == "ON" { "" } else { "set enforce_gtid_consistency=ON" }),
        check("binary log", f[3] == "1", if f[3] == "1" { "enabled" } else { "disabled" }, if f[3] == "1" { "" } else { "enable log_bin (RDS: enable automated backups)" }),
        check("binlog format", f[4] == "ROW", format!("actual: {}", f[4]), if f[4] == "ROW" { "" } else { "set binlog_format=ROW" }),
        check("server id", f[5] != "0", format!("server_id={}", f[5]), if f[5] != "0" { "" } else { "set a non-zero server_id" }),
        check("can replicate", can_replicate, "", if can_replicate { "" } else { "grant REPLICATION SLAVE; see the grant script" }),
        check("can read tables", can_read, "", if can_read { "" } else { "grant SELECT; see the grant script" }),
        check("databases found", !databases.is_empty(), databases.replace(' ', ", "), "the source has no user databases"),
        duplicate_source(url),
    ];
    Ok(Report { checks, grant_script: grant, tables: String::new(), unkeyed: vec![], databases })
}

fn mongodb(url: &str) -> R<Report> {
    let row = match mongosh(url, "const h = db.hello(); const v = db.version(); let cs = 'ok'; try { const s = db.watch([]); s.tryNext(); s.close(); } catch (e) { cs = e.codeName || String(e); } \
        print([h.setName || '', h.isWritablePrimary, v, cs, db.adminCommand({ listDatabases: 1, nameOnly: true }).databases.filter(d => !['admin', 'local', 'config'].includes(d.name)).length].join('|'))") {
        Ok(r) => r,
        Err(e) => return Ok(unreachable_report("connection", e, "check the URI, credentials, and network access (Atlas: IP access list)")),
    };
    let f: Vec<&str> = row.trim().split('|').collect();
    let major: u32 = f.get(2).and_then(|v| v.split('.').next()).and_then(|m| m.parse().ok()).unwrap_or(0);
    let checks = vec![
        check("connection", true, format!("MongoDB {}", f.get(2).unwrap_or(&"")), ""),
        check("mongodb version", major >= 6, "needs 6.0+", "upgrade the source"),
        check("replica set", !f[0].is_empty(), if f[0].is_empty() { "standalone".to_string() } else { format!("set {}", f[0]) }, if f[0].is_empty() { "change streams need a replica set: rs.initiate() on the source (single node is fine)" } else { "" }),
        check("is writer", f.get(1) == Some(&"true"), "", "point at the primary or the replica set URI"),
        check("change streams", f.get(3) == Some(&"ok"), f.get(3).map(|s| s.to_string()).unwrap_or_default(), if f.get(3) == Some(&"ok") { "" } else { "the user needs changeStream and find on all databases (Atlas: readAnyDatabase role)" }),
        check("databases found", f.get(4).map(|n| n != &"0").unwrap_or(false), format!("{} user databases", f.get(4).unwrap_or(&"0")), "the source has no user databases"),
        duplicate_source(url),
    ];
    Ok(Report { checks, grant_script: String::new(), tables: String::new(), unkeyed: vec![], databases: String::new() })
}
