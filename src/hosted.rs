//! Hosted workspaces. Only the trusted web gateway supplies a tenant identity.
//! BYOC processes have no SNAPSHOTDB_TENANT and never enter these billing paths.
use crate::*;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::net::ToSocketAddrs;

pub fn enabled() -> bool { env::var("SNAPSHOTDB_TENANT").is_ok() }
pub fn tenant_home(root: &Path, tenant: &str) -> R<PathBuf> {
    if !tenant.starts_with("github-") || tenant.len() > 32 || tenant[7..].is_empty()
        || !tenant[7..].bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid hosted identity".into());
    }
    let home = root.join(".tenants").join(tenant);
    io(fs::create_dir_all(&home))?;
    io(fs::set_permissions(&home, fs::Permissions::from_mode(0o700)))?;
    Ok(home)
}

pub fn recover() -> R<()> {
    for entry in fs::read_dir(home().join(".jobs")).into_iter().flatten().flatten() {
        if entry.path().extension().and_then(|v|v.to_str()) != Some("json") { continue; }
        let Ok(bytes)=fs::read(entry.path()) else { continue; };
        let Ok(mut v)=serde_json::from_slice::<Value>(&bytes) else { continue; };
        if matches!(v["state"].as_str(),Some("queued"|"running")) {
            v["state"]=json!("done");v["exit_code"]=json!(1);
            v["stderr"]=json!("Server restarted. Inspect branch state before retrying.");
            io(fs::write(entry.path(),v.to_string()))?;
        }
    }
    // On a clean shutdown the last meter sample already records zero engines.
    // On a crash only charge surviving processes, not the entire offline interval.
    let c=db(&home())?;
    if !branches(&home()).iter().any(live) {
        c.execute("UPDATE meter SET at=?1,active=0 WHERE id=1",[now()]).map_err(|e|e.to_string())?;
    }
    Ok(())
}

fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64 }
fn db(home: &Path) -> R<Connection> {
    let c = Connection::open(home.join(".billing.sqlite")).map_err(|e| e.to_string())?;
    c.busy_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())?;
    c.execute_batch("CREATE TABLE IF NOT EXISTS account(id INTEGER PRIMARY KEY CHECK(id=1), customer TEXT NOT NULL DEFAULT '', subscription TEXT NOT NULL DEFAULT '', period_start INTEGER NOT NULL DEFAULT 0, period_end INTEGER NOT NULL DEFAULT 0, paid INTEGER NOT NULL DEFAULT 0, event_time INTEGER NOT NULL DEFAULT 0);
        INSERT OR IGNORE INTO account(id) VALUES(1);
        CREATE TABLE IF NOT EXISTS usage(period INTEGER PRIMARY KEY, seconds INTEGER NOT NULL DEFAULT 0);
        CREATE TABLE IF NOT EXISTS transfer(period INTEGER PRIMARY KEY, bytes INTEGER NOT NULL DEFAULT 0);
        CREATE TABLE IF NOT EXISTS meter(id INTEGER PRIMARY KEY CHECK(id=1), at INTEGER NOT NULL, active INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS checkout(id INTEGER PRIMARY KEY CHECK(id=1), nonce TEXT NOT NULL, at INTEGER NOT NULL, url TEXT NOT NULL DEFAULT '');
        CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY);")
        .map_err(|e| e.to_string())?;
    Ok(c)
}
fn branches(home: &Path) -> Vec<Branch> {
    fs::read_dir(home).into_iter().flatten().flatten().filter_map(|e| {
        let name = e.file_name().into_string().ok()?;
        if name.starts_with('.') { return None; }
        let engine = Engine::parse(fs::read_to_string(e.path().join("engine")).ok()?.trim()).ok()?;
        Some(Branch { name, dir: e.path(), engine })
    }).collect()
}
fn live(b: &Branch) -> bool { if b.engine == Engine::Sqlite { b.proxy_alive() } else { b.running() } }
fn bytes(path: &Path) -> u64 {
    let Ok(m) = fs::symlink_metadata(path) else { return 0 };
    if m.is_symlink() { return 0; }
    if m.is_file() { return m.len(); }
    fs::read_dir(path).into_iter().flatten().flatten().map(|e| bytes(&e.path())).sum()
}

