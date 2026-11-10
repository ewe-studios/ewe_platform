//! Tests for the `Ctx` methods that came from the old context bag.
//!
//! WHAT: `store`, `get_cloned`, `remove`, `contains`, `build`, `with_empty_map`, `rebase`
//! and `fork`. Also the RNG seeds: `host` draws from the entropy source, and `with_seed`
//! repeats.

use std::sync::Arc;

use foundation_compact::rng::Rng;
use foundation_context::{Clock, Ctx};

/// A context with the real host clock, for the map tests that do not read time.
fn host_ctx() -> Ctx {
    Ctx::host()
}

fn host_clock() -> Arc<dyn Clock> {
    Ctx::host().clock()
}

#[test]
fn store_then_get_returns_the_value() {
    let ctx = host_ctx();
    ctx.store(42u32);
    assert_eq!(*ctx.get::<u32>().expect("u32 should be stored"), 42u32);
}

#[test]
fn get_returns_the_same_arc_each_time() {
    let ctx = host_ctx();
    ctx.store(String::from("hello"));
    let first = ctx.get::<String>().expect("String should be stored");
    let second = ctx.get::<String>().expect("String should still exist");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(*first, "hello");
}

#[test]
fn store_overwrites_the_prior_value() {
    let ctx = host_ctx();
    ctx.store(1u32);
    ctx.store(99u32);
    assert_eq!(*ctx.get::<u32>().expect("u32 should be stored"), 99u32);
}

#[test]
fn store_then_get_cloned_returns_the_value() {
    let ctx = host_ctx();
    ctx.store(7u32);
    assert_eq!(ctx.get_cloned::<u32>(), Some(7));
    assert_eq!(ctx.get_cloned::<u64>(), None);
}

#[test]
fn remove_returns_a_typed_arc() {
    let ctx = host_ctx();
    ctx.store("test".to_string());
    let removed: Arc<String> = ctx.remove::<String>().expect("String should be removed");
    assert_eq!(*removed, "test");
    assert!(ctx.get::<String>().is_none());
}

#[test]
fn contains_follows_the_map() {
    let ctx = host_ctx();
    assert!(!ctx.contains::<i64>());
    ctx.store(100i64);
    assert!(ctx.contains::<i64>());
    assert!(!ctx.contains::<f64>());
}

#[test]
fn build_fills_the_context() {
    let ctx = Ctx::build(host_clock(), |c| {
        c.store(10usize);
        c.store("configured".to_string());
    });
    assert_eq!(*ctx.get::<usize>().unwrap(), 10usize);
    assert_eq!(*ctx.get::<String>().unwrap(), "configured");
    ctx.assert_minimum();
}

#[test]
fn multiple_types_do_not_collide() {
    let ctx = host_ctx();
    ctx.store(1u8);
    ctx.store(2u16);
    ctx.store(3u32);
    ctx.store(4u64);
    assert_eq!(*ctx.get::<u8>().unwrap(), 1);
    assert_eq!(*ctx.get::<u16>().unwrap(), 2);
    assert_eq!(*ctx.get::<u32>().unwrap(), 3);
    assert_eq!(*ctx.get::<u64>().unwrap(), 4);
}

#[test]
fn with_empty_map_shares_the_sources_and_has_no_entries() {
    let ctx = host_ctx();
    ctx.store(5u32);

    let fresh = ctx.with_empty_map();

    assert!(Arc::ptr_eq(&fresh.clock(), &ctx.clock()));
    assert!(Arc::ptr_eq(&fresh.config(), &ctx.config()));
    assert!(Arc::ptr_eq(&fresh.rng(), &ctx.rng()));
    assert!(!fresh.contains::<u32>(), "the map starts empty");
    fresh.assert_minimum();
}

#[test]
fn rebase_keeps_the_map_and_takes_the_other_sources() {
    let sources = host_ctx();
    let original = host_ctx();
    original.store(7u32);

    let rebased = original.rebase(&sources);

    assert_eq!(*rebased.get::<u32>().unwrap(), 7u32);
    assert!(Arc::ptr_eq(&rebased.clock(), &sources.clock()));
    assert!(Arc::ptr_eq(&rebased.config(), &sources.config()));
    assert!(Arc::ptr_eq(&rebased.rng(), &sources.rng()));
    assert!(!Arc::ptr_eq(&rebased.rng(), &original.rng()));

    // The map copy follows the `fork` rule: a later insert stays private.
    original.store(8u64);
    assert!(!rebased.contains::<u64>());
    assert_eq!(*original.get::<u32>().unwrap(), 7u32);
}

#[test]
fn fork_keeps_the_sources_and_copies_the_map() {
    let ctx = host_ctx();
    ctx.store(5u32);

    let fork = ctx.fork();

    assert_eq!(*fork.get::<u32>().unwrap(), 5u32);
    assert!(Arc::ptr_eq(&fork.clock(), &ctx.clock()));
    assert!(Arc::ptr_eq(&fork.rng(), &ctx.rng()));

    ctx.store(6u64);
    assert!(!fork.contains::<u64>());
    fork.assert_minimum();
}

fn first_u64(ctx: &Ctx) -> u64 {
    ctx.rng().lock().expect("RNG lock").next_u64()
}

#[test]
fn two_host_contexts_draw_different_first_values() {
    let first = first_u64(&Ctx::host());
    let second = first_u64(&Ctx::host());
    assert_ne!(first, second, "each host must seed from entropy");
}

#[test]
fn try_host_seeds_from_entropy() {
    let first = first_u64(&Ctx::try_host().expect("entropy source"));
    let second = first_u64(&Ctx::try_host().expect("entropy source"));
    assert_ne!(first, second);
}

#[test]
fn with_seed_repeats_for_the_same_seed() {
    let clock = host_clock();
    let a = first_u64(&Ctx::with_seed(Arc::clone(&clock), 1234));
    let b = first_u64(&Ctx::with_seed(Arc::clone(&clock), 1234));
    let c = first_u64(&Ctx::with_seed(clock, 1235));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn new_keeps_the_fixed_seed_zero() {
    let clock = host_clock();
    let a = first_u64(&Ctx::fixed(Arc::clone(&clock)));
    let b = first_u64(&Ctx::with_seed(clock, 0));
    assert_eq!(a, b);
}
