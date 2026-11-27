//! Helpers for the KMS tests.

use foundation_context::Ctx;
use foundation_core::env::Env;

/// A host context with the real effect sources installed.
pub fn test_ctx() -> (Ctx, Env) {
    let ctx = Ctx::host();
    let env = foundation_core::env::install(&ctx, foundation_core::env::system)
        .expect("install the effect sources");
    (ctx, env)
}
