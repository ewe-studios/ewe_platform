use std::{
    sync::{Arc, Mutex},
    thread,
};
use std::time::Duration;

use foundation_core::synca::mpp;
use foundation_core::valtron::{block_on, get_pool, FnReady, NoSpawner, NotificationItem, Stream, TaskIterator, TaskStatus, valtron_test};
use rand::RngCore;
use serial_test::serial;
use tracing_test::traced_test;

struct DCounter(usize, Arc<Mutex<Vec<usize>>>);

impl DCounter {
    pub fn new(val: usize, list: Arc<Mutex<Vec<usize>>>) -> Self {
        Self(val, list)
    }
}

impl TaskIterator for DCounter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next_status(
        &mut self,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let mut items = self.1.lock().unwrap();
        let item_size = items.len();

        if item_size == self.0 {
            return None;
        }

        items.push(item_size);
        let new_len = items.len();

        Some(TaskStatus::Ready(new_len))
    }
}

struct Counter(usize, Arc<Mutex<Vec<usize>>>, mpp::Sender<()>);

impl Counter {
    pub fn new(val: usize, list: Arc<Mutex<Vec<usize>>>) -> (Self, mpp::Receiver<()>) {
        let (sender, receiver) = mpp::bounded(10);
        (Counter(val, list, sender), receiver)
    }
}

impl TaskIterator for Counter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next_status(
        &mut self,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        tracing::debug!("Counter Task is running");

        let result = {
            let mut items = self.1.lock().unwrap();
            let item_size = items.len();

            if item_size == self.0 {
                None // signal "done" — will send after lock release
            } else {
                items.push(item_size);
                Some(TaskStatus::Ready(items.len()))
            }
        }; // lock released here

        match result {
            None => {
                tracing::debug!("Sending signal with sender");
                self.2.send(()).expect("send signal");
                None
            }
            some => some,
        }
    }
}

#[derive(Default)]
struct PanicCounter;

impl TaskIterator for PanicCounter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next_status(
        &mut self,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        tracing::debug!("PanicCounter Task is running");
        panic!("Bad stuff");
    }
}


#[test]
#[serial]
#[traced_test]
#[valtron_test(threads = 2)]
fn can_queue_and_complete_task_with_iterator() {
    let seed = rand::rng().next_u64();

    let shared_list = Arc::new(Mutex::new(Vec::new()));
    let counter = DCounter::new(5, shared_list.clone());

    let iter = get_pool()
        .spawn()
        .with_task(counter)
        .schedule_iter(Duration::from_nanos(50))
        .expect("should deliver task");

    let complete: Vec<usize> = iter
        .map(|item| match item {
            NotificationItem::Ready(inner) => match inner {
                TaskStatus::Ready(value) => Some(value),
                _ => None,
            },
            NotificationItem::None => None,
        })
        .take_while(|t: &Option<usize>| t.is_some())
        .map(|t: Option<usize>| t.unwrap())
        .collect();

    assert_eq!(complete, vec![1, 2, 3, 4, 5]);
}

#[test]
#[serial]
#[traced_test]
#[valtron_test(threads = 2)]
fn can_queue_use_stream_iterator_from_task_iterator() {
    let seed = rand::rng().next_u64();

    let shared_list = Arc::new(Mutex::new(Vec::new()));
    let counter = DCounter::new(5, shared_list.clone());

    let iter = get_pool()
        .spawn()
        .with_task(counter)
        .stream_iter_with_config(
            Duration::from_nanos(20), // wait_cycle (used as park_duration)
            10,                                  // max_turns
        )
        .expect("should deliver task");

    let complete: Vec<usize> = iter
        .map(|item| match item {
            Stream::Next(value) => Some(value),
            _ => None,
        })
        .take_while(|t| t.is_some())
        .map(|t| t.unwrap())
        .collect();

    assert_eq!(complete, vec![1, 2, 3, 4, 5]);
}


#[test]
#[serial]
#[traced_test]
fn can_finish_even_when_task_panics() {
    let seed = rand::rng().next_u64();

    let handler_kill = thread::spawn(move || {
        tracing::debug!("Waiting for kill signal");
        thread::sleep(Duration::from_secs(5));
        tracing::debug!("Got kill signal");
        get_pool().kill();
        tracing::debug!("Closing thread");
    });

    let (task_sent_sender, task_sent_receiver) = mpp::bounded(1);
    let _guard = block_on(seed, None, |pool| {
        pool.spawn()
            .with_task(PanicCounter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {item:?}");
            })))
            .schedule()
            .expect("should deliver task");
        task_sent_sender.send(()).expect("deliver message");
    });

    task_sent_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("should have spawned task");

    tracing::info!("Wait for thread to die");
    handler_kill.join().expect("should finish");
    tracing::info!("Wait for thread to die");
}

#[test]
#[serial]
#[traced_test]
fn can_queue_and_complete_task() {
    let seed = rand::rng().next_u64();

    let shared_list = Arc::new(Mutex::new(vec![]));
    let (counter, receiver) = Counter::new(5, shared_list.clone());

    let handler_kill = thread::spawn(move || {
        tracing::debug!("Waiting for kill signal");
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("receive signal");
        tracing::debug!("Got kill signal");
        get_pool().kill();
        tracing::debug!("Closing thread");
    });

    let (task_sent_sender, task_sent_receiver) = mpp::bounded(1);
    let _guard = block_on(seed, None, |pool| {
        tracing::debug!("Spawning new task into pool");
        pool.spawn()
            .with_task(counter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {item:?}");
            })))
            .schedule()
            .expect("should deliver task");
        tracing::debug!("Task spawned");
        task_sent_sender.send(()).expect("deliver message");
    });

    task_sent_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("should have spawned task");

    handler_kill.join().expect("should finish");

    assert_eq!(shared_list.lock().unwrap().clone(), vec![0, 1, 2, 3, 4]);
}
