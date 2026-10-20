//! Tests for `ThreadSafeEntry`, the shared-lock wrapper over `EntryList`.

use foundation_core::synca::{Entry, EntryList, ThreadSafeEntry};
use std::sync::Arc;
use std::thread;

#[test]
fn test_thread_safe_entry_new() {
    let safe_list: ThreadSafeEntry<usize> = ThreadSafeEntry::new();
    assert_eq!(0, safe_list.active_slots());
}

#[test]
fn test_thread_safe_entry_default() {
    let safe_list: ThreadSafeEntry<usize> = ThreadSafeEntry::default();
    assert_eq!(0, safe_list.active_slots());
}

#[test]
fn test_thread_safe_entry_from_list() {
    let mut list: EntryList<usize> = EntryList::new();
    list.insert(1);
    list.insert(2);

    let safe_list = ThreadSafeEntry::from(list);
    assert_eq!(2, safe_list.active_slots());
}

#[test]
fn test_thread_safe_entry_concurrent_inserts() {
    let safe_list = Arc::new(ThreadSafeEntry::<usize>::new());
    let mut handles = vec![];

    for i in 0..10 {
        let list_clone = safe_list.clone();
        handles.push(thread::spawn(move || list_clone.insert(i)));
    }

    let entries: Vec<Entry> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(10, safe_list.active_slots());
    assert_eq!(10, entries.len());
}

#[test]
fn test_thread_safe_entry_concurrent_reads() {
    let safe_list = Arc::new(ThreadSafeEntry::<usize>::new());

    let entries: Vec<Entry> = (0..5).map(|i| safe_list.insert(i * 10)).collect();

    let mut handles = vec![];
    for entry in entries {
        let list_clone = safe_list.clone();
        handles.push(thread::spawn(move || {
            let mut result = None;
            list_clone.get(&entry, |value| {
                result = value.copied();
            });
            result
        }));
    }

    let results: Vec<Option<usize>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(5, results.iter().filter(|r| r.is_some()).count());
}

#[test]
fn test_thread_safe_entry_get_mut() {
    let safe_list = ThreadSafeEntry::<usize>::new();
    let entry = safe_list.insert(10);

    safe_list.get_mut(&entry, |value| {
        if let Some(v) = value {
            *v = 20;
        }
    });

    let mut result = None;
    safe_list.get(&entry, |value| {
        result = value.copied();
    });

    assert_eq!(Some(20), result);
}

#[test]
fn test_thread_safe_entry_park_unpark() {
    let safe_list = ThreadSafeEntry::<usize>::new();
    let entry = safe_list.insert(100);

    assert_eq!(0, safe_list.parked_slots());

    let parked = safe_list.park(&entry);
    assert_eq!(Some(100), parked);
    assert_eq!(1, safe_list.parked_slots());

    assert!(safe_list.unpark(&entry, 200));
    assert_eq!(0, safe_list.parked_slots());

    let mut result = None;
    safe_list.get(&entry, |value| {
        result = value.copied();
    });
    assert_eq!(Some(200), result);
}

#[test]
fn test_thread_safe_entry_concurrent_mixed_ops() {
    let safe_list = Arc::new(ThreadSafeEntry::<usize>::new());

    // Insert initial entries
    let entries: Vec<Entry> = (0..20).map(|i| safe_list.insert(i)).collect();

    let mut read_handles = vec![];
    let mut update_handles = vec![];

    // Spawn readers
    for entry in entries.iter().take(10) {
        let list_clone = safe_list.clone();
        let entry_copy = *entry;
        read_handles.push(thread::spawn(move || {
            let mut found = false;
            list_clone.get(&entry_copy, |value| {
                found = value.is_some();
            });
            found
        }));
    }

    // Spawn updaters
    for entry in entries.iter().skip(10) {
        let list_clone = safe_list.clone();
        let entry_copy = *entry;
        update_handles.push(thread::spawn(move || list_clone.update(&entry_copy, 999)));
    }

    // Wait for all operations
    for handle in read_handles {
        handle.join().unwrap();
    }
    for handle in update_handles {
        handle.join().unwrap();
    }

    assert_eq!(20, safe_list.active_slots());
}

#[test]
fn test_thread_safe_entry_select_take() {
    let safe_list = ThreadSafeEntry::<usize>::new();

    for i in 0..10 {
        safe_list.insert(i);
    }

    let evens = safe_list.select_take(|&v| v % 2 == 0);
    assert_eq!(5, evens.len());
    assert_eq!(5, safe_list.active_slots());
}

#[test]
fn test_thread_safe_entry_map_with() {
    let safe_list = ThreadSafeEntry::<usize>::new();

    for i in 1..=5 {
        safe_list.insert(i);
    }

    let doubled: Vec<usize> = safe_list.map_with(|&v| Some(v * 2));
    assert_eq!(vec![2, 4, 6, 8, 10], doubled);
}

#[test]
fn test_thread_safe_entry_for_each() {
    let safe_list = ThreadSafeEntry::<usize>::new();

    for i in 1..=5 {
        safe_list.insert(i);
    }

    let sum = std::sync::Arc::new(std::sync::Mutex::new(0));
    let sum_clone = sum.clone();

    safe_list.for_each(|value| {
        if let Some(&v) = value {
            *sum_clone.lock().unwrap() += v;
        }
    });

    assert_eq!(15, *sum.lock().unwrap());
}
