#![cfg(feature = "global_gen")]

use std::collections::HashSet;
use std::sync::mpsc;
use std::thread;

#[test]
fn generates_no_ids_sharing_same_timestamp_and_counters_under_multithreading(
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    for _ in 0..4 {
        let tx = tx.clone();
        thread::Builder::new()
            .spawn(move || {
                for _ in 0..10_000 {
                    tx.send(foundation_compact::ids::new()).unwrap();
                }
            })
            .map_err(|err| format!("failed to spawn thread: {:?}", err))?;
    }
    drop(tx);

    let mut s = HashSet::new();
    while let Ok(e) = rx.recv() {
        s.insert((e.timestamp(), e.counter_hi(), e.counter_lo()));
    }

    assert_eq!(s.len(), 4 * 10_000);
    Ok(())
}
