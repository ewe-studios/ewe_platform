//! Proves that `foundation_core::valtron::Ctx` is the `foundation_context::Ctx` type.

use std::sync::Arc;

use foundation_core::valtron::Ctx;

fn takes_core_ctx(ctx: Ctx) -> u32 {
    *ctx.get::<u32>().expect("the entry is present")
}

#[test]
fn the_re_export_is_the_same_type() {
    let ctx: foundation_context::Ctx = foundation_context::Ctx::host();
    ctx.insert(Arc::new(9u32));

    assert_eq!(takes_core_ctx(ctx), 9);
}

#[test]
fn the_re_exported_clock_is_the_same_trait() {
    let clock: Arc<dyn foundation_core::valtron::Clock> =
        Arc::new(foundation_context::MonotonicClock::new());
    let ctx = Ctx::new(clock);
    ctx.assert_minimum();
}
