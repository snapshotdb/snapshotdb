//! Bounded scheduling: preserve workspace order without blocking other tenants.
use std::{collections::{HashSet, VecDeque}, path::PathBuf, sync::{atomic::{AtomicBool, Ordering}, Condvar, Mutex}, time::Duration};

pub struct Job {
    pub dir: PathBuf,
    pub id: String,
    pub args: Vec<String>,
    pub tenant: Option<String>,
}

#[derive(Default)]
struct State {
    pending: VecDeque<Job>,
    running: HashSet<PathBuf>,
}

#[derive(Default)]
pub struct Queue {
    state: Mutex<State>,
    changed: Condvar,
}

impl Queue {
    pub fn submit(&self, job: Job) -> Result<(), &'static str> {
        let mut state = self.state.lock().expect("job queue poisoned");
        if state.pending.len() >= 32 { return Err("server job queue full"); }
        let own = state.pending.iter().filter(|j| j.dir == job.dir).count()
            + usize::from(state.running.contains(&job.dir));
        if own >= 8 { return Err("workspace job queue full; wait for existing jobs"); }
        state.pending.push_back(job);
        self.changed.notify_all();
        Ok(())
    }

    pub fn next(&self, stopping: &AtomicBool) -> Option<Job> {
        let mut state = self.state.lock().expect("job queue poisoned");
        loop {
            if stopping.load(Ordering::Relaxed) { return None; }
            if let Some(index) = state.pending.iter().position(|j| !state.running.contains(&j.dir)) {
                let job = state.pending.remove(index).unwrap();
                state.running.insert(job.dir.clone());
                return Some(job);
            }
            state = self.changed.wait_timeout(state, Duration::from_millis(100)).expect("job queue poisoned").0;
        }
    }

    pub fn finish(&self, dir: &std::path::Path) {
        self.state.lock().expect("job queue poisoned").running.remove(dir);
        self.changed.notify_all();
    }

    pub fn drain(&self) -> Vec<Job> {
        self.state.lock().expect("job queue poisoned").pending.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job(tenant: &str, id: &str) -> Job {
        Job { dir: PathBuf::from(tenant), id: id.into(), args: vec![], tenant: Some(tenant.into()) }
    }
    #[test]
    fn workspace_fifo_does_not_block_another_tenant() {
        let q=Queue::default();let stop=AtomicBool::new(false);
        q.submit(job("a","a1")).unwrap();q.submit(job("a","a2")).unwrap();q.submit(job("b","b1")).unwrap();
        let first=q.next(&stop).unwrap();assert_eq!(first.id,"a1");
        assert_eq!(q.next(&stop).unwrap().id,"b1");
        q.finish(&first.dir);assert_eq!(q.next(&stop).unwrap().id,"a2");
        stop.store(true,Ordering::Relaxed);assert!(q.next(&stop).is_none());
    }
    #[test]
    fn one_tenant_cannot_fill_the_shared_queue() {
        let q=Queue::default();
        for i in 0..8 { q.submit(job("a",&i.to_string())).unwrap(); }
        assert!(q.submit(job("a","overflow")).is_err());
        q.submit(job("b","available")).unwrap();
        assert_eq!(q.drain().len(),9);
    }
}
