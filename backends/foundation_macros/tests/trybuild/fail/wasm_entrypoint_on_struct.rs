use foundation_macros::wasm_entrypoint;

#[wasm_entrypoint(name = "bad", desc = "wrong")]
struct NotAFunction;

fn main() {}
