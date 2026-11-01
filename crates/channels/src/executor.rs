// Crate implementing the Engineering Principles of Executors

use async_channel;
use futures::{
    future,
    task::{waker_ref, ArcWake},
    Future,
};
use std::{
    sync::{self, Arc},
    task::Context,
};
use thiserror::Error;

use crate::mspc;

// default capacity allocated within executioner service.
const DEFAULT_TASK_PENDING_CAPACITY: usize = 10;

struct Task<E: Send + 'static> {
    handler: sync::Mutex<Option<future::BoxFuture<'static, ()>>>,

    // we need to be able to re-queue/re-send the task if the thread gets
    // woken up. Basically we just send it back into the channel for reprocessing.
    sender: async_channel::Sender<Arc<Task<E>>>,
    ready_notification: async_channel::Sender<()>,
}

impl<E: Send + 'static> ArcWake for Task<E> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let cloned_task = arc_self.clone();
        arc_self
            .sender
            .try_send(cloned_task)
            .expect("Failed to resend task into executor channel");
        arc_self
            .ready_notification
            .try_send(())
            .expect("failed to send resolved notification");
        println!("Sent wake action");
    }
}

#[must_use]
pub fn create<E: Send + 'static>() -> (ExecutionService<E>, Executor<E>) {
    let (sender, receiver) = async_channel::unbounded::<Arc<Task<E>>>();
    let (task_completed_sender, task_completed_receiver) = async_channel::unbounded::<()>();

    (
        ExecutionService {
            completed_notification: task_completed_receiver,
            receiver,
        },
        Executor {
            completed_notification: task_completed_sender,
            sender,
        },
    )
}

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("executor is no more usable")]
    Decommission,

    #[error("executor requires retry")]
    ChannelFull,

    #[error("executor has no tasks")]
    NoTasks,
}

pub type ExecutorResult<E> = anyhow::Result<E, ExecutorError>;

pub struct ExecutionService<E: Send + 'static> {
    completed_notification: async_channel::Receiver<()>,
    receiver: async_channel::Receiver<Arc<Task<E>>>,
}

impl<E: Send + 'static> Drop for ExecutionService<E> {
    fn drop(&mut self) {
        self.close();
    }
}

impl<E: Send + 'static> Clone for ExecutionService<E> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
            completed_notification: self.completed_notification.clone(),
        }
    }
}

impl<E: Send + 'static> ExecutionService<E> {
    pub fn close(&self) {
        self.receiver.close();
        self.completed_notification.close();
    }

    #[must_use]
    pub fn task_ready(&self) -> async_channel::Receiver<()> {
        self.completed_notification.clone()
    }

    /// # Errors
    ///
    /// Returns an error if there are no tasks to schedule or if the executor is decommissioned.
    pub fn schedule_serve_async(&mut self) -> ExecutorResult<()> {
        self.schedule_serve()
    }

    /// [`ExecutionService::schedule_serve`] attempts to resolve all pending tasks in the
    /// queue, wrapping them in a waker ensuring that if they are not readily to be
    /// resolved now then the [`ExecutionService`] will be notified once they are resolved
    /// and ready asynchronously.
    ///
    /// This method is more suitable for non-blocking, async environments or environments we
    /// are guaranteed to live-long enough for the tasks to be resolved in the background
    /// and re-queued.
    ///
    /// Something to note is that when executed in an environment without an async runtime
    /// will have the calls to `Future.poll()` behave like a synchronous system where it simply
    /// blocks on the current thread till the future is completed, then moving on sequentially.
    ///
    /// Whilst under async runtimes like Tokio, async-std, tasks that are not compeleted immediately
    /// will signal alter via the Waker re-adding the tasks for processing.
    ///
    /// To automtically have these re-processed, please use the `serve_forever` method.
    ///
    /// WARNING: the completion of the future this function returns does not mean all
    /// the task are completed. It simply means they have being scheduled for completion
    /// and that completion might take a while, even past when this function's future completes.
    /// All the async function does is schedule them for completion
    ///
    /// # Errors
    ///
    /// Returns an error if there are no tasks to schedule or if the executor is decommissioned.
    pub fn schedule_serve(&mut self) -> ExecutorResult<()> {
        if self.receiver.is_empty() {
            return ExecutorResult::Err(ExecutorError::NoTasks);
        }

        self.serve_and_capture_pending();

        Ok(())
    }

    // This function triggers processing of every tasks within the execution service.
    //
    // Something to note is that when executed in an environment without an async runtime
    // will have the calls to Future.poll() behave like a synchronous system where it simply
    // blocks on the current thread till the future is completed, then moving on sequentially.
    //
    // Whilst under async runtimes like Tokio, async-std, tasks that are not compeleted immediately
    // will signal alter via the Waker re-adding the tasks for processing.
    //
    // To automtically have these re-processed, please use the serve_forever method.
    fn serve_and_capture_pending(&self) -> Vec<Arc<Task<E>>> {
        let mut pending_tasks = Vec::<Arc<Task<E>>>::with_capacity(DEFAULT_TASK_PENDING_CAPACITY);
        while let Ok(task) = self.receiver.try_recv() {
            // get the future in the task container - we use an option here so we can easily
            // slot back in a future that might not be ready.
            let mut future_container = task.handler.lock().unwrap();

            // without using Option<> here its impossible to take the
            // future and do something with it then return it back in if not
            // ready or completed.
            if let Some(mut future) = future_container.take() {
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&waker);

                if future.as_mut().poll(context).is_ready() {
                    // Future is complete, don't add it back to pending tasks
                } else {
                    // put back the future since its still pending
                    *future_container = Some(future);
                    pending_tasks.push(task.clone());
                }
            }
        }

        pending_tasks
    }
}

