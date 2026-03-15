pub mod handlers;

#[wasm_entrypoint(name = "root_worker", desc = "Root-level worker")]
pub fn root_entry() {
    // root entrypoint
}
