// Crate implementing the Engineering Principles of Executors

use crossbeam::channel;
use futures::{
    future,
    task::{waker_ref, ArcWake},
    Future,
};
use std::{
    borrow::BorrowMut,
    ops::Deref,
    sync::{self, Arc},
    task::Context,
};
use thiserror::Error;

use crate::channels;

struct Task<E: Send + 'static> {
    receiver: sync::Mutex<Option<channels::ReceiveChannel<E>>>,
    handler: sync::Mutex<Option<future::BoxFuture<'static, ()>>>,

    // we need to be able to re-queue/re-send the task if the thread gets
    // woken up. Basically we just send it back into the channel for reprocessing.
    task_sender: channel::Sender<Arc<Task<E>>>,
}

impl<E: Send + 'static> ArcWake for Task<E> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let cloned_task = arc_self.clone();
        arc_self
            .task_sender
            .send(cloned_task)
            .expect("Failed to resend task into executor channel");
    }
}

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("executor is no more usable")]
    Decommission,
}

pub type ExecutorResult<E> = anyhow::Result<E, ExecutorError>;

pub struct ExecutionService<E: Send + 'static> {
    receiver: channel::Receiver<Arc<Task<E>>>,
}

pub struct Executor<E: Send + 'static> {
    sender: channel::Sender<Arc<Task<E>>>,
}

pub fn create<E: Send + 'static>() -> (ExecutionService<E>, Executor<E>) {
    let (sender, receiver) = channel::unbounded::<Arc<Task<E>>>();

    (ExecutionService { receiver }, Executor { sender })
}

impl<E: Send + 'static> ExecutionService<E> {
    pub fn serve(&mut self) -> ExecutorResult<()> {
        while !self.receiver.is_empty() {
            let res = self.receiver.try_recv();
            if res.is_err() {
                return ExecutorResult::Err(ExecutorError::Decommission);
            }

            // collect the message, check the receiver has message,
            // if not, put back the task into the channel else
            // call the function accordingly with a spawn

            let task = res.unwrap();

            // if the task has a receiver channel it is waiting for
            // and channel is empty then re-add task to sender and
            // end continuation here.
            //
            // Task sometimes should only run when receiver finally has
            // something.
            let mut receiver_slot = task.receiver.lock().unwrap();
            if let Some(receiver) = receiver_slot.take() {
                // if receiver is still empty then put back into slot for next run.
                // once the receiver has data, we simply do not re-add it to indicate the
                // future is ready for scheduling.
                if receiver.is_empty().unwrap() {
                    *receiver_slot = Some(receiver);
                }
            }

            // get the future in the task container - we use an option here so we can easily
            // slot back in a future that might not be ready.
            let mut future_container = task.handler.lock().unwrap();

            // without using Option<> here its impossible to take the
            // future and do something with it then return it back in if not
            // ready or completed.
            if let Some(mut future) = future_container.take() {
                // create the waker and context
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&waker);

                if future.as_mut().poll(context).is_pending() {
                    // put back the future since its still pending
                    *future_container = Some(future)
                }
            }
        }

        return Ok(());
    }
}

impl<E: Send + 'static> Executor<E> {
    // schedule a task to execute when the receiver has data
    // usually the future here should really get scheduled
    // for polling if it's receiver finally received value.
    //
    // this allows us create inter-dependent work that
    // depends on the readiness of response on a channel.
    pub fn schedule(
        &self,
        receiver: channels::ReceiveChannel<E>,
        fut: impl Future<Output = ()> + 'static + Send,
    ) {
        let box_future = Box::pin(fut);
        let task = Arc::new(Task {
            task_sender: self.sender.clone(),
            receiver: sync::Mutex::new(Some(receiver)),
            handler: sync::Mutex::new(Some(box_future)),
        });

        self.sender
            .send(task)
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
            receiver: sync::Mutex::new(None),
            handler: sync::Mutex::new(Some(box_future)),
        });

        self.sender
            .send(task)
            .expect("Failed to send tasks into unbounded channel")
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use crate::{channels, executor};

    #[tokio::test]
    async fn can_execute_a_task_with_an_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create();

        let (mut sr, mut rr) = channels::create::<String>();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            executor.schedule(rr.clone(), async move {
                sender.try_send(rr.block_receive().unwrap()).unwrap();
            });
        })
        .await;

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[test]
    fn can_execute_a_task_without_an_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>();

        let (mut sr, mut rr) = channels::create::<String>();
        executor.schedule(rr.clone(), async move {
            sender.try_send(rr.try_receive().unwrap()).unwrap();
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[test]
    fn can_execute_work_without_a_receiver_and_no_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>();

        executor.spawn(async move {
            sender.try_send(String::from("new text")).unwrap();
        });

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test]
    async fn can_execute_work_without_a_receiver_and_a_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            executor.spawn(async move {
                sender.try_send(String::from("new text")).unwrap();
            });
        })
        .await;

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }
}
