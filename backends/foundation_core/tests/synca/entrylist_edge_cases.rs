//! Edge-case and stress tests for `EntryList`.

use foundation_core::synca::{Entry, EntryList};

#[test]
fn test_for_each_iteration() {
    let mut list: EntryList<usize> = EntryList::new();
    list.insert(1);
    list.insert(2);
    list.insert(3);

    let sum = std::sync::Arc::new(std::sync::Mutex::new(0));
    let sum_clone = sum.clone();

    list.for_each(|value| {
        if let Some(&v) = value {
            *sum_clone.lock().unwrap() += v;
        }
    });

    assert_eq!(6, *sum.lock().unwrap());
}

#[test]
fn test_map_with_transformation() {
    let mut list: EntryList<usize> = EntryList::new();
    list.insert(1);
    list.insert(2);
    list.insert(3);

    let doubled: Vec<usize> = list.map_with(|&v| Some(v * 2));

    assert_eq!(vec![2, 4, 6], doubled);
}

#[test]
fn test_map_with_filtering() {
    let mut list: EntryList<usize> = EntryList::new();
    list.insert(1);
    list.insert(2);
    list.insert(3);
    list.insert(4);

    let evens: Vec<usize> = list.map_with(|&v| if v % 2 == 0 { Some(v) } else { None });

    assert_eq!(vec![2, 4], evens);
}

#[test]
fn test_large_scale_insertions() {
    let mut list: EntryList<usize> = EntryList::new();
    let count = 1000;

    let mut entries = Vec::new();
    for i in 0..count {
        entries.push(list.insert(i));
    }

    assert_eq!(count, list.active_slots());
    assert_eq!(count, list.allocated_slots());
    assert_eq!(0, list.open_slots());

    // Verify random access
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(Some(&i), list.get(entry));
    }
}

#[test]
fn test_alternating_insert_remove() {
    let mut list: EntryList<usize> = EntryList::new();

    for i in 0..100 {
        let entry = list.insert(i);
        assert_eq!(1, list.active_slots());

        list.take(&entry);
        assert_eq!(0, list.active_slots());
        assert_eq!(1, list.open_slots());
    }

    // After 100 cycles, we should have reused slots
    assert_eq!(1, list.allocated_slots());
    assert_eq!(1, list.open_slots());
}

#[test]
fn test_generation_increment_on_reuse() {
    let mut list: EntryList<usize> = EntryList::new();

    let entry1 = list.insert(1);
    list.take(&entry1);

    let entry2 = list.insert(2);

    // entry2 should have same id but different generation
    assert!(entry1.id == entry2.id);
    assert!(entry1.gen != entry2.gen);

    // Old entry should be invalid
    assert!(list.not_valid(&entry1));
    assert!(list.has(&entry2));
}

#[test]
fn test_multiple_select_take_operations() {
    let mut list: EntryList<usize> = EntryList::new();

    for i in 0..10 {
        list.insert(i);
    }

    // Take all even numbers
    let evens = list.select_take(|&v| v % 2 == 0);
    assert_eq!(5, evens.len());
    assert_eq!(5, list.active_slots());

    // Take all remaining numbers > 5
    let high = list.select_take(|&v| v > 5);
    assert_eq!(2, high.len());
    assert_eq!(3, list.active_slots());
}

#[test]
fn test_vacate_vs_take() {
    let mut list: EntryList<String> = EntryList::new();

    let entry1 = list.insert("value1".to_string());
    let entry2 = list.insert("value2".to_string());

    // vacate drops without returning
    list.vacate(&entry1);
    assert_eq!(None, list.get(&entry1));
    assert_eq!(1, list.open_slots());

    // take returns the value
    let value = list.take(&entry2);
    assert_eq!(Some("value2".to_string()), value);
    assert_eq!(2, list.open_slots());
}

#[test]
fn test_park_and_unpark_cycle() {
    let mut list: EntryList<usize> = EntryList::new();

    let entry = list.insert(100);
    assert_eq!(1, list.active_slots());
    assert_eq!(0, list.parked_slots());

    // Park the entry
    let parked_value = list.park(&entry);
    assert_eq!(Some(100), parked_value);
    assert_eq!(1, list.active_slots()); // Still counts as active
    assert_eq!(1, list.parked_slots());
    assert_eq!(None, list.get(&entry)); // But value is None

    // Unpark with different value
    assert!(list.unpark(&entry, 200));
    assert_eq!(Some(&200), list.get(&entry));
    assert_eq!(0, list.parked_slots());
    assert_eq!(1, list.active_slots());
}

#[test]
fn test_unpark_nonexistent_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = Entry::new(999, 0);

    // Trying to unpark a non-existent entry should fail
    assert!(!list.unpark(&entry, 100));
}

#[test]
fn test_replace_invalidates_old_entry() {
    let mut list: EntryList<usize> = EntryList::new();

    let entry1 = list.insert(10);
    assert!(list.has(&entry1));

    let (entry2, old_value) = list.replace(&entry1, 20).unwrap();
    assert_eq!(10, old_value);

    // entry1 is now invalid
    assert!(list.not_valid(&entry1));
    assert!(!list.has(&entry1));

    // entry2 is valid with new value
    assert!(list.has(&entry2));
    assert_eq!(Some(&20), list.get(&entry2));
}

#[test]
fn test_mixed_operations_stress() {
    let mut list: EntryList<usize> = EntryList::new();
    let mut active_entries = Vec::new();

    // Insert initial batch
    for i in 0..50 {
        active_entries.push(list.insert(i));
    }

    // Remove half
    for i in (0..25).rev() {
        list.take(&active_entries.remove(i));
    }

    assert_eq!(25, list.active_slots());
    assert_eq!(25, list.open_slots());

    // Update some
    for entry in active_entries.iter().take(10) {
        list.update(entry, 999);
    }

    // Replace some (invalidates old entries)
    let mut new_entries = Vec::new();
    for entry in active_entries.iter().skip(10).take(5) {
        if let Some((new_entry, _)) = list.replace(entry, 888) {
            new_entries.push(new_entry);
        }
    }

    // Verify integrity
    for entry in new_entries.iter() {
        assert_eq!(Some(&888), list.get(entry));
    }
}
