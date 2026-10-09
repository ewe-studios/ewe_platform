use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use foundation_core::make_collection;

#[test]
fn can_make_vec() {
    let generated: Vec<_> = make_collection![1, 2, 3];
    assert_eq!(vec![1, 2, 3], generated);
}

#[test]
fn can_make_btree_map() {
    let generated: BTreeMap<_, _> = make_collection! { 1 => 2, 3 => 4 };

    let mut expected = BTreeMap::<usize, usize>::new();
    expected.insert(1, 2);
    expected.insert(3, 4);

    assert_eq!(expected, generated)
}

#[test]
fn can_make_hash_map() {
    let generated: HashMap<_, _> = make_collection! { 1 => 2, 3 => 4 };

    let mut expected = HashMap::<usize, usize>::new();
    expected.insert(1, 2);
    expected.insert(3, 4);

    assert_eq!(expected, generated)
}

#[test]
fn can_make_btree_set() {
    let generated: BTreeSet<_> = make_collection! { 1, 2, 3 };

    let mut expected = BTreeSet::<usize>::new();
    expected.insert(1);
    expected.insert(2);
    expected.insert(3);

    assert_eq!(expected, generated)
}

#[test]
fn can_make_hash_set() {
    let generated: HashSet<_> = make_collection! { 1, 2, 3 };

    let mut expected = HashSet::<usize>::new();
    expected.insert(1);
    expected.insert(2);
    expected.insert(3);

    assert_eq!(expected, generated)
}
