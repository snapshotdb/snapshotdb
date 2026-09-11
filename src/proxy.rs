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
    env::var("ANYBRANCH_IDLE_MINUTES").ok().and_then(|v| v.parse().ok()).unwrap_or(5)
}

pub fn serve(name: &str) -> R<()> {
    let b = Branch::load(name)?;
    let port = b.port().ok_or("no public port allocated")?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    let activity = Arc::new(Activity { open: AtomicUsize::new(0), last: Mutex::new(Instant::now()), start: Mutex::new(()) });

    let idle = idle_minutes();
    if idle > 0 && b.source().is_none() {
        let (watched, act) = (Branch::load(name)?, activity.clone());
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            let quiet = act.open.load(Ordering::SeqCst) == 0
                && act.last.lock().map(|t| t.elapsed() >= Duration::from_secs(idle * 60)).unwrap_or(false);
            if quiet && watched.running() {
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
        if !b.running() {
            b.start_engine()?; // resume: the client simply waits a few hundred ms
        }
    }
    let eport = b.eport().ok_or("engine port unknown")?;
    let upstream = TcpStream::connect(("127.0.0.1", eport)).map_err(|e| format!("connect engine {eport}: {e}"))?;
    let (mut c_in, mut c_out) = (client.try_clone().map_err(|e| e.to_string())?, client);
    let (mut u_in, mut u_out) = (upstream.try_clone().map_err(|e| e.to_string())?, upstream);
    let to_engine = thread::spawn(move || {
        let _ = io::copy(&mut c_in, &mut u_out);
        let _ = u_out.shutdown(Shutdown::Write);
    });
    let _ = io::copy(&mut u_in, &mut c_out);
    let _ = c_out.shutdown(Shutdown::Write);
    let _ = to_engine.join();
    Ok(())
}
