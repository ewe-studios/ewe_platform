//! The `agent-slave` program (feature 06).
//!
//! This build uses only the `slave` and `cli` features, so it links no TLS
//! library and no Docker client and builds as a static musl program (decision
//! D11). The slave blocks `SIGCHLD` as its first act, before any thread starts.

#[cfg(target_os = "linux")]
fn main() {
    use clap::Parser as _;
    use foundation_context::Ctx;
    use foundation_sandbox::cli::slave::{run, SlaveCli};
    use foundation_sandbox::native::slave::block_sigchld;

    // The signal mask belongs to a thread and a new thread copies it. Block
    // `SIGCHLD` before any thread starts, so the reaper's `signalfd` sees every
    // child.
    if let Err(error) = block_sigchld() {
        eprintln!("agent-slave: the signal mask failed: {error}");
        std::process::exit(70);
    }
    let ctx = Ctx::host();
    if let Ok(env) = foundation_core::env::system(&ctx) {
        ctx.store(env);
    }
    std::process::exit(run(ctx, SlaveCli::parse()));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("agent-slave: the slave needs Linux");
    std::process::exit(1);
}
