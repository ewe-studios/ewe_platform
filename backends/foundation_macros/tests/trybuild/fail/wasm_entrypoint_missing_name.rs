use foundation_macros::wasm_entrypoint;

#[wasm_entrypoint(desc = "no name")]
pub fn missing_name() {}

fn main() {}