pub struct Executor<E: Send + 'static> {
    completed_notification: async_channel::Sender<()>,
    sender: async_channel::Sender<Arc<Task<E>>>,
}

impl<E: Send + 'static> Executor<E> {
    // schedule a task to execute when the receiver has data
    // usually the future here should really get scheduled
    // for polling if it's receiver finally received value.
    //
    // this allows us create inter-dependent work that
    // depends on the readiness of response on a channel.
    /// # Errors
    ///
    /// Returns an error if the executor is decommissioned or the task queue is full.
    pub fn schedule<Fut>(
        &self,
        receiver: mspc::ReceiveChannel<E>,
        receiver_fn: impl FnOnce(mspc::ChannelResult<E>) -> Fut + 'static + Send,
    ) -> ExecutorResult<()>
    where
        Fut: future::Future<Output = ()> + Send,
    {
        let captured_async_fn = async move {
            let mut mutable_receiver = receiver.clone();
            let result = mutable_receiver.async_receive().await;
            receiver_fn(result).await;
        };

        let box_future = Box::pin(captured_async_fn);
        let task = Arc::new(Task {
            sender: self.sender.clone(),
            handler: sync::Mutex::new(Some(box_future)),
            ready_notification: self.completed_notification.clone(),
        });

        match self.sender.try_send(task) {
            Ok(()) => Ok(()),
            Err(async_channel::TrySendError::Closed(_)) => Err(ExecutorError::Decommission),
            Err(async_channel::TrySendError::Full(_)) => Err(ExecutorError::ChannelFull),
        }
    }

    // schedules a task for completion without dependence on a channel
    // get data. This is useful for work that is independent of
    // some underlying response from another work or processes.
    //
    // The focus is on the future itself and it's compeleness.
    //
    /// # Errors
    ///
    /// Returns an error if the executor is decommissioned or the task queue is full.
    pub fn spawn(&self, fut: impl Future<Output = ()> + 'static + Send) -> ExecutorResult<()> {
        let box_future = Box::pin(fut);
        let task = Arc::new(Task {
            sender: self.sender.clone(),
            handler: sync::Mutex::new(Some(box_future)),
            ready_notification: self.completed_notification.clone(),
        });

        match self.sender.try_send(task) {
            Ok(()) => Ok(()),
            Err(async_channel::TrySendError::Closed(_)) => Err(ExecutorError::Decommission),
            Err(async_channel::TrySendError::Full(_)) => Err(ExecutorError::ChannelFull),
        }
    }
}

