use foundation_testing::scenarios::Barrier;
use std::sync::Arc;
use std::thread;

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
