#![allow(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::type_complexity)]

// valtron's `multi` executor is a native thread pool: it does not compile for
// wasm32 (no threads, no idle-wake path). Cargo merges features across the whole
// graph, so one dependency that asks for `foundation_core/multi` (directly or
// through `foundation_netio/multi`, `foundation_db/multi`, ...) turns it on for
// every wasm build of the graph. Fail with the cause instead of deep inside the
// executor. Put the dependency that asks for `multi` in a
// `[target.'cfg(not(target_family = "wasm"))'.dependencies]` table.
#[cfg(all(feature = "multi", target_family = "wasm"))]
compile_error!(
    "foundation_core: the `multi` feature is native-only and cannot be enabled for wasm32. \
     Find who enables it with \
     `cargo tree -e features -i foundation_core --target wasm32-unknown-unknown`, \
     and move that dependency to a `cfg(not(target_family = \"wasm\"))` target table."
);

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
