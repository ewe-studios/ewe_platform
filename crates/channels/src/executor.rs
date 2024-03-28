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
    thread,
    time::Duration,
    usize,
};
use thiserror::Error;

use crate::mspc;

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
            .expect("failed to send resolved notication")
    }
}

const DEFAULT_EXECUTOR_SERVICE_SLEEP: u64 = 300;

pub fn create<E: Send + 'static>() -> (ExecutionService<E>, Executor<E>) {
    create_with_sleep_timeout(DEFAULT_EXECUTOR_SERVICE_SLEEP)
}

pub fn create_with_sleep_timeout<E: Send + 'static>(
    sleep_in_millisecond: u64,
) -> (ExecutionService<E>, Executor<E>) {
    let (sender, receiver) = async_channel::unbounded::<Arc<Task<E>>>();
    let (task_completed_sender, task_completed_receiver) = async_channel::unbounded::<()>();

    (
        ExecutionService {
            completed_notification: task_completed_receiver,
            sleep_in_millisecond,
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

    #[error("executor has no tasks")]
    NoTasks,
}

pub type ExecutorResult<E> = anyhow::Result<E, ExecutorError>;

pub struct ExecutionService<E: Send + 'static> {
    sleep_in_millisecond: u64,
    receiver: async_channel::Receiver<Arc<Task<E>>>,
    completed_notification: async_channel::Receiver<()>,
}

// default capacity allocated within executioner service.
const DEFAULT_TASK_PENDING_CAPACITY: usize = 10;

impl<E: Send + 'static> Drop for ExecutionService<E> {
    fn drop(&mut self) {
        self.close();
    }
}

impl<E: Send + 'static> ExecutionService<E> {
    pub fn close(&self) {
        self.receiver.close();
        self.completed_notification.close();
    }

    /// [`ExecutionService`].serve_async attempts to resolve all pending tasks in the
    /// queue, wrapping them in a waker ensuring that if they are not readily to be
    /// resolved now then the [`ExecutionService`] will be notified once they are resolved
    /// and ready asynchronously.
    ///
    /// This method is more suitable for non-blocking, async environments or environments we
    /// are guaranteed to live-long enough for the tasks to be resolved in the background
    /// and re-queued.
    pub async fn serve_async(&mut self) -> ExecutorResult<()> {
        if self.receiver.is_empty() {
            return ExecutorResult::Err(ExecutorError::NoTasks);
        }

        if let Err(_) = self.serve_and_capture_pending() {
            return ExecutorResult::Err(ExecutorError::Decommission);
        }

        return Ok(());
    }

    /// [`ExecutionService`].serve_until_completed blocks the current thread until
    /// all pending tasks in the task channels have fully resolved.
    /// This means all unresolved tasks even if async will
    /// still get re-queued for processing until they all
    /// completes via checking their respective futures.
    ///
    /// This means this blocks the current thread for how-ever long any of the
    /// tasks takes to complete and thoughtful consideration should be taken
    /// e.g sending this into another thread or web worker in the browser
    /// to ensure the main thread is never blocked.
    ///
    /// You also can use the [`ExecutionService`].single_serve method which runs all
    /// execution onces and rely's entirely on the completed futures to notify it when
    /// they are ready and completed via the future waker (see [`Task`]).
    pub fn serve_until_completed(&mut self) -> ExecutorResult<()> {
        loop {
            if self.receiver.is_empty() {
                break;
            }

            let pending_tasks_result = self.serve_and_capture_pending();
            if pending_tasks_result.is_err() {
                return ExecutorResult::Err(ExecutorError::Decommission);
            }

            let mut pending_tasks = pending_tasks_result.unwrap();
            // if last loop found that tasks were still not finished, then re-queue them.
            if pending_tasks.len() != 0 {
                while let Some(task) = pending_tasks.pop() {
                    task.task_sender
                        .try_send(task.clone())
                        .expect("Failed to resend task into queue")
                }

                thread::sleep(Duration::from_millis(self.sleep_in_millisecond));
            }
        }

        return Ok(());
    }

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
                // Note: this will cause potentially double queue'ing of a task, say
                // the loop runs to completion and the task was not completed and not pending
                // the waker will still queue the task up but since its now considered completed
                // it will just skip this portion.
                //
                // I do not think yet we need to optimize that yet.
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

    /// serve_forever provides a listening that continously monitors
    /// for when any tasks signals its ready for resolving, kickstarting
    /// necessary calls to resolve the tasks.
    ///
    /// WARNING: Always run this method in it's own thread as it blocks forever
    /// until the underyling execution service is dropped. If executed on
    /// the main thread then the main thread will be blocked.
    pub fn serve_forever(&self) {
        loop {
            if let Err(_) = self.serve_and_capture_pending() {
                return;
            }

            let res = self.completed_notification.recv_blocking();
            if res.is_err() {
                if self.completed_notification.is_closed() {
                    return;
                }
            }
        }
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
        receiver_fn: impl FnOnce(mspc::Result<E>) -> Fut + 'static + Send,
    ) where
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

        self.sender
            .try_send(task)
            .expect("Failed to send tasks into unbounded channel")
    }

    // schedules a task for completion without dependence on a channel
    // get data. This is useful for work that is independent of
    // some underlying response from another work or processes.
    //
    // The focus is on the future itself and it's compeleness.
    //
    pub fn spawn(&self, fut: impl Future<Output = ()> + 'static + Send) {
        let box_future = Box::pin(fut);
        let task = Arc::new(Task {
            task_sender: self.sender.clone(),
            handler: sync::Mutex::new(Some(box_future)),
            ready_notification: self.completed_notification.clone(),
        });

        self.sender
            .try_send(task)
            .expect("Failed to send tasks into unbounded channel")
    }
}

