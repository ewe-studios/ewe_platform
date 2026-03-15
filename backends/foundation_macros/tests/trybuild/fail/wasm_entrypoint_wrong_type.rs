use foundation_macros::wasm_entrypoint;

#[wasm_entrypoint(name = 42, desc = "wrong type")]
pub fn wrong_type() {}

fn main() {}
