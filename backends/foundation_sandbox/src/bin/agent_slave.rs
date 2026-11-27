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

    // The unpack child mode: the slave starts this same program with the target
    // and the options, and the child applies the tar stream from its standard
    // input after a `chroot`.
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str)
        == Some(foundation_sandbox::native::slave::unpack::UNPACK_CHILD_FLAG)
    {
        let (Some(dest), Some(options)) = (arguments.get(2), arguments.get(3)) else {
            eprintln!("agent-slave: the unpack child needs a target and options");
            std::process::exit(64);
        };
        std::process::exit(foundation_sandbox::native::slave::unpack::run_unpack_child(
            dest, options,
        ));
    }

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
