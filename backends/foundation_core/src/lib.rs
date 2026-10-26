#![allow(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::type_complexity)]

pub mod compati;
pub mod extensions;
pub mod io;
pub mod macros;
pub mod retries;
pub mod synca;
pub mod trace;
pub mod traits;
pub mod type_uuid;
pub mod url;
pub mod valtron;

/// `#[wasm_logs(stdout | console)]` — install a tracing subscriber (console.log
/// on wasm, stdout on native) at the top of a function body. See
/// `foundation_macros::wasm_logs`.
pub use foundation_macros::wasm_logs;
