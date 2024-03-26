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
    thread,
    time::Duration,
    usize,
};
use thiserror::Error;

use crate::channels::{self, ReceiveChannel, SendChannel};

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
    sleep_in_millisecond: u64,
    receiver: channel::Receiver<Arc<Task<E>>>,
}

pub struct Executor<E: Send + 'static> {
    sender: channel::Sender<Arc<Task<E>>>,
}

pub fn create<E: Send + 'static>(sleep_in_millisecond: u64) -> (ExecutionService<E>, Executor<E>) {
    let (sender, receiver) = channel::unbounded::<Arc<Task<E>>>();

    (
        ExecutionService {
            sleep_in_millisecond,
            receiver,
        },
        Executor { sender },
    )
}

// default capacity allocated within executioner service.
const DEFAULT_TASK_PENDING_CAPACITY: usize = 10;

impl<E: Send + 'static> ExecutionService<E> {
    pub fn serve(&mut self) -> ExecutorResult<()> {
        let mut pending_tasks = Vec::<Arc<Task<E>>>::with_capacity(DEFAULT_TASK_PENDING_CAPACITY);
        loop {
            // if last loop found that tasks were still not finished, then re-queue them.
            if pending_tasks.len() != 0 {
                while let Some(task) = pending_tasks.pop() {
                    task.task_sender
                        .send(task.clone())
                        .expect("Failed to resend task into queue")
                }

                thread::sleep(Duration::from_millis(self.sleep_in_millisecond));
            }

            if self.receiver.is_empty() {
                break;
            }

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

                let mut has_reciever = false;
                if let Some(mut receiver) = receiver_slot.take() {
                    has_reciever = true;

                    // if receiver is still empty then put back into slot for next run.
                    // once the receiver has data, we simply do not re-add it to indicate the
                    // future is ready for scheduling.
                    let is_empty = receiver.is_empty();
                    if (is_empty.is_ok() && is_empty.unwrap())
                        && !receiver.read_atleast_once().unwrap()
                    {
                        *receiver_slot = Some(receiver);

                        // add back task into queue till channel is ready or
                        // shown to be read e.g channel was read in another thread
                        // hence received data already.
                        pending_tasks.push(task.clone());

                        continue;
                    }
                }

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

                    // if it has a receiver we are watching
                    // make it empty
                    if has_reciever {
                        receiver_slot.take();
                    }
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

    use crate::{
        channels::{self, ChannelError},
        executor,
    };

    #[test]
    fn can_execute_a_task_without_an_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>(100);

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
    fn can_execute_a_task_even_if_receiver_was_read_before_execution_service() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>(100);

        let (mut sr, mut rr) = channels::create::<String>();

        let mut rrs = rr.clone();
        executor.schedule(rr.clone(), async move {
            match rrs.try_receive() {
                Ok(item) => sender.try_send(item),
                Err(_) => return,
            };
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        // empty the receiver
        _ = rr.try_receive();

        // validate the service works as expected
        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // validate receiver was read already
        assert!(rr.read_atleast_once().unwrap());
        assert!(!receiver.read_atleast_once().unwrap());
    }

    #[test]
    fn can_execute_work_without_a_receiver_and_no_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>(100);

        executor.spawn(async move {
            sender.try_send(String::from("new text")).unwrap();
        });

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // expect to receive from second channel
        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_a_task_with_an_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create(100);

        let (mut sr, mut rr) = channels::create::<String>();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            executor.schedule(rr.clone(), async move {
                sender.try_send(rr.try_receive().unwrap()).unwrap();
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_work_without_a_receiver_and_a_async_runtime() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create::<String>(100);

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execution_in_runtime_while_waiting_for_reciever_lead_to_panic() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create(100);

        let (mut sr, mut rr) = channels::create::<String>();

        executor.schedule(rr.clone(), async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                sender.try_send(rr.try_receive().unwrap()).unwrap();
            })
            .await;
        });

        // send on first channel
        sr.try_send(String::from("new text")).unwrap();

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));

        // expect to receive from second channel
        let mut recv_message = receiver.block_receive().unwrap();

        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_execute_with_closed_sender_with_runtime_and_no_panic() {
        let (mut sender, mut receiver) = channels::create::<String>();

        let (mut servicer, mut executor) = executor::create(100);

        let (mut sr, mut rr) = channels::create::<String>();

        executor.schedule(rr.clone(), async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if let Ok(msg) = rr.try_receive() {
                    sender.try_send(msg).unwrap();
                }
            })
            .await;
        });

        // send on first channel
        drop(sr);

        assert!(matches!(servicer.serve(), executor::ExecutorResult::Ok(())));
    }
}