#[cfg(test)]
mod tests {
    use std::{sync, thread, time::Duration};

    use crate::{executor, mspc};

    #[test]
    fn can_execute_a_task_without_an_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create::<String>();

        let (mut sr, rr) = mspc::create::<String>();
        executor.schedule(rr.clone(), move |item| async move {
            sender
                .async_send(item.unwrap())
                .await
                .expect("to have sent message")
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.serve_until_completed(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[test]
    fn can_execute_work_without_a_receiver_and_no_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create::<String>();

        executor.spawn(async move {
            sender.try_send(String::from("new text")).unwrap();
        });

        assert!(matches!(
            servicer.serve_until_completed(),
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_a_task_with_a_thread_runtime_and_serve_forever() {
        let (servicer, executor) = executor::create();
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut sr, rr) = mspc::create::<String>();

        let servicer_arc = sync::Arc::new(servicer);

        let threaded_servicer = servicer_arc.clone();
        let thread_handle = thread::spawn(move || {
            threaded_servicer.serve_forever();
        });

        executor.schedule(rr, move |item| async move {
            thread::sleep(Duration::from_millis(500));

            sender
                .async_send(item.unwrap())
                .await
                .expect("should have sent result");
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        // expect to receive from second channel
        let recv_message = receiver.block_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);

        servicer_arc.close();

        thread_handle
            .join()
            .expect("should have finished successfully");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_a_task_with_an_async_runtime() {
        let (mut servicer, executor) = executor::create();
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut sr, rr) = mspc::create::<String>();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            executor.schedule(rr, move |item| async move {
                sender
                    .async_send(item.unwrap())
                    .await
                    .expect("should have sent result");
            });
        })
        .await
        .expect("should have completed");

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.serve_until_completed(),
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

            executor.spawn(async move {
                sender.try_send(String::from("new text")).unwrap();
            });
        })
        .await
        .expect("should have completed");

        assert!(matches!(
            servicer.serve_until_completed(),
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

        executor.schedule(rr.clone(), move |item| async {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                sender
                    .async_send(item.unwrap())
                    .await
                    .expect("should have sent result");
            })
            .await
            .expect("should have completed");
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.serve_until_completed(),
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

        executor.schedule(rr.clone(), move |item| async {
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
        });

        // send on first channel
        drop(sr);
        drop(receiver);

        assert!(matches!(
            servicer.serve_until_completed(),
            executor::ExecutorResult::Ok(())
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_with_async_serve_with_async_runtime() {
        let (mut sender, mut receiver) = mspc::create::<String>();

        let (mut servicer, executor) = executor::create();

        let (mut sr, rr) = mspc::create::<String>();

        executor.schedule(rr.clone(), move |item| async {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                sender
                    .async_send(item.unwrap())
                    .await
                    .expect("should have sent result");
            })
            .await
            .expect("should have completed");
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(
            servicer.serve_async().await,
            executor::ExecutorResult::Ok(())
        ));

        // expect to receive from second channel
        let recv_message = receiver.block_receive().unwrap();

        assert_eq!(String::from("new text"), recv_message);
    }
}
