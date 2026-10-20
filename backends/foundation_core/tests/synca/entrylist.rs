//! Tests for `EntryList`: insert, get, park/unpark, take, vacate, replace.

use foundation_core::synca::{Entry, EntryList};

#[test]
fn entry_list_insert_reference() {
    let mut list: EntryList<&usize> = EntryList::new();
    let entry = list.insert(&1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&&1), list.get(&entry));
    assert_eq!(Some(&mut &1), list.get_mut(&entry));
}

#[test]
fn entry_list_multi_insert_reference() {
    let mut list: EntryList<&usize> = EntryList::new();
    let entry = list.insert(&1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&&1), list.get(&entry));
    assert_eq!(Some(&mut &1), list.get_mut(&entry));

    let entry2 = list.insert(&2);
    assert_eq!(entry2, Entry::new(1, 0));

    let entry3 = list.insert(&3);
    assert_eq!(entry3, Entry::new(2, 0));
}

#[test]
fn entry_list_can_select_take() {
    let mut list: EntryList<usize> = EntryList::new();
    list.insert(1);
    list.insert(2);

    assert_eq!(2, list.allocated_slots());
    assert_eq!(0, list.open_slots());
    assert_eq!(2, list.active_slots());

    let values = list.select_take(|_| true);
    assert_eq!(vec![1, 2], values);

    assert_eq!(2, list.allocated_slots());
    assert_eq!(2, list.open_slots());
    assert_eq!(0, list.active_slots());
}

#[test]
fn entry_list_can_park_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));
    assert_eq!(Some(&mut 1), list.get_mut(&entry));

    assert_eq!(Some(1), list.park(&entry));

    assert_eq!(1, list.allocated_slots());
    assert_eq!(0, list.open_slots());
    assert_eq!(1, list.parked_slots());
    assert_eq!(1, list.active_slots());

    assert!(list.unpark(&entry, 2));
    assert_eq!(Some(&2), list.get(&entry));

    assert_eq!(1, list.allocated_slots());
    assert_eq!(0, list.open_slots());
    assert_eq!(0, list.parked_slots());
    assert_eq!(1, list.active_slots());
}

#[test]
fn entry_list_can_take_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));
    assert_eq!(Some(&mut 1), list.get_mut(&entry));

    assert_eq!(Some(1), list.take(&entry));
    assert_eq!(1, list.allocated_slots());
    assert_eq!(1, list.open_slots());
    assert_eq!(0, list.active_slots());
}

#[test]
fn entry_list_insert_value() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));
    assert_eq!(Some(&mut 1), list.get_mut(&entry));
}

#[test]
fn entry_list_can_vacate_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));
    list.vacate(&entry);
    assert_eq!(None, list.get(&entry));

    assert_eq!(1, list.allocated_slots());
    assert_eq!(1, list.open_slots());
    assert_eq!(0, list.active_slots());
}

#[test]
fn entry_list_can_check_entry_validity() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));

    assert!(list.has(&entry));
}

#[test]
fn entry_list_can_check_if_is_invalid_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));

    let (_, old_value) = list.replace(&entry, 2).expect("should have value");
    assert_eq!(1, old_value);

    assert!(list.not_valid(&entry));
}

#[test]
fn entry_list_can_replace_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));

    let (new_entry, old_value) = list.replace(&entry, 2).expect("should have value");
    assert_eq!(new_entry, Entry::new(0, 1));
    assert_eq!(1, old_value);

    assert_eq!(None, list.get(&entry));
    assert_eq!(Some(&2), list.get(&new_entry));
    assert_eq!(new_entry.id, entry.id);
}

#[test]
fn entry_list_can_update_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));
    assert_eq!(Some(1), list.update(&entry, 2));
    assert_eq!(Some(&2), list.get(&entry));
}

#[test]
fn entry_list_can_modify_entry() {
    let mut list: EntryList<usize> = EntryList::new();
    let entry = list.insert(1);
    assert_eq!(entry, Entry::new(0, 0));

    assert_eq!(Some(&1), list.get(&entry));

    if let Some(value) = list.get_mut(&entry) {
        *value = 2;
    }

    assert_eq!(Some(&2), list.get(&entry));
}
