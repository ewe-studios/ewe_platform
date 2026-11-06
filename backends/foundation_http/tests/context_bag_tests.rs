//! `ContextBag` tests — `store/get/get_cloned/remove/contains/build` and the
//! engine-minimum clock and RNG.

use foundation_core::valtron::Ctx;
use foundation_http::shared::context::ContextBag;
use std::sync::Arc;

/// A bag with the real host clock, for the map-storage tests that do not read
/// time.
fn host_bag() -> ContextBag {
    ContextBag::new(Ctx::host().clock())
}

#[test]
fn test_store_and_get() {
    let bag = host_bag();
    bag.store(42u32);
    let val = bag.get::<u32>().expect("u32 should be stored");
    assert_eq!(*val, 42u32);
}

#[test]
fn test_get_returns_arc_clone() {
    let bag = host_bag();
    bag.store(String::from("hello"));
    let v1 = bag.get::<String>().expect("String should be stored");
    let v2 = bag.get::<String>().expect("String should still exist");
    assert!(Arc::ptr_eq(&v1, &v2));
    assert_eq!(*v1, "hello");
}

#[test]
fn test_store_overwrites() {
    let bag = host_bag();
    bag.store(1u32);
    bag.store(99u32);
    let val = bag.get::<u32>().expect("u32 should be stored");
    assert_eq!(*val, 99u32);
}

#[test]
fn test_get_cloned() {
    let bag = host_bag();
    bag.store(7u32);
    let val: u32 = bag.get_cloned::<u32>().expect("u32 should be cloned");
    assert_eq!(val, 7);
}

#[test]
fn test_remove() {
    let bag = host_bag();
    bag.store("test".to_string());
    let removed = bag.remove::<String>().expect("String should be removed");
    let downcast = Arc::downcast::<String>(removed).expect("Should downcast to String");
    assert_eq!(*downcast, "test");
    assert!(bag.get::<String>().is_none());
}

#[test]
fn test_contains() {
    let bag = host_bag();
    assert!(!bag.contains::<i64>());
    bag.store(100i64);
    assert!(bag.contains::<i64>());
    assert!(!bag.contains::<f64>());
}

#[test]
fn test_build_closure() {
    let bag = ContextBag::build(Ctx::host().clock(), |b| {
        b.store(10usize);
        b.store("configured".to_string());
    });
    assert_eq!(*bag.get::<usize>().unwrap(), 10usize);
    assert_eq!(*bag.get::<String>().unwrap(), "configured");
    bag.assert_minimum();
}

#[test]
fn test_multiple_types() {
    let bag = host_bag();
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
fn from_ctx_shares_the_clock_and_rng() {
    let ctx = Ctx::host();
    let bag = ContextBag::from_ctx(&ctx);

    assert!(
        Arc::ptr_eq(&bag.clock(), &ctx.clock()),
        "from_ctx must share the task clock"
    );
    assert!(
        Arc::ptr_eq(&bag.rng(), &ctx.rng()),
        "from_ctx must share the task RNG stream"
    );
    assert!(!bag.contains::<u32>(), "from_ctx starts with an empty map");
    bag.assert_minimum();
}

#[test]
fn with_ctx_copies_the_map_and_takes_the_ctx_clock_and_rng() {
    let ctx = Ctx::host();
    let original = ContextBag::new(ctx.clock());
    original.store(7u32);

    let rebased = original.with_ctx(&ctx);

    assert_eq!(*rebased.get::<u32>().unwrap(), 7u32);
    assert!(Arc::ptr_eq(&rebased.clock(), &ctx.clock()));
    assert!(Arc::ptr_eq(&rebased.rng(), &ctx.rng()));

    // The map copy follows the `Ctx::fork` rule: a later insert into the
    // original stays private to it.
    original.store(8u64);
    assert!(!rebased.contains::<u64>());
    assert_eq!(*original.get::<u32>().unwrap(), 7u32);
}

#[test]
fn fork_keeps_the_clock_and_rng_and_copies_the_map() {
    let clock = Ctx::host().clock();
    let bag = ContextBag::new(Arc::clone(&clock));
    bag.store(5u32);

    let fork = bag.fork();

    assert_eq!(*fork.get::<u32>().unwrap(), 5u32);
    assert!(Arc::ptr_eq(&fork.clock(), &clock));
    assert!(Arc::ptr_eq(&fork.rng(), &bag.rng()));

    bag.store(6u64);
    assert!(!fork.contains::<u64>());
    fork.assert_minimum();
}
