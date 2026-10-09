//! Standalone OpenAPI code generator binary.
//!
//! Install: `cargo install --path backends/foundation_codegentools --features cli`
//! Run: `genapi generate cloudflare`

#![cfg(feature = "cli")]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::pedantic)]

fn main() {
	let matches = foundation_codegentools::cli::gen_api::command().get_matches();
	if let Err(e) = foundation_codegentools::cli::gen_api::run(&matches) {
		eprintln!("error: {e}");
		std::process::exit(1);
	}
}
