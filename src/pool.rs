//! Explicit immutable snapshots with a finite inventory of prestarted, private branches.
//! The global worker lock serializes preparation and claims. Engine data paths never
//! move: claiming publishes a logical-name symlink to a private physical directory.
use crate::*;
use serde_json::json;

pub const PREFIX: &str = "abwarm-";

pub fn is_snapshot(b: &Branch) -> bool {
    b.dir.join("snapshot.json").exists()
}

pub fn writable(b: &Branch) -> R<()> {
    if is_snapshot(b) {
        Err("prepared snapshots are immutable; create an agent branch from the snapshot".into())
    } else {
        Ok(())
    }
}

fn slots(snapshot: &str) -> Vec<Branch> {
    all_branches()
        .into_iter()
        .filter(|b| b.name.starts_with(PREFIX) && b.parent().as_deref() == Some(snapshot))
        .collect()
}

pub fn prepare(name: &str, a: &Args) -> R<String> {
    remote::validate_name(name)?;
    let parent = a.flags.get("from").ok_or("prepare needs --from <parent>")?;
    let count: usize = a
        .flags
        .get("count")
        .ok_or("prepare needs --count <1-32>")?
        .parse()
        .map_err(|_| "count must be 1-32")?;
    if !(1..=32).contains(&count) {
        return Err("count must be 1-32".into());
    }
    let snapshot = if let Ok(b) = Branch::load(name) {
        if !is_snapshot(&b) || b.parent().as_deref() != Some(parent.as_str()) {
            return Err("name already exists with a different snapshot or parent".into());
        }
        b
    } else {
        let p = Branch::load(parent)?;
        writable(&p)?;
        let default_db = p.database();
        let b = create_cold(name, parent)?;
        b.stop()?;
        if let Some(pid) = live_pid(&b.run().join("proxypid")) {
            sh(Command::new("kill").args(["-TERM", &pid.to_string()]))?;
            wait(|| !alive(pid), 10, "snapshot proxy to stop")?;
        }
        io(fs::write(b.dir.join("default_db"), default_db))?;
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        io(fs::write(
            b.dir.join("snapshot.json"),
            json!({"source":parent,"created_unix_seconds":created}).to_string(),
        ))?;
        b
    };
    // A lost ready engine is repaired during preparation, never on the fast claim path.
    let mut ready = 0;
    for b in slots(name) {
        if b.dir.join("pool-ready").exists() {
            b.start()?;
            ready += 1;
        }
    }
    while ready < count {
        let id = format!("{PREFIX}{}", &random_password()?[..24]);
        let b = create_cold(&id, name)?;
        io(fs::write(b.dir.join("pool-ready"), ""))?;
        ready += 1;
    }
    Ok(json!({"snapshot":name,"ready":ready,"snapshot_metadata":serde_json::from_slice::<serde_json::Value>(&io(fs::read(snapshot.dir.join("snapshot.json")))?).map_err(|e| e.to_string())?}).to_string())
}

pub fn claim(name: &str, snapshot: &Branch) -> R<Branch> {
    remote::validate_name(name)?;
    if let Ok(existing) = Branch::load(name) {
        if existing.parent().as_deref() != Some(snapshot.name.as_str()) {
            return Err("branch name already exists with a different parent".into());
        }
        existing.start()?;
        return Ok(existing);
    }
    for b in slots(&snapshot.name) {
        let ready = b.dir.join("pool-ready");
        if !ready.exists() || !b.proxy_alive() || (b.engine != Engine::Sqlite && !b.running()) {
            continue;
        }
        // Remove availability before publication: a crash can strand a slot, but can
        // never give two agents the same database. Preparation creates replacement slots.
        io(fs::write(b.run().join("claimed-at"), ""))?;
        io(fs::remove_file(&ready))?;
        if let Err(e) = std::os::unix::fs::symlink(&b.dir, home().join(name)) {
            let _ = fs::write(ready, "");
            return Err(e.to_string());
        }
        let claimed = Branch::load(name)?;
        claimed.set_current()?;
        return Ok(claimed);
    }
    Err(format!(
        "no ready branches in snapshot {}; refill with: anybranch prepare {} --from {} --count <N>",
        snapshot.name,
        snapshot.name,
        snapshot.parent().unwrap_or_default()
    ))
}

pub fn before_remove(b: &Branch) -> R<()> {
    if !is_snapshot(b) {
        return Ok(());
    }
    if all_branches()
        .iter()
        .any(|c| !c.name.starts_with(PREFIX) && c.parent().as_deref() == Some(b.name.as_str()))
    {
        return Err("snapshot has agent branches; remove those branches first".into());
    }
    if b.dir.join("lock").exists() {
        return Err("snapshot is locked".into());
    }
    for slot in slots(&b.name) {
        slot.rm()?;
    }
    Ok(())
}
