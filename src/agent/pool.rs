//! Agent mode: worker pool that keeps the HTTP accept loop non-blocking.
//!
//! The agent server used to handle requests inline on the accept loop, so one
//! 30-second exec stalled every other request on the box: session creation,
//! file writes, `/metrics`, and every other tenant. Real concurrency was 1,
//! which also meant `--max-concurrent-exec` and the per-tenant caps could never
//! bind over HTTP. The accept loop now hands each request to a worker and goes
//! straight back to accepting.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Extra workers above the exec cap, reserved for requests that do not exec.
///
/// A request handler blocks for the whole exec it starts, so a pool sized
/// exactly at `max_concurrent_exec` would leave nothing to answer session CRUD,
/// file writes or `/metrics` while execs are saturated.
const WORKER_HEADROOM: usize = 16;

/// Pool ceiling when the exec cap is `0` (unlimited). Unlimited execs must not
/// mean unlimited threads: a client could otherwise spawn one per connection.
const UNLIMITED_EXEC_WORKERS: usize = 512;

/// How long [`WorkerPool::shutdown`] waits for in-flight requests before
/// leaving them to die with the process. Bounded because a request can be
/// blocked on an exec running to its own (much longer) timeout.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Resolve the worker-pool ceiling from the configured value.
///
/// `configured == 0` means auto: derive it from the exec cap, since that is
/// what the pool has to be able to cover for the cap to be reachable at all.
pub fn resolve_workers(configured: usize, max_concurrent_exec: usize) -> usize {
    if configured > 0 {
        return configured;
    }
    if max_concurrent_exec == 0 {
        return UNLIMITED_EXEC_WORKERS;
    }
    max_concurrent_exec.saturating_add(WORKER_HEADROOM)
}

/// Live pool counters, shared with the server so `/metrics` can report them.
///
/// Owned by the server and handed to the pool at `start()`, because a pool only
/// exists while the server is listening.
#[derive(Default)]
pub struct PoolStats {
    live: AtomicUsize,
    busy: AtomicUsize,
}

impl PoolStats {
    /// Worker threads currently spawned (busy or parked).
    pub fn live(&self) -> u64 {
        self.live.load(Ordering::Acquire) as u64
    }

    /// Requests currently being handled.
    pub fn busy(&self) -> u64 {
        self.busy.load(Ordering::Acquire) as u64
    }
}

/// One spawned worker: a channel to hand it a job, and its thread.
struct Worker {
    /// `None` once [`WorkerPool::shutdown`] has closed the channel, which is
    /// what tells the thread to exit.
    tx: Option<Sender<Job>>,
    handle: JoinHandle<()>,
}

/// Grow-on-demand pool of request-handling threads.
///
/// A worker is spawned only when every existing one is busy, up to `max`. At
/// `max` [`dispatch`](WorkerPool::dispatch) blocks until a worker frees up, so
/// backpressure lands on the TCP backlog rather than on unbounded thread
/// spawning. Workers are not reaped once spawned: a parked thread costs a few
/// KB of resident stack and the pool is bounded.
///
/// Owned by the accept loop, which is the only dispatcher; the idle set is
/// therefore plain state rather than a shared queue, and workers hand
/// themselves back over `ret_tx` when a request completes.
pub struct WorkerPool {
    workers: Vec<Option<Worker>>,
    /// Ids ready for work, most recently used first (warm thread first).
    idle: Vec<usize>,
    live: usize,
    max: usize,
    ret_tx: Sender<usize>,
    ret_rx: Receiver<usize>,
    stats: Arc<PoolStats>,
}

impl WorkerPool {
    pub fn new(max: usize, stats: Arc<PoolStats>) -> Self {
        let (ret_tx, ret_rx) = channel();
        Self {
            workers: Vec::new(),
            idle: Vec::new(),
            live: 0,
            max: max.max(1),
            ret_tx,
            ret_rx,
            stats,
        }
    }

    /// Run `job` on a worker.
    ///
    /// Blocks only when the pool is at `max` and every worker is busy. Falls
    /// back to running the job inline if no thread could be spawned at all,
    /// which keeps the server correct (if serial) under thread exhaustion.
    pub fn dispatch(&mut self, mut job: Job) {
        loop {
            let Some(id) = self.acquire() else {
                job();
                return;
            };
            self.stats.busy.fetch_add(1, Ordering::AcqRel);
            let sender = self.workers[id].as_ref().and_then(|w| w.tx.as_ref());
            match sender.map(|tx| tx.send(job)) {
                Some(Ok(())) => return,
                // The worker is gone: retire the slot and retry elsewhere.
                Some(Err(returned)) => {
                    self.stats.busy.fetch_sub(1, Ordering::AcqRel);
                    self.retire(id);
                    job = returned.0;
                }
                None => unreachable!("acquire only returns live workers"),
            }
        }
    }