// Charge the previous sample's active instances; source replicas count too.
// Durable, serialized samples prevent duplicate charges from API/proxy watchdogs.
pub fn usage(home: &Path) -> R<Value> { usage_at(home, now()) }
fn usage_at(home: &Path, time: i64) -> R<Value> {
    let bs = branches(home);
    let active = bs.iter().filter(|b| live(b)).count() as i64;
    let sources = bs.iter().filter(|b| b.parent().is_none()).count();
    let storage: u64 = bs.iter().map(|b| bytes(&b.data())).sum();
    let mut c = db(home)?;
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
    let (customer, subscription, start, end, paid): (String,String,i64,i64,bool) = tx.query_row(
        "SELECT customer,subscription,period_start,period_end,paid FROM account WHERE id=1", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|e| e.to_string())?;
    let previous: Option<(i64,i64)> = tx.query_row("SELECT at,active FROM meter WHERE id=1", [], |r| Ok((r.get(0)?,r.get(1)?))).ok();
    if let Some((at, count)) = previous {
        let time = time.max(at);
        // Split a sample at the paid-period boundary; the free trial never resets.
        let pro_seconds = if paid { (time.min(end) - at.max(start)).max(0) } else { 0 };
        for (period, elapsed) in [(start,pro_seconds),(0,(time-at-pro_seconds).max(0))] {
            tx.execute("INSERT INTO usage(period,seconds) VALUES(?1,?2) ON CONFLICT(period) DO UPDATE SET seconds=seconds+excluded.seconds", params![period,elapsed.saturating_mul(count)]).map_err(|e| e.to_string())?;
        }
    }
    let sampled_at = previous.map(|p| p.0.max(time)).unwrap_or(time);
    tx.execute("INSERT INTO meter VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET at=excluded.at,active=excluded.active", params![sampled_at,active]).map_err(|e| e.to_string())?;
    let pro = paid && time >= start && time < end;
    let period = if pro { start } else { 0 };
    let used: i64 = tx.query_row("SELECT seconds FROM usage WHERE period=?1", [period], |r| r.get(0)).unwrap_or(0);
    let transfer: i64 = tx.query_row("SELECT bytes FROM transfer WHERE period=?1", [period], |r| r.get(0)).unwrap_or(0);
    tx.commit().map_err(|e| e.to_string())?;
    let limit = if pro { 300*3600 } else { 2*3600 };
    let storage_limit: u64 = if pro { 50 } else { 1 } * 1024*1024*1024;
    Ok(json!({"plan":if pro {"pro"} else {"free"},"price_monthly_usd":if pro {150} else {0},
        "source_limit":if pro {Value::Null} else {json!(1)},"sources":sources,"active":active,
        "concurrency_limit":if pro {4} else {2},"used_seconds":used,"limit_seconds":limit,
        "remaining_seconds":(limit-used).max(0),"period_end":if pro {end} else {0},
        "storage_bytes":storage,"storage_limit_bytes":storage_limit,
        "transfer_bytes":transfer,"transfer_limit_bytes":if pro {50u64*1024*1024*1024} else {1024u64*1024*1024},
        "customer":customer,"subscription":subscription}))
}

pub fn check(home: &Path) -> R<()> {
    let u = usage(home)?;
    if u["remaining_seconds"].as_i64().unwrap_or(0) <= 0 { return Err("PLAN_LIMIT: Branch-hour allowance exhausted. Upgrade or wait for your paid billing period to renew.".into()); }
    if u["storage_bytes"].as_u64().unwrap_or(0) >= u["storage_limit_bytes"].as_u64().unwrap_or(0) { return Err("PLAN_LIMIT: Storage allowance reached. Delete unused data or upgrade.".into()); }
    if u["transfer_bytes"].as_u64().unwrap_or(0) >= u["transfer_limit_bytes"].as_u64().unwrap_or(0) { return Err("PLAN_LIMIT: Data transfer allowance reached.".into()); }
    Ok(())
}

