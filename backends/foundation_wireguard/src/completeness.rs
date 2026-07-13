//! Spec-55 completeness enforcement gate.
//!
//! When `feature = "spec55-complete"` is enabled, `check()` verifies at
//! compile time that every required integration point exists. If any type,
//! trait impl, or method is missing, the crate refuses to compile.
//!
//! Runtime integration tests live in `tests/completeness_tests.rs`.

/// Compile-time completeness check. Every call inside verifies a required
/// integration point exists. If this function compiles, the spec-55 type
/// surface is complete.
#[cfg(feature = "spec55-complete")]
pub fn check() {
    // Check 1: JsDevice implements smoltcp::phy::Device (F07 wasm data plane)
    {
        fn _assert<T: smoltcp::phy::Device>(_: &T) {}
        let dev = crate::wasm::device::JsDevice::new(1420);
        _assert(&dev);
    }

    // Check 2: BrowserWgNode has tick() (F07 browser peer lifecycle)
    {
        // Type-level proof that tick() exists on BrowserWgNode
        fn _call_tick(n: &mut crate::wasm::browser::BrowserWgNode) { n.tick(); }
    }

    // Check 3: RelayServer + RelayClient instantiable (F05 relay)
    {
        use crate::shared::relay::{RelaySelector, RelayStrategy};
        let _s = crate::native::relay::RelayServer::new(16, 1000, 60);
        let _c = crate::native::relay::RelayClient::new(
            RelaySelector::new(RelayStrategy::LowestLoad),
        );
    }

    // Check 4: HolePuncher instantiable (F05 NAT traversal)
    {
        let _ = crate::native::relay::HolePuncher::new(30);
    }

    // Check 5: WgConfig::from_env callable (F09/F10 self-assembly)
    {
        let _: fn() -> crate::shared::error::WgResult<crate::shared::config::WgConfig> =
            || { crate::shared::config::WgConfig::from_env() };
    }
}

/// No-op when spec55-complete is not enabled.
#[cfg(not(feature = "spec55-complete"))]
pub fn check() {}
