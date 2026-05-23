#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::needless_continue)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
// Unstable std library features, available when `nightly` feature is enabled.
#![cfg_attr(feature = "nightly", feature(unix_socket_peek, tcp_linger))]

#[cfg(all(feature = "ssl-native-tls", not(target_arch = "wasm32")))]
extern crate native_tls;

pub mod compati;
pub mod extensions;
pub mod io;
pub mod macros;
pub mod netcap;
pub mod retries;
pub mod synca;
pub mod trace;
pub mod url;
pub mod valtron;
pub mod wire;
