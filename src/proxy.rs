//! Per-branch TCP proxy: owns the branch's stable public port, starts the engine on the
//! first connection after a suspend, splices bytes, and stops the engine when idle.
//! Synced roots never suspend: they must keep replicating.
use std::{
    env, io,
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{Branch, R};

struct Activity {
    open: AtomicUsize,
    last: Mutex<Instant>,
    start: Mutex<()>, // serialises engine starts across concurrent first connections
}

pub fn idle_minutes() -> u64 {
    env::var("SNAPSHOTDB_IDLE_MINUTES").ok().and_then(|v| v.parse().ok()).unwrap_or(5)
}

pub fn port_free(port: u16) -> bool {
    let bind = env::var("SNAPSHOTDB_DB_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    TcpListener::bind((bind.as_str(), port)).is_ok()
}

pub fn serve(name: &str) -> R<()> {
    let b = Branch::load(name)?;
    if b.engine == crate::Engine::Sqlite { return crate::sqlite::serve(&b); }
    let port = b.port().ok_or("no public port allocated")?;
    let bind = env::var("SNAPSHOTDB_DB_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    let listener = TcpListener::bind((bind.as_str(), port)).map_err(|e| format!("bind {bind}:{port}: {e}"))?;
    let activity = Arc::new(Activity { open: AtomicUsize::new(0), last: Mutex::new(Instant::now()), start: Mutex::new(()) });

    if crate::hosted::enabled() {
        let monitored = Branch::load(name)?;
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));
            if crate::hosted::check(&crate::home()).is_err() && monitored.running() { let _ = monitored.stop(); }
        });
    }
    let idle = idle_minutes();
    if idle > 0 && b.source().is_none() {
        let (watched, act) = (Branch::load(name)?, activity.clone());
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            let quiet = act.open.load(Ordering::SeqCst) == 0
                && act.last.lock().map(|t| t.elapsed() >= Duration::from_secs(idle * 60)).unwrap_or(false);
            let recently_claimed = watched.dir.join("run/claimed-at").metadata().and_then(|m| m.modified())
                .map(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(idle * 60)).unwrap_or(false);
            if quiet && watched.running() && !watched.dir.join("pool-ready").exists() && !recently_claimed {
                let _ = watched.stop();
            }
        });
    }

    for client in listener.incoming().flatten() {
        let (b, act) = (Branch::load(name)?, activity.clone());
        thread::spawn(move || {
            act.open.fetch_add(1, Ordering::SeqCst);
            if let Err(e) = splice(client, &b, &act) {
                eprintln!("{e}");
            }
            act.open.fetch_sub(1, Ordering::SeqCst);
            if let Ok(mut t) = act.last.lock() {
                *t = Instant::now();
            }
        });
    }
    Ok(())
}

fn splice(client: TcpStream, b: &Branch, act: &Activity) -> R<()> {
    {
        let _guard = act.start.lock().map_err(|_| "lock poisoned")?;
        b.start_engine()?; // Also gates connections while credentials/hooks are initializing.
    }
    let eport = b.eport().ok_or("engine port unknown")?;
    let upstream = TcpStream::connect(("127.0.0.1", eport)).map_err(|e| format!("connect engine {eport}: {e}"))?;
    let (mut c_in, mut c_out) = (client.try_clone().map_err(|e| e.to_string())?, client);
    let (mut u_in, mut u_out) = (upstream.try_clone().map_err(|e| e.to_string())?, upstream);
    let to_engine = thread::spawn(move || {
        let _ = io::copy(&mut c_in, &mut u_out);
        let _ = u_out.shutdown(Shutdown::Write);
    });
    if crate::hosted::enabled() {
        if let Err(e) = crate::hosted::copy_out(&mut u_in, &mut c_out) {
            eprintln!("{e}");
            let _ = c_out.shutdown(Shutdown::Both);
            let _ = u_in.shutdown(Shutdown::Both);
        }
    } else { let _ = io::copy(&mut u_in, &mut c_out); }
    let _ = c_out.shutdown(Shutdown::Write);
    let _ = to_engine.join();
    Ok(())
}
