//! The `agent-rpc` process.
//!
//! WHY: an operator needs a process entry point to seal the clear-text key
//! files of the first version and to rotate keys.
//!
//! WHAT: `agent-rpc keys migrate|rotate|rotate-kek`.
//!
//! HOW: the crate command line parses the arguments, the host builds its
//! context, and the process blocks on the async command.

use foundation_agent_rpc::cli;
use foundation_context::Ctx;
use foundation_core::valtron::{block_on_future, valtron};

#[valtron]
fn main() {
    let command = match cli::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let ctx = match Ctx::try_host() {
        Ok(ctx) => ctx,
        Err(error) => {
            eprintln!("agent-rpc: the host context failed: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = foundation_core::env::install(&ctx, foundation_core::env::system) {
        eprintln!("agent-rpc: the effect sources failed: {error}");
        std::process::exit(1);
    }
    let result = block_on_future(async move { cli::run(&command, &ctx).await });
    match result {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("agent-rpc keys: {error}");
            std::process::exit(1);
        }
    }
}
