//! Simple utility for tracking and aborting background tasks spawned with
//! `tokio::spawn`.
//!
//! The current codebase occasionally spawns detached background tasks to fan
//! streams or perform bookkeeping work.  These tasks are effectively
//! "fire-and-forget" and can outlive the object that spawned them, leading to
//! resource leaks or unexpected work continuing after the owning context has
//! been dropped.  `TaskTracker` offers a lightweight way to register each
//! `JoinHandle` so that all outstanding tasks can be aborted deterministically
//! when the tracker is dropped.

use std::sync::Arc;
use std::sync::Mutex;

use tokio::task::JoinHandle;

/// Tracks a collection of [`tokio::spawn`]ed tasks and aborts them on drop.
#[derive(Debug, Clone, Default)]
pub struct TaskTracker {
    inner: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl TaskTracker {
    /// Register a new [`JoinHandle`]. The caller should invoke this immediately
    /// after `tokio::spawn`:
    ///
    /// ```no_run
    /// # use tokio::task::JoinHandle;
    /// # use codex_core::task_tracker::TaskTracker;
    /// # async fn example(tt: TaskTracker) {
    /// let handle: JoinHandle<()> = tokio::spawn(async move { /* … */ });
    /// tt.track(handle);
    /// # }
    /// ```
    pub fn track(&self, handle: JoinHandle<()>) {
        let mut guard = self.inner.lock().expect("poisoned lock");
        guard.push(handle);
    }

    /// Returns true if this is the last strong reference to the inner state.
    pub fn is_last_ref(&self) -> bool {
        Arc::strong_count(&self.inner) == 1
    }

    /// Abort all currently-tracked tasks.  Aborted tasks are removed from the
    /// internal list so that repeated calls are no-ops.
    pub fn abort_all(&self) {
        let mut guard = self.inner.lock().expect("poisoned lock");
        for h in guard.drain(..) {
            h.abort();
        }
    }
}

impl Drop for TaskTracker {
    fn drop(&mut self) {
        // Only abort tasks when this is the last strong reference to the inner state.
        // This prevents premature cancellation when short-lived clones are dropped.
        if Arc::strong_count(&self.inner) == 1 {
            let mut guard = self.inner.lock().expect("poisoned lock");
            for h in guard.drain(..) {
                h.abort();
            }
        }
    }
}
