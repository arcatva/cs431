//! Thread pool that joins all thread when dropped.

use core::time;
// NOTE: Crossbeam channels are MPMC, which means that you don't need to wrap the receiver in
// Arc<Mutex<..>>. Just clone the receiver and give it to each worker thread.
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{unbounded, Receiver, Sender};

struct Job(Box<dyn FnOnce() + Send + 'static>);

#[derive(Debug)]
struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(_id: usize, receiver: Receiver<Job>, pool_inner: Arc<ThreadPoolInner>) -> Worker {
        Worker {
            _id: _id,
            thread: Some(
                thread::Builder::new()
                    .spawn(move || loop {
                        match receiver.recv() {
                            Ok(job) => {
                                job.0();
                                pool_inner.finish_job();
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    })
                    .unwrap(),
            ),
        }
    }
}

impl Drop for Worker {
    /// When dropped, the thread's `JoinHandle` must be `join`ed.  If the worker panics, then this
    /// function should panic too.
    ///
    /// NOTE: The thread is detached if not `join`ed explicitly.
    fn drop(&mut self) {
        if let Some(handler) = self.thread.take() {
            handler
                .join()
                .map_err(|op| format!("Failed to join worker {} {:?}", self._id, op))
                .unwrap();
        }
    }
}

/// Internal data structure for tracking the current job status. This is shared by worker closures
/// via `Arc` so that the workers can report to the pool that it started/finished a job.
#[derive(Debug, Default)]
struct ThreadPoolInner {
    job_count: Mutex<usize>,
    empty_condvar: Condvar,
}

impl ThreadPoolInner {
    /// Increment the job count.
    fn start_job(&self) {
        let mut job_count_guard = self.job_count.lock().unwrap();
        *job_count_guard += 1;
    }

    /// Decrement the job count.
    fn finish_job(&self) {
        let mut job_count_guard = self.job_count.lock().unwrap();
        *job_count_guard -= 1;
        if *job_count_guard == 0 {
            self.empty_condvar.notify_one();
        }
    }

    /// Wait until the job count becomes 0.
    ///
    /// NOTE: We can optimize this function by adding another field to `ThreadPoolInner`, but let's
    /// not care about that in this homework.
    fn wait_empty(&self) {
        let mut job_count_guard = self.job_count.lock().unwrap();
        while *job_count_guard != 0 {
            job_count_guard = self.empty_condvar.wait(job_count_guard).unwrap();
        }
    }
}

/// Thread pool.
#[derive(Debug)]
pub struct ThreadPool {
    _workers: Vec<Worker>,
    job_sender: Option<Sender<Job>>,
    pool_inner: Arc<ThreadPoolInner>,
}

impl ThreadPool {
    /// Create a new ThreadPool with `size` threads.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    pub fn new(size: usize) -> Self {
        assert!(size > 0);
        let (sender, receiver) = unbounded();
        let pool_inner = Arc::new(ThreadPoolInner {
            job_count: Mutex::new(0),
            empty_condvar: Condvar::new(),
        });
        let mut pool = ThreadPool {
            _workers: Vec::with_capacity(size),
            job_sender: Some(sender),
            pool_inner: Arc::clone(&pool_inner),
        };
        for i in 0..size {
            pool._workers
                .push(Worker::new(i, receiver.clone(), Arc::clone(&pool_inner)));
        }
        pool
    }

    /// Execute a new job in the thread pool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.pool_inner.start_job();
        self.job_sender
            .as_ref()
            .unwrap()
            .send(Job(Box::new(f)))
            .unwrap();
    }

    /// Block the current thread until all jobs in the pool have been executed.
    ///
    /// NOTE: This method has nothing to do with `JoinHandle::join`.
    pub fn join(&self) {
        self.pool_inner.wait_empty();
    }
}

impl Drop for ThreadPool {
    /// When dropped, all worker threads' `JoinHandle` must be `join`ed. If the thread panicked,
    /// then this function should panic too.
    fn drop(&mut self) {
        drop(self.job_sender.take());
    }
}
