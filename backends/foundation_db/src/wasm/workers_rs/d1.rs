/// workers-rs interop: `From<worker::D1Database>` for `crate::D1Database`.
///
/// Both types wrap the same underlying JS `D1Database` object. Converting
/// through `JsValue` gives us a zero-cost cast to the foundation_db type
/// so it can be used with `D1WasmStorage` and stored in `ContextBag`.
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use worker::D1Database as WorkerD1Database;

use crate::D1Database;

impl From<WorkerD1Database> for D1Database {
    fn from(db: WorkerD1Database) -> Self {
        let js_val: JsValue = db.into();
        js_val.unchecked_into()
    }
}
