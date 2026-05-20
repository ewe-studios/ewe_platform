/// workers-rs interop: `From<worker::D1Database>` for `crate::D1Database`.
///
/// Both types wrap the same underlying JS `D1Database` object. Converting
/// through `JsValue` gives us a zero-cost cast to the foundation_db type
/// so it can be used with `D1WasmStorage` and stored in `ContextBag`.
#[allow(unused_imports)]
pub mod d1;

#[allow(unused_imports)]
pub use d1::*;
