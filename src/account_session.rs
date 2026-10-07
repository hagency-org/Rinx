//! Account-scoped backend tasks. Closing a scope aborts and drains every task
//! before UI cleanup and before opening the next account's scope.
use std::{
    future::Future,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    sync::oneshot,
    task::{AbortHandle, JoinHandle},
};

static EPOCH: AtomicU64 = AtomicU64::new(0);
static TASKS: LazyLock<Mutex<Tasks>> = LazyLock::new(|| {
    Mutex::new(Tasks {
        open: true,
        tasks: Vec::new(),
    })
});
struct Tasks {
    open: bool,
    tasks: Vec<(AbortHandle, oneshot::Receiver<()>)>,
}
struct Finished(Option<oneshot::Sender<()>>);
impl Drop for Finished {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
pub fn epoch() -> u64 {
    EPOCH.load(Ordering::Acquire)
}

pub(crate) fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let mut scope = TASKS.lock().unwrap();
    scope.tasks.retain(|(handle, _)| !handle.is_finished());
    let (sender, receiver) = oneshot::channel();
    // Capture the guard outside the async body, so even an unpolled aborted task
    // notifies the drain barrier.
    let finished = Finished(Some(sender));
    let permitted = scope.open;
    let handle = crate::sliding_sync::backend_runtime().spawn(async move {
        let _finished = finished;
        if !permitted {
            std::future::pending::<()>().await;
        }
        future.await
    });
    if !scope.open {
        handle.abort();
    }
    scope.tasks.push((handle.abort_handle(), receiver));
    handle
}

pub(crate) async fn close() {
    {
        let mut scope = TASKS.lock().unwrap();
        scope.open = false;
        EPOCH.fetch_add(1, Ordering::AcqRel);
    }
    loop {
        let tasks = std::mem::take(&mut TASKS.lock().unwrap().tasks);
        if tasks.is_empty() {
            break;
        }
        for (handle, _) in &tasks {
            handle.abort();
        }
        for (_, finished) in tasks {
            let _ = finished.await;
        }
    }
}
pub(crate) fn open() {
    TASKS.lock().unwrap().open = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn transition_drains_old_work_and_rejects_launches_until_next_account() {
        open();
        let old_epoch = epoch();
        let (started, running) = oneshot::channel();
        let (late, mut result) = oneshot::channel();
        let old = spawn(async move {
            let _ = started.send(());
            std::future::pending::<()>().await;
            let _ = late.send("old account");
        });
        running.await.unwrap();
        close().await;
        assert!(old.await.unwrap_err().is_cancelled());
        assert!(result.try_recv().is_err());
        assert_ne!(epoch(), old_epoch);
        let rejected = spawn(async { "must not run" });
        close().await;
        assert!(rejected.await.unwrap_err().is_cancelled());
        open();
        assert_eq!(spawn(async { "new account" }).await.unwrap(), "new account");
    }
}
