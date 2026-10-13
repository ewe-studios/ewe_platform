//! Standalone testbed CLI — VM lifecycle via the Provider trait.
//!
//! Install: `cargo install --path backends/foundation_deployment_platform --features cli --bin testbed`
//! Run: `testbed start windows-build`

#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::pedantic)]

use foundation_core::valtron::valtron;

#[valtron]
fn main() {
    let matches = foundation_deployment_platform::cli::command().get_matches();
    if let Err(e) = foundation_deployment_platform::cli::run(&matches) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
