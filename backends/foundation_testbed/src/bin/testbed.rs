//! Standalone testbed CLI — VM lifecycle via the Provider trait.
//!
//! Install: `cargo install --path backends/foundation_testbed --features cli`
//! Run: `testbed start windows-build`

#![cfg(feature = "vms")]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::pedantic)]

fn main() {
    let _guard = foundation_core::valtron::initialize_pool(100, None);

    let matches = foundation_testbed::vms::cli::command().get_matches();
    if let Err(e) = foundation_testbed::vms::cli::run(&matches) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
