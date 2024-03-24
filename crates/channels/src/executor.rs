// Crate implementing the Engineering Principles of Executors

use crossbeam::channel;
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

use crate::channels;

struct Task<E: Send + 'static> {
    receiver: channels::ReceiveChannel<E>,
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
        print!("Is empty: {}\n", self.receiver.is_empty());
        while !self.receiver.is_empty() {
            let res = self.receiver.try_recv();
            print!("Not empty: {}\n", self.receiver.is_empty());
            if res.is_err() {
                return ExecutorResult::Err(ExecutorError::Decommission);
            }

            print!("Execute task: {}\n", self.receiver.is_empty());
            // collect the message, check the receiver has message,
            // if not, put back the task into the channel else
            // call the function accordingly with a spawn

            let task = res.unwrap();

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
    pub fn schedule(
        &self,
        receiver: channels::ReceiveChannel<E>,
        fut: impl Future<Output = ()> + 'static + Send,
    ) {
        let task = Arc::new(Task {
            receiver,
            task_sender: self.sender.clone(),
            handler: sync::Mutex::new(Some(Box::pin(fut))),
        });

        self.sender
            .send(task)
            .expect("Failed to send tasks into unbounded channel")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{channels, executor};

    #[test]
    fn can_execute_a_task_with_an_execution() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create();

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
}