pub fn charge_transfer(count: usize) -> R<()> {
    if !enabled() { return Ok(()); }
    let mut c=db(&home())?;
    let tx=c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
    let (start,end,paid):(i64,i64,bool)=tx.query_row("SELECT period_start,period_end,paid FROM account WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
    let pro=paid && now()>=start && now()<end;
    let period=if pro {start} else {0};
    let limit=if pro {50i64*1024*1024*1024} else {1024i64*1024*1024};
    let used:i64=tx.query_row("SELECT bytes FROM transfer WHERE period=?1",[period],|r|r.get(0)).unwrap_or(0);
    if used+count as i64>limit { return Err("PLAN_LIMIT: Data transfer allowance reached.".into()); }
    tx.execute("INSERT INTO transfer VALUES(?1,?2) ON CONFLICT(period) DO UPDATE SET bytes=bytes+excluded.bytes",params![period,count as i64]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())
}

pub fn copy_out(input: &mut impl std::io::Read, output: &mut impl std::io::Write) -> R<()> {
    metered_copy(input, output, charge_transfer)
}

/// Charge in batches (1 MiB or 1 s), not per 64 KiB read: each charge is an IMMEDIATE
/// SQLite transaction. A busy/failed charge is retried with the next batch instead of
/// cutting the client's connection; only an exhausted allowance does that. A connection
/// can overrun its allowance by at most one batch.
fn metered_copy(input: &mut impl std::io::Read, output: &mut impl std::io::Write, mut charge: impl FnMut(usize) -> R<()>) -> R<()> {
    let (mut buf, mut pending, mut last) = ([0u8; 65536], 0usize, Instant::now());
    let result = loop {
        let n = match input.read(&mut buf) { Ok(0) => break Ok(()), Ok(n) => n, Err(e) => break Err(e.to_string()) };
        pending += n;
        if pending >= 1 << 20 || last.elapsed() >= Duration::from_secs(1) {
            last = Instant::now();
            match charge(pending) {
                Ok(()) => pending = 0,
                Err(e) if e.starts_with("PLAN_LIMIT") => return Err(e),
                Err(e) => eprintln!("transfer charge deferred: {e}"),
            }
        }
        if let Err(e) = output.write_all(&buf[..n]) { break Err(e.to_string()); }
    };
    if pending > 0 { if let Err(e) = charge(pending) { eprintln!("transfer charge lost ({pending} bytes): {e}"); } }
    result
}
pub fn before_start(b: &Branch) -> R<()> {
    if !enabled() { return Ok(()); }
    if env::var("SNAPSHOTDB_CLEANUP").as_deref() == Ok("1") { return Ok(()); }
    check(&home())?;
    let u = usage(&home())?;
    if !live(b) && u["active"].as_u64() >= u["concurrency_limit"].as_u64() {
        return Err("PLAN_LIMIT: Concurrent database limit reached. Stop another database first.".into());
    }
    Ok(())
}

pub fn start_lock() -> R<Option<fs::File>> {
    if !enabled() { return Ok(None); }
    let file = io(fs::OpenOptions::new().create(true).truncate(false).write(true).open(home().join(".start.lock")))?;
    io(fs2::FileExt::lock_exclusive(&file))?;
    Ok(Some(file))
}

/// Attach hosted processes to a delegated cgroup before exec. The kernel limits
/// each database to the 1 vCPU / 2 GiB compute unit used by our pricing model.
pub fn limit_process(cmd: &mut Command, b: &Branch) -> R<()> {
    if !enabled() { return Ok(()); }
    let root = env::var_os("SNAPSHOTDB_CGROUP_ROOT").ok_or("Hosted compute requires a delegated SNAPSHOTDB_CGROUP_ROOT (cgroup v2)")?;
    if !cfg!(target_os = "linux") { return Err("Hosted compute requires Linux cgroup v2".into()); }
    let tenant = env::var("SNAPSHOTDB_TENANT").map_err(|e| e.to_string())?;
    let group = PathBuf::from(root).join(format!("{tenant}-{}",b.name));
    io(fs::create_dir_all(&group))?;
    io(fs::write(group.join("cpu.max"), "100000 100000"))?;
    io(fs::write(group.join("memory.max"), "2147483648"))?;
    io(fs::write(group.join("memory.swap.max"), "0"))?;
    io(fs::write(group.join("pids.max"), "512"))?;
    let file = io(fs::OpenOptions::new().write(true).open(group.join("cgroup.procs")))?;
    // The fd is opened before fork; writing 0 moves the calling child. No allocation.
    unsafe { cmd.pre_exec(move || { use std::io::Write; (&file).write_all(b"0") }); }
    Ok(())
}

pub fn validate(args: &Args) -> R<()> {
    if !enabled() { return Ok(()); }
    let p: Vec<&str> = args.pos.iter().map(String::as_str).collect();
    match p.as_slice() {
        ["preflight", _, url] | ["sync", _, _, url] => public_source(url)?,
        _ => (),
    }
    match p.as_slice() {
        ["_hosted_up"] | ["_hosted_down"] | ["list"] | ["info", ..] | ["url", ..] | ["status", _] | ["stop", _] | ["rm", _]
        | ["lock", _] | ["unlock", _] | ["settings", _] | ["preflight", _, _] => return Ok(()),
        ["import", "sqlite", _, "--new"] => (),
        ["sync", _, _, _] | ["create", _] | ["start", _] | ["reset", _] | ["repair", _] | ["reconcile", _] => (),
        _ => return Err("This command is available only on a BYOC server.".into()),
    }
    check(&home())?;
    if matches!(p[0], "import" | "sync") {
        let u = usage(&home())?;
        if let Some(limit) = u["source_limit"].as_u64() {
            if u["sources"].as_u64().unwrap_or(0) >= limit { return Err("PLAN_LIMIT: Free includes one source database. Upgrade to Pro for unlimited source databases.".into()); }
        }
    }
    Ok(())
}

/// Hosted source URLs run psql/pg_dump/mongosh as the server user, outside the sandbox, and
/// every branch engine trusts its local socket. So a source must be a URL whose hosts are all
/// public network addresses: no socket paths (`?host=/…`, `%2F…`), no conninfo strings,
/// no loopback or private ranges (which also covers the gateway's own services).
fn public_source(url: &str) -> R<()> {
    let bad = || Err::<(), String>("Hosted sources must be a database URL with a public hostname.".into());
    let Some((scheme, rest)) = url.split_once("://") else { return bad() };
    if !matches!(scheme, "postgres" | "postgresql" | "mysql" | "mongodb" | "mongodb+srv") { return bad(); }
    let (authority, query) = match rest.split_once('?') { Some((a, q)) => (a, q), None => (rest, "") };
    let authority = authority.split('/').next().unwrap_or("");
    let hosts = authority.rsplit_once('@').map_or(authority, |x| x.1);
    for pair in query.split('&') {
        let key = decode(pair.split('=').next().unwrap_or("")).to_ascii_lowercase();
        if matches!(key.as_str(), "host" | "hostaddr" | "service" | "servicefile" | "passfile" | "socket") { return bad(); }
    }
    for hp in hosts.split(',') {
        let (host, port) = match hp.strip_prefix('[') {
            Some(v6) => v6.split_once(']').map(|(h, p)| (h, p.trim_start_matches(':'))).unwrap_or((v6, "")),
            None => hp.rsplit_once(':').unwrap_or((hp, "")),
        };
        let (host, port) = (decode(host), port.parse().unwrap_or(0));
        let host = host.as_str();
        if host.is_empty() || host.contains(['/', '\\', '%']) { return bad(); }
        // ponytail: resolve-then-connect leaves a DNS-rebinding window, and mongodb+srv SRV
        // targets are not checked; pin resolved IPs into the child's URL if that matters.
        if scheme == "mongodb+srv" { continue; }
        let addrs: Vec<_> = match (host, port).to_socket_addrs() { Ok(a) => a.collect(), Err(_) => return bad() };
        if addrs.is_empty() || addrs.iter().any(|a| !public_ip(a.ip())) { return bad(); }
    }
    Ok(())
}

fn public_ip(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr::*;
    match ip {
        V4(v) => !(v.is_loopback() || v.is_private() || v.is_link_local() || v.is_unspecified()
            || v.is_broadcast() || v.is_multicast() || v.octets()[0] == 100 && v.octets()[1] & 0xc0 == 64),
        V6(v) => match v.to_ipv4_mapped() {
            Some(v4) => public_ip(V4(v4)),
            None => !(v.is_loopback() || v.is_unspecified() || v.is_multicast()
                || v.segments()[0] & 0xfe00 == 0xfc00 || v.segments()[0] & 0xffc0 == 0xfe80),
        },
    }
}

// Runs independently of the command queue, including during a long import.
pub fn enforce(home: &Path) -> R<()> {
    if check(home).is_err() {
        for b in branches(home).into_iter().filter(live) {
            if let Err(e) = b.stop() { eprintln!("quota stop {}: {e}", b.name); }
        }
        usage(home)?;
    }
    Ok(())
}

/// Entitlements arrive only through the gateway's verified payment webhook.
pub fn entitlement(home: &Path, v: &Value) -> R<()> {
    usage(home)?;
    let event = v["event_id"].as_str().filter(|s| s.len() <= 128).ok_or("missing event id")?;
    let timestamp = v["event_time"].as_i64().ok_or("missing event time")?;
    let customer = v["customer"].as_str().ok_or("missing customer")?;
    let subscription = v["subscription"].as_str().ok_or("missing subscription")?;
    let start = v["period_start"].as_i64().ok_or("missing period start")?;
    let end = v["period_end"].as_i64().ok_or("missing period end")?;
    let paid = v["paid"].as_bool().ok_or("missing paid state")?;
    if start <= 0 || end <= start || end-start > 32*86400 { return Err("invalid monthly billing period".into()); }
    let mut c = db(home)?;
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
    let inserted = tx.execute("INSERT OR IGNORE INTO events(id) VALUES(?1)", [event]).map_err(|e| e.to_string())?;
    if inserted > 0 {
        tx.execute("UPDATE account SET customer=?1,subscription=?2,period_start=?3,period_end=?4,paid=?5,event_time=?6 WHERE id=1 AND event_time<=?6", params![customer,subscription,start,end,paid,timestamp]).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn checkout(home: &Path, v: &Value) -> R<Value> {
    let mut c = db(home)?;
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
    let current: Option<(String,i64,String)> = tx.query_row("SELECT nonce,at,url FROM checkout WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).ok();
    if let Some(url) = v["url"].as_str() {
        let nonce = v["nonce"].as_str().ok_or("missing checkout nonce")?;
        if current.as_ref().is_none_or(|c| c.0 != nonce) { return Err("checkout reservation expired".into()); }
        tx.execute("UPDATE checkout SET url=?1 WHERE id=1",[url]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        return Ok(json!({"url":url}));
    }
    if let Some((_,at,url)) = current {
        if now()-at < 1800 && !url.is_empty() { return Ok(json!({"url":url})); }
        if now()-at < 1860 { return Err("Checkout is pending. Wait up to 31 minutes before retrying if no checkout link appeared.".into()); }
    }
    let nonce = crate::random_password()?;
    tx.execute("INSERT INTO checkout VALUES(1,?1,?2,'') ON CONFLICT(id) DO UPDATE SET nonce=excluded.nonce,at=excluded.at,url=''",params![nonce,now()]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(json!({"nonce":nonce}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let h=env::temp_dir().join(format!("snapshotdb-meter-{}",crate::random_password().unwrap()));
        fs::create_dir_all(&h).unwrap();h
    }
    #[test]
    fn usage_is_durable_and_splits_period_boundary() {
        let h=fixture();let c=db(&h).unwrap();
        c.execute("UPDATE account SET period_start=100,period_end=200,paid=1",[]).unwrap();
        c.execute("INSERT INTO meter VALUES(1,190,2)",[]).unwrap();
        let u=usage_at(&h,210).unwrap();
        assert_eq!(u["plan"],"free");assert_eq!(u["used_seconds"],20);
        assert_eq!(c.query_row("SELECT seconds FROM usage WHERE period=100",[],|r|r.get::<_,i64>(0)).unwrap(),20);
        assert_eq!(usage_at(&h,220).unwrap()["used_seconds"],20); // no active engines, no extra charge
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn duplicate_and_out_of_order_payments_never_reset_usage() {
        let h=fixture();let time=now();
        let v=json!({"event_id":"one","event_time":time,"customer":"cust_test","subscription":"sub_test","period_start":time-100,"period_end":time+1000,"paid":true});
        entitlement(&h,&v).unwrap();
        let c=db(&h).unwrap();c.execute("INSERT INTO usage VALUES(?1,100)",[time-100]).unwrap();
        entitlement(&h,&v).unwrap();
        let mut old=v.clone();old["event_id"]=json!("old");old["event_time"]=json!(time-1);old["paid"]=json!(false);
        entitlement(&h,&old).unwrap();
        let u=usage(&h).unwrap();assert_eq!(u["plan"],"pro");assert_eq!(u["used_seconds"],100);assert_eq!(u["limit_seconds"],300*3600);
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn checkout_reservation_prevents_duplicate_creation() {
        let h=fixture();let v=checkout(&h,&json!({})).unwrap();
        assert!(checkout(&h,&json!({})).is_err());
        assert!(checkout(&h,&json!({"nonce":"wrong","url":"https://rzp.io/example"})).is_err());
        checkout(&h,&json!({"nonce":v["nonce"],"url":"https://rzp.io/example"})).unwrap();
        assert_eq!(checkout(&h,&json!({})).unwrap()["url"],"https://rzp.io/example");
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn transfer_is_charged_in_batches_and_busy_charges_are_retried() {
        let data = vec![7u8; (3 << 20) + 100];
        let (mut out, mut charges, mut busy) = (Vec::new(), Vec::new(), true);
        metered_copy(&mut &data[..], &mut out, |n| { if std::mem::take(&mut busy) { return Err("database is locked".into()); } charges.push(n); Ok(()) }).unwrap();
        assert_eq!(out, data);
        assert_eq!(charges.iter().sum::<usize>(), data.len(), "every byte is charged exactly once");
        assert!(charges.len() <= 4, "batched, not per read: {charges:?}");
        let mut sent = Vec::new();
        let r = metered_copy(&mut &data[..], &mut sent, |_| Err("PLAN_LIMIT: Data transfer allowance reached.".into()));
        assert!(r.unwrap_err().starts_with("PLAN_LIMIT") && sent.len() < 2 << 20);
    }

    #[test]
    fn hosted_sources_must_be_public_network_urls() {
        for url in ["postgresql:///postgres?host=/srv/.tenants/github-1/prod/run&port=5433",
            "postgresql://u@db.example.com/app?HOST=%2Ftmp", "postgresql://u@%2Ftmp/app",
            "postgresql://u@127.0.0.1:5432/app", "postgresql://u@localhost/app", "postgresql://u@[::1]:5432/app",
            "postgresql://u@10.0.0.5/app", "postgresql://u@169.254.169.254/app", "postgresql://u@1.1.1.1,127.0.0.1/app",
            "mongodb://%2Ftmp%2Fmongodb-27017.sock", "mysql://root@192.168.1.2:3306/", "host=/tmp dbname=x",
            "file:///etc/passwd", "postgresql://u@[::ffff:127.0.0.1]/app"] {
            assert!(public_source(url).is_err(), "{url}");
        }
        for url in ["postgresql://u:p@1.1.1.1:5432/app?sslmode=require", "mysql://u@8.8.8.8/", "mongodb+srv://u:p@cluster0.example.net/app",
            "postgresql://u@[2606:4700:4700::1111]:5432/app"] {
            assert!(public_source(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn account_paths_reject_traversal_and_quota_fails_closed() {
        let h=fixture();
        for name in ["../other","github-../../other","github-", "other-123"] { assert!(tenant_home(&h,name).is_err()); }
        let a=tenant_home(&h,"github-123").unwrap();let b=tenant_home(&h,"github-456").unwrap();
        db(&a).unwrap().execute("INSERT INTO usage VALUES(0,7200)",[]).unwrap();
        assert!(check(&a).unwrap_err().contains("PLAN_LIMIT"));assert!(check(&b).is_ok());
        fs::remove_dir_all(h).unwrap();
    }
}
