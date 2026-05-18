//! ContextBag tests — store/get/get_cloned/remove/contains/build.

use std::sync::Arc;
use foundation_http::shared::context::ContextBag;

#[test]
fn test_store_and_get() {
    let bag = ContextBag::new();
    bag.store(42u32);
    let val = bag.get::<u32>().expect("u32 should be stored");
    assert_eq!(*val, 42u32);
}

#[test]
fn test_get_returns_arc_clone() {
    let bag = ContextBag::new();
    bag.store(String::from("hello"));
    let v1 = bag.get::<String>().expect("String should be stored");
    let v2 = bag.get::<String>().expect("String should still exist");
    assert!(Arc::ptr_eq(&v1, &v2));
    assert_eq!(*v1, "hello");
}

#[test]
fn test_store_overwrites() {
    let bag = ContextBag::new();
    bag.store(1u32);
    bag.store(99u32);
    let val = bag.get::<u32>().expect("u32 should be stored");
    assert_eq!(*val, 99u32);
}

#[test]
fn test_get_cloned() {
    let bag = ContextBag::new();
    bag.store(7u32);
    let val: u32 = bag.get_cloned::<u32>().expect("u32 should be cloned");
    assert_eq!(val, 7);
}

#[test]
fn test_remove() {
    let bag = ContextBag::new();
    bag.store("test".to_string());
    let removed = bag.remove::<String>().expect("String should be removed");
    let downcast = Arc::downcast::<String>(removed).expect("Should downcast to String");
    assert_eq!(*downcast, "test");
    assert!(bag.get::<String>().is_none());
}

#[test]
fn test_contains() {
    let bag = ContextBag::new();
    assert!(!bag.contains::<i64>());
    bag.store(100i64);
    assert!(bag.contains::<i64>());
    assert!(!bag.contains::<f64>());
}

#[test]
fn test_build_closure() {
    let bag = ContextBag::build(|b| {
        b.store(10usize);
        b.store("configured".to_string());
    });
    assert_eq!(*bag.get::<usize>().unwrap(), 10usize);
    assert_eq!(*bag.get::<String>().unwrap(), "configured");
}

#[test]
fn test_multiple_types() {
    let bag = ContextBag::new();
    bag.store(1u8);
    bag.store(2u16);
    bag.store(3u32);
    bag.store(4u64);
    assert_eq!(*bag.get::<u8>().unwrap(), 1);
    assert_eq!(*bag.get::<u16>().unwrap(), 2);
    assert_eq!(*bag.get::<u32>().unwrap(), 3);
    assert_eq!(*bag.get::<u64>().unwrap(), 4);
}

#[test]
fn test_default() {
    let bag = ContextBag::default();
    assert!(!bag.contains::<i32>());
    assert!(bag.get::<i32>().is_none());
}
