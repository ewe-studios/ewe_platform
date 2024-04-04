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
    usize,
};
use thiserror::Error;

use crate::mspc;

// default capacity allocated within executioner service.
const DEFAULT_TASK_PENDING_CAPACITY: usize = 10;

struct Task<E: Send + 'static> {
    handler: sync::Mutex<Option<future::BoxFuture<'static, ()>>>,

    // we need to be able to re-queue/re-send the task if the thread gets
    // woken up. Basically we just send it back into the channel for reprocessing.
    task_sender: async_channel::Sender<Arc<Task<E>>>,
    ready_notification: async_channel::Sender<()>,
}

impl<E: Send + 'static> ArcWake for Task<E> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let cloned_task = arc_self.clone();
        arc_self
            .task_sender
            .try_send(cloned_task)
            .expect("Failed to resend task into executor channel");
        arc_self
            .ready_notification
            .try_send(())
            .expect("failed to send resolved notication");
        println!("Sent wake action");
    }
}

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

    pub fn task_ready(&self) -> async_channel::Receiver<()> {
        self.completed_notification.clone()
    }

    pub async fn schedule_serve_async(&mut self) -> ExecutorResult<()> {
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
    /// will have the calls to Future.poll() behave like a synchronous system where it simply
    /// blocks on the current thread till the future is completed, then moving on sequentially.
    ///
    /// Whilst under async runtimes like Tokio, async-std, tasks that are not compeleted immediately
    /// will signal alter via the Waker re-adding the tasks for processing.
    ///
    /// To automtically have these re-processed, please use the serve_forever method.
    ///
    /// WARNING: the completion of the future this function returns does not mean all
    /// the task are completed. It simply means they have being scheduled for completion
    /// and that completion might take a while, even past when this function's future completes.
    /// All the async function does is schedule them for completion
    pub fn schedule_serve(&mut self) -> ExecutorResult<()> {
        if self.receiver.is_empty() {
            return ExecutorResult::Err(ExecutorError::NoTasks);
        }

        if let Err(_) = self.serve_and_capture_pending() {
            return ExecutorResult::Err(ExecutorError::Decommission);
        }

        return Ok(());
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
    fn serve_and_capture_pending(&self) -> ExecutorResult<Vec<Arc<Task<E>>>> {
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

                if future.as_mut().poll(context).is_pending() {
                    // put back the future since its still pending
                    *future_container = Some(future);

                    pending_tasks.push(task.clone());
                    continue;
                }
            }
        }

        ExecutorResult::Ok(pending_tasks)
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
            let received = mutable_receiver.async_receive().await;
            receiver_fn(received).await
        };

        let box_future = Box::pin(captured_async_fn);
        let task = Arc::new(Task {
            task_sender: self.sender.clone(),
            handler: sync::Mutex::new(Some(box_future)),
            ready_notification: self.completed_notification.clone(),
        });

        match self.sender.try_send(task) {
            Ok(_) => Ok(()),
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
    pub fn spawn(&self, fut: impl Future<Output = ()> + 'static + Send) -> ExecutorResult<()> {
        let box_future = Box::pin(fut);
        let task = Arc::new(Task {
            task_sender: self.sender.clone(),
            handler: sync::Mutex::new(Some(box_future)),
            ready_notification: self.completed_notification.clone(),
        });

        match self.sender.try_send(task) {
            Ok(_) => Ok(()),
            Err(async_channel::TrySendError::Closed(_)) => Err(ExecutorError::Decommission),
            Err(async_channel::TrySendError::Full(_)) => Err(ExecutorError::ChannelFull),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{executor, mspc};

    #[test]
    fn can_execute_a_task_without_an_async_runtime_with_scheduled_serve() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create::<String>();

        let (mut sr, rr) = mspc::create::<String>();

        let mut sender_clone = sender.clone();
        executor
            .schedule(rr.clone(), move |item| async move {
                sender_clone
                    .async_send(item.unwrap())
                    .await
                    .expect("to have sent message")
            })
            .expect("should have scheduled task");

        executor
            .spawn(async move {
                sender
                    .async_send(String::from("second"))
                    .await
                    .expect("to have sent message")
            })
            .expect("should have scheduled task");

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.block_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);

        let recv_message = receiver.block_receive().unwrap();
        assert_eq!(String::from("second"), recv_message);
    }

    #[test]
    fn can_execute_a_task_without_an_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create::<String>();

        let (mut sr, rr) = mspc::create::<String>();
        executor
            .schedule(rr.clone(), move |item| async move {
                sender
                    .async_send(item.unwrap())
                    .await
                    .expect("to have sent message")
            })
            .expect("should have scheduled task");

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.block_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[test]
    fn can_execute_work_without_a_receiver_and_no_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create::<String>();

        executor
            .spawn(async move {
                sender.try_send(String::from("new text")).unwrap();
            })
            .expect("should have scheduled task");

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_a_task_with_an_async_runtime() {
        let (mut servicer, executor) = executor::create();
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut sr, rr) = mspc::create::<String>();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            executor
                .schedule(rr, move |item| async move {
                    sender
                        .async_send(item.unwrap())
                        .await
                        .expect("should have sent result");
                })
                .expect("should have scheduled task");
        })
        .await
        .expect("should have completed");

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_work_without_a_receiver_and_a_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create::<String>();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            executor
                .spawn(async move {
                    sender.try_send(String::from("new text")).unwrap();
                })
                .expect("should have scheduled task");
        })
        .await
        .expect("should have completed");

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execution_in_runtime_while_waiting_for_reciever_lead_to_panic() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create();

        let (mut sr, rr) = mspc::create::<String>();

        executor
            .schedule(rr.clone(), move |item| async {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    sender
                        .async_send(item.unwrap())
                        .await
                        .expect("should have sent result");
                })
                .await
                .expect("should have completed");
            })
            .expect("should have scheduled task");

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.block_receive().unwrap();

        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_with_closed_sender_with_runtime_and_no_panic() {
        let (mut sender, receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create();

        let (sr, rr) = mspc::create::<String>();

        executor
            .schedule(rr.clone(), move |item| async {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if let Ok(value) = item {
                        sender
                            .async_send(value)
                            .await
                            .expect("should have sent result");
                    }
                })
                .await
                .expect("should complete");
            })
            .expect("should have scheduled task");

        // send on first channel
        drop(sr);
        drop(receiver);

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_with_async_serve_with_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create();

        let (mut sr, rr) = mspc::create::<String>();

        executor
            .schedule(rr.clone(), move |item| async {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    sender
                        .async_send(item.unwrap())
                        .await
                        .expect("should have sent result");
                })
                .await
                .expect("should have completed");
            })
            .expect("should have scheduled task");

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.schedule_serve(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.block_receive().unwrap();

        assert_eq!(String::from("new text"), recv_message);
    }
}
