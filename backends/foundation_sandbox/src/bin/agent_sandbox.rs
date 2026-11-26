//! The `agent-sandbox` program (feature 06).
//!
//! This is the only place that builds the real side effects of the control
//! plane: the clock and the `Env` entry, the native `SandboxHost` and the
//! valtron pool. The library code reads them from the [`Ctx`].

use clap::Parser;
use foundation_compact::rng::Rng as _;
use foundation_context::Ctx;
use foundation_sandbox::cli::sandbox::{run, SandboxCli};
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};

fn main() {
    std::process::exit(real_main());
}

/// The program body, so a test can call it and read its exit code.
fn real_main() -> i32 {
    let ctx = Ctx::host();
    match foundation_core::env::system(&ctx) {
        Ok(env) => ctx.store(env),
        Err(error) => {
            eprintln!("agent-sandbox: the environment is not available: {error}");
            return 125;
        }
    }
    let Ok(executable) = std::env::current_exe() else {
        eprintln!("agent-sandbox: the program path is not known");
        return 125;
    };
    let Some(directory) = executable.parent() else {
        eprintln!("agent-sandbox: the program directory is not known");
        return 125;
    };
    let config = HostConfig::discover(|name| std::env::var(name).ok(), directory);
    ctx.store(SandboxHost::native(config));
    start_pool(&ctx);
    run(ctx, SandboxCli::parse())
}

/// Starts the valtron pool, unless this process already has one.
fn start_pool(ctx: &Ctx) {
    if foundation_core::valtron::multi::is_pool_initialized() {
        return;
    }
    let seed = {
        let rng = ctx.rng();
        let mut rng = rng.lock().expect("the Ctx RNG lock is not poisoned");
        rng.next_u64()
    };
    let guard = foundation_core::valtron::initialize_pool_with_ctx(seed, Some(8), ctx.clone());
    // The pool lives for the whole process.
    std::mem::forget(guard);
}