    /// An idle worker id, or `None` when the pool has no workers and cannot
    /// spawn one (the caller then runs the job inline).
    fn acquire(&mut self) -> Option<usize> {
        loop {
            while let Ok(id) = self.ret_rx.try_recv() {
                self.idle.push(id);
            }
            while let Some(id) = self.idle.pop() {
                if self.workers[id].is_some() {
                    return Some(id);
                }
            }
            if self.live < self.max {
                if let Some(id) = self.spawn() {
                    return Some(id);
                }
                if self.live == 0 {
                    return None;
                }
            }
            // Saturated, or out of threads with some still live: wait for the
            // first worker to finish its request.
            match self.ret_rx.recv() {
                Ok(id) => self.idle.push(id),
                Err(_) => return None,
            }
        }
    }

    /// Spawn a worker and return its id, or `None` if the thread could not be
    /// created (fd/thread exhaustion).
    fn spawn(&mut self) -> Option<usize> {
        let id = self
            .workers
            .iter()
            .position(|w| w.is_none())
            .unwrap_or(self.workers.len());
        let (tx, rx) = channel::<Job>();
        let ret = self.ret_tx.clone();
        let stats = self.stats.clone();
        let handle = std::thread::Builder::new()
            .name(format!("wasmrun-agent-{id}"))
            .spawn(move || worker_loop(id, rx, ret, stats))
            .map_err(|e| eprintln!("Failed to spawn request worker: {e}"))
            .ok()?;
        let worker = Some(Worker {
            tx: Some(tx),
            handle,
        });
        if id == self.workers.len() {
            self.workers.push(worker);
        } else {
            self.workers[id] = worker;
        }
        self.live += 1;
        self.stats.live.fetch_add(1, Ordering::AcqRel);
        Some(id)
    }

