use foundation_macros::wasm_entrypoint;

#[wasm_entrypoint(name = "incomplete")]
pub fn missing_desc() {}

fn main() {}
