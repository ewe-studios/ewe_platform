extern crate url;

#[cfg(all(feature = "native-tls", not(target_arch = "wasm32")))]
extern crate native_tls_crate as native_tls;

pub mod directorate;
pub mod extensions;
pub mod io;
pub mod macros;
pub mod retries;
pub mod valtron;
pub mod wire;
