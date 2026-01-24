use foundation_testing::scenarios::Barrier;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_barrier_simple() {
    println!("Starting barrier test");
    let barrier: Arc<Barrier> = Arc::new(Barrier::new(2));
    let mut handles = vec![];

    for i in 0..2 {
        let barrier_clone = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            println!("Thread {} before barrier", i);
            let is_leader = barrier_clone.wait();
            println!("Thread {} after barrier, leader={}", i, is_leader);
            is_leader
        }));
    }

    // Wait with timeout
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(is_leader) => println!("Thread {} joined, leader={}", i, is_leader),
            Err(e) => panic!("Thread {} panicked: {:?}", i, e),
        }
    }

    println!("Test completed successfully");
}

#[test]
#[ignore = "This test will timeout, for debugging only"]
fn test_barrier_timeout() {
    let barrier: Arc<Barrier> = Arc::new(Barrier::new(3)); // Need 3 threads
    let barrier_clone = Arc::clone(&barrier);

    let handle = thread::spawn(move || {
        println!("Thread waiting on barrier (will timeout)");
        barrier_clone.wait();
    });

    // Give it a second, then timeout
    thread::sleep(Duration::from_secs(2));
    println!("Test timed out as expected (only 1 of 3 threads reached barrier)");
}
