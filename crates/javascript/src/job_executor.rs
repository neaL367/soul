//! A bounded ECMAScript job executor.
//!
//! Boa's default executor drains the job queue until it is empty, which an
//! unbounded promise-reaction chain (e.g. `Promise.resolve().then(f)` where
//! `f` enqueues itself again) would starve forever. This executor processes at
//! most [`MAX_JOBS_PER_DRAIN`] jobs per drain and then fails, leaving the
//! remaining jobs queued for the next drain.

use boa_engine::{
    Context, JsError, JsResult, JsString, JsValue,
    context::time::JsInstant,
    job::{GenericJob, Job, JobExecutor, NativeAsyncJob, PromiseJob, TimeoutJob},
};
use futures_concurrency::future::FutureGroup;
use futures_lite::{StreamExt, future};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Debug;
use std::mem;
use std::rc::Rc;

/// Maximum number of jobs executed by a single [`JobExecutor::run_jobs`] drain.
pub const MAX_JOBS_PER_DRAIN: usize = 4096;

/// A FIFO job executor that bails with an error once the per-drain budget is consumed.
// (field names mirror Boa's own `SimpleJobExecutor`)
#[allow(clippy::struct_field_names)]
#[derive(Default)]
pub struct BoundedJobExecutor {
    promise_jobs: RefCell<VecDeque<PromiseJob>>,
    async_jobs: RefCell<VecDeque<NativeAsyncJob>>,
    timeout_jobs: RefCell<BTreeMap<JsInstant, TimeoutJob>>,
    generic_jobs: RefCell<VecDeque<GenericJob>>,
}

impl BoundedJobExecutor {
    /// Creates a new `BoundedJobExecutor`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn clear(&self) {
        self.promise_jobs.borrow_mut().clear();
        self.async_jobs.borrow_mut().clear();
        self.timeout_jobs.borrow_mut().clear();
        self.generic_jobs.borrow_mut().clear();
    }
}

impl Debug for BoundedJobExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedJobExecutor").finish_non_exhaustive()
    }
}

impl JobExecutor for BoundedJobExecutor {
    fn enqueue_job(self: Rc<Self>, job: Job, context: &mut Context) {
        match job {
            Job::PromiseJob(p) => self.promise_jobs.borrow_mut().push_back(p),
            Job::AsyncJob(a) => self.async_jobs.borrow_mut().push_back(a),
            Job::TimeoutJob(t) => {
                let now = context.clock().now();
                self.timeout_jobs.borrow_mut().insert(now + t.timeout(), t);
            }
            Job::GenericJob(g) => self.generic_jobs.borrow_mut().push_back(g),
            _ => {}
        }
    }

    fn run_jobs(self: Rc<Self>, context: &mut Context) -> JsResult<()> {
        future::block_on(self.run_jobs_async(&RefCell::new(context)))
    }

    #[allow(
        clippy::future_not_send,
        reason = "all our APIs are single-threaded, mirroring Boa itself"
    )]
    async fn run_jobs_async(self: Rc<Self>, context: &RefCell<&mut Context>) -> JsResult<()>
    where
        Self: Sized,
    {
        let mut group = FutureGroup::new();
        let mut budget = MAX_JOBS_PER_DRAIN;
        loop {
            for job in mem::take(&mut *self.async_jobs.borrow_mut()) {
                group.insert(job.call(context));
            }

            // There are no timeout jobs to run IIF there are no jobs to execute right now.
            let no_timeout_jobs_to_run = {
                let now = context.borrow().clock().now();
                !self.timeout_jobs.borrow().iter().any(|(t, _)| &now >= t)
            };

            if self.promise_jobs.borrow().is_empty()
                && self.async_jobs.borrow().is_empty()
                && self.generic_jobs.borrow().is_empty()
                && no_timeout_jobs_to_run
                && group.is_empty()
            {
                break;
            }

            // Count only completed work against the budget: waiting on a
            // pending async job (e.g. a fetch in flight) must not consume it,
            // otherwise a slow network response would always trip the budget.
            if let Some(result) = future::poll_once(group.next()).await {
                if let Some(Err(err)) = result {
                    self.clear();
                    return Err(err);
                }
                budget = self.consume_budget(budget)?;
            }

            {
                let now = context.borrow().clock().now();
                let mut timeouts_borrow = self.timeout_jobs.borrow_mut();
                let mut jobs_to_keep = timeouts_borrow.split_off(&now);
                jobs_to_keep.retain(|_, job| !job.is_cancelled());
                let jobs_to_run = mem::replace(&mut *timeouts_borrow, jobs_to_keep);
                drop(timeouts_borrow);

                for job in jobs_to_run.into_values() {
                    budget = self.consume_budget(budget)?;
                    if let Err(err) = job.call(&mut context.borrow_mut()) {
                        self.clear();
                        return Err(err);
                    }
                }
            }

            let jobs = mem::take(&mut *self.promise_jobs.borrow_mut());
            for job in jobs {
                budget = self.consume_budget(budget)?;
                if let Err(err) = job.call(&mut context.borrow_mut()) {
                    self.clear();
                    return Err(err);
                }
            }

            let jobs = mem::take(&mut *self.generic_jobs.borrow_mut());
            for job in jobs {
                budget = self.consume_budget(budget)?;
                if let Err(err) = job.call(&mut context.borrow_mut()) {
                    self.clear();
                    return Err(err);
                }
            }
            context.borrow_mut().clear_kept_objects();
            future::yield_now().await;
        }

        Ok(())
    }
}

impl BoundedJobExecutor {
    /// Decrements the per-drain job budget, failing once it is exhausted.
    ///
    /// Like Boa's own error path, the remaining queued jobs are cleared so a
    /// runaway chain cannot poison every subsequent drain of the runtime.
    fn consume_budget(&self, budget: usize) -> JsResult<usize> {
        let Some(remaining) = budget.checked_sub(1) else {
            self.clear();
            return Err(JsError::from_opaque(JsValue::from(JsString::from(
                "microtask budget exceeded",
            ))));
        };
        Ok(remaining)
    }
}
