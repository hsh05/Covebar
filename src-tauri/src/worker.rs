//! A background thread that runs queued jobs and a periodic tick. Each hardware module
//! (media, audio, brightness, ...) owns one, so slow or COM-bound calls never block the UI.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

type Job = Box<dyn FnOnce() + Send>;

pub struct Worker {
    tx: Sender<Job>,
}

impl Worker {
    /// Start the thread: `init` runs first on it, then `tick` every `every` (between jobs).
    pub fn spawn(
        init: impl FnOnce() + Send + 'static,
        every: Duration,
        mut tick: impl FnMut() + Send + 'static,
    ) -> Worker {
        let (tx, rx) = channel::<Job>();
        std::thread::spawn(move || {
            init();
            tick();
            let mut next = Instant::now() + every;
            loop {
                let wait = next.saturating_duration_since(Instant::now());
                match rx.recv_timeout(wait) {
                    Ok(job) => job(),
                    Err(RecvTimeoutError::Timeout) => {
                        tick();
                        next = Instant::now() + every;
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        });
        Worker { tx }
    }

    /// Queue `f` on the worker thread.
    pub fn run(&self, f: impl FnOnce() + Send + 'static) {
        let _ = self.tx.send(Box::new(f));
    }

    /// Run `f` on the worker thread and wait for its result.
    pub fn call<R: Send + 'static>(&self, f: impl FnOnce() -> R + Send + 'static) -> Option<R> {
        let (rtx, rrx) = channel();
        self.run(move || {
            let _ = rtx.send(f());
        });
        rrx.recv().ok()
    }
}

/// Initialise COM for a worker thread (multithreaded apartment).
pub fn com_mta() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}
