//! JavaScript runtime wrapper around Boa with microtask and macrotask scheduling.

use crate::error::JsError;
use crate::job_executor::BoundedJobExecutor;
use boa_engine::{Context, Source};
use std::collections::VecDeque;
use std::rc::Rc;

/// Type alias for an asynchronous or scheduled JavaScript macrotask callback.
pub type JsTask = Box<dyn FnOnce(&mut Context) + Send + 'static>;

/// Maximum accepted size for a script evaluated through [`JsRuntime::eval`].
///
/// Scripts larger than this are rejected before parsing so a hostile page
/// cannot force unbounded parser and memory work.
pub const MAX_SCRIPT_BYTES: usize = 4 * 1024 * 1024;

/// JavaScript execution environment managing ECMAScript context, microtasks, and macrotasks.
pub struct JsRuntime {
    /// Inner Boa engine context.
    pub context: Context,
    /// FIFO macrotask queue for timers and asynchronous events.
    task_queue: VecDeque<JsTask>,
}

impl JsRuntime {
    /// Creates a new `JsRuntime` instance with a default ECMAScript context.
    ///
    /// The context uses a [`BoundedJobExecutor`] so that runaway microtask
    /// chains cannot starve the event loop forever.
    #[must_use]
    pub fn new() -> Self {
        let context = boa_engine::context::ContextBuilder::new()
            .job_executor(Rc::new(BoundedJobExecutor::new()))
            .build()
            .expect("a default Boa context cannot fail to construct");
        Self {
            context,
            task_queue: VecDeque::new(),
        }
    }

    /// Evaluates a JavaScript source code string and drains any generated microtasks.
    ///
    /// # Errors
    ///
    /// Returns a `JsError` if the script exceeds [`MAX_SCRIPT_BYTES`], if
    /// evaluation fails, or if microtasks produce an unhandled rejection.
    pub fn eval(&mut self, source: &str) -> Result<String, JsError> {
        if source.len() > MAX_SCRIPT_BYTES {
            return Err(JsError::ScriptTooLarge(source.len(), MAX_SCRIPT_BYTES));
        }

        let result = self
            .context
            .eval(Source::from_bytes(source.as_bytes()))
            .map(|val| val.display().to_string())
            .map_err(|err| JsError::EvaluationError(err.to_string()))?;

        self.drain_microtasks()?;
        Ok(result)
    }

    /// Appends a new macrotask closure to the task queue.
    pub fn enqueue_task<F>(&mut self, task: F)
    where
        F: FnOnce(&mut Context) + Send + 'static,
    {
        self.task_queue.push_back(Box::new(task));
    }

    /// Drains all pending Promise reactions and microtask jobs.
    ///
    /// # Errors
    ///
    /// Returns a `JsError` if a microtask job fails.
    pub fn drain_microtasks(&mut self) -> Result<(), JsError> {
        if let Err(err) = self.context.run_jobs() {
            return Err(JsError::EventLoopError(err.to_string()));
        }
        Ok(())
    }

    /// Executes the next scheduled macrotask followed by all queued microtasks.
    ///
    /// Returns `Ok(true)` if a task was processed, or `Ok(false)` if the task queue was empty.
    ///
    /// # Errors
    ///
    /// Returns a `JsError` if microtask execution fails.
    pub fn step(&mut self) -> Result<bool, JsError> {
        if let Some(task) = self.task_queue.pop_front() {
            task(&mut self.context);
            self.drain_microtasks()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Runs all queued macrotasks and microtasks until the task queue is completely empty.
    ///
    /// # Errors
    ///
    /// Returns a `JsError` if any step in the event loop fails.
    pub fn run_until_idle(&mut self) -> Result<(), JsError> {
        while self.step()? {}
        Ok(())
    }

    /// Returns the number of pending tasks in the macrotask queue.
    #[must_use]
    pub fn pending_task_count(&self) -> usize {
        self.task_queue.len()
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new()
    }
}