    /// Drop a dead worker from rotation, freeing its slot for a new one.
    fn retire(&mut self, id: usize) {
        if self.workers[id].take().is_some() {
            self.live -= 1;
            self.stats.live.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Stop accepting work and wait up to [`SHUTDOWN_GRACE`] for in-flight
    /// requests to finish.
    ///
    /// Best-effort: a request still running at the deadline is left to die with
    /// the process, since it may be blocked on an exec with a far longer
    /// timeout of its own.
    pub fn shutdown(mut self) {
        // Closing every job channel ends each worker loop once it goes idle.
        for worker in self.workers.iter_mut().flatten() {
            worker.tx = None;
        }
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while self.stats.busy() > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.stats.busy() > 0 {
            return;
        }
        for worker in self.workers.drain(..).flatten() {
            let _ = worker.handle.join();
        }
    }
}

/// Run jobs until the pool closes the channel, handing this worker back to the
/// dispatcher after each one.
///
/// A panicking handler must not take the worker (or the server) with it, so the
/// job is run under `catch_unwind`: the request is dropped, the client sees the
/// connection close, and the worker stays in rotation.
fn worker_loop(id: usize, rx: Receiver<Job>, ret: Sender<usize>, stats: Arc<PoolStats>) {
    while let Ok(job) = rx.recv() {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        if outcome.is_err() {
            eprintln!("Request handler panicked; worker recovered");
        }
        // Hand the worker back before clearing the busy gauge, so anything that
        // observes `busy == 0` can rely on every worker being available again.
        let returned = ret.send(id).is_ok();
        stats.busy.fetch_sub(1, Ordering::AcqRel);
        if !returned {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    fn pool(max: usize) -> (WorkerPool, Arc<PoolStats>) {
        let stats = Arc::new(PoolStats::default());
        (WorkerPool::new(max, stats.clone()), stats)
    }

    /// Wait for `cond` to hold, up to a generous ceiling (CI is slow).
    fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if cond() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        cond()
    }

    #[test]
    fn test_resolve_workers_covers_the_exec_cap() {
        // Auto sizing leaves room above the exec cap for cheap requests.
        assert_eq!(resolve_workers(0, 100), 100 + WORKER_HEADROOM);
        assert_eq!(resolve_workers(0, 1), 1 + WORKER_HEADROOM);
        // Unlimited execs still get a bounded pool.
        assert_eq!(resolve_workers(0, 0), UNLIMITED_EXEC_WORKERS);
        // An explicit value wins, including one below the exec cap.
        assert_eq!(resolve_workers(4, 100), 4);
        assert_eq!(resolve_workers(1, 0), 1);
        assert_eq!(resolve_workers(usize::MAX, 0), usize::MAX);
    }

    #[test]
    fn test_pool_runs_jobs_and_reuses_one_worker() {
        let (mut pool, stats) = pool(8);
        for i in 0..5 {
            let (tx, rx) = channel();
            pool.dispatch(Box::new(move || tx.send(i).unwrap()));
            assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), i);
            assert!(wait_for(|| stats.busy() == 0));
        }
        assert_eq!(stats.live(), 1, "sequential work needs one thread");
        pool.shutdown();
    }

    #[test]
    fn test_pool_grows_only_while_workers_are_busy() {
        let (mut pool, stats) = pool(8);
        let (release_tx, release_rx) = channel::<()>();
        let release_rx = Arc::new(std::sync::Mutex::new(release_rx));

        for _ in 0..3 {
            let rx = release_rx.clone();
            pool.dispatch(Box::new(move || {
                let _ = rx.lock().unwrap().recv();
            }));
        }
        assert!(wait_for(|| stats.busy() == 3));
        assert_eq!(stats.live(), 3, "one worker per concurrently blocked job");

        for _ in 0..3 {
            release_tx.send(()).unwrap();
        }
        assert!(wait_for(|| stats.busy() == 0));

        // The freed workers are reused rather than added to.
        pool.dispatch(Box::new(|| {}));
        assert!(wait_for(|| stats.busy() == 0));
        assert_eq!(stats.live(), 3);
        pool.shutdown();
    }

    #[test]
    fn test_dispatch_blocks_at_max_until_a_worker_frees_up() {
        let (mut pool, stats) = pool(2);
        // A rendezvous channel: each job parks until the test releases it.
        let (release_tx, release_rx) = sync_channel::<()>(0);
        let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
        for _ in 0..2 {
            let rx = release_rx.clone();
            pool.dispatch(Box::new(move || {
                let _ = rx.lock().unwrap().recv();
            }));
        }
        assert!(wait_for(|| stats.busy() == 2));

        // Release one job after a delay; the third dispatch must wait for it.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let _ = release_tx.send(());
            let _ = release_tx.send(());
        });
        let start = Instant::now();
        let (done_tx, done_rx) = channel();
        pool.dispatch(Box::new(move || done_tx.send(()).unwrap()));
        assert!(
            start.elapsed() >= Duration::from_millis(200),
            "dispatch returned before a worker was free"
        );
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(stats.live(), 2, "the pool never exceeds its ceiling");
        pool.shutdown();
    }

    #[test]
    fn test_pool_survives_a_panicking_job() {
        let (mut pool, stats) = pool(4);
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        pool.dispatch(Box::new(|| panic!("handler exploded")));
        assert!(wait_for(|| stats.busy() == 0));
        std::panic::set_hook(hook);

        // Same worker, still serving.
        let (tx, rx) = channel();
        pool.dispatch(Box::new(move || tx.send(7).unwrap()));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), 7);
        assert_eq!(stats.live(), 1);
        pool.shutdown();
    }

    #[test]
    fn test_shutdown_joins_idle_workers() {
        let (mut pool, stats) = pool(4);
        let (tx, rx) = channel();
        pool.dispatch(Box::new(move || tx.send(()).unwrap()));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(wait_for(|| stats.busy() == 0));
        // Returns once the workers have exited, not after the grace period.
        let start = Instant::now();
        pool.shutdown();
        assert!(start.elapsed() < SHUTDOWN_GRACE);
    }

    #[test]
    fn test_shutdown_gives_up_on_a_stuck_request() {
        let (mut pool, _stats) = pool(2);
        let (block_tx, block_rx) = channel::<()>();
        pool.dispatch(Box::new(move || {
            let _ = block_rx.recv();
        }));
        let start = Instant::now();
        pool.shutdown();
        assert!(start.elapsed() >= SHUTDOWN_GRACE, "waited for the deadline");
        assert!(
            start.elapsed() < SHUTDOWN_GRACE * 4,
            "but did not wait for the request"
        );
        drop(block_tx);
    }
}
