mod compatibility;
mod graph;
mod workers;

use std::sync::mpsc::Receiver;
use std::time::Duration;

fn receive<T>(receiver: &Receiver<T>) -> T {
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the worker must send before the test deadline")
}

fn check_shared<T: Send + Sync>(_value: &T) {}
