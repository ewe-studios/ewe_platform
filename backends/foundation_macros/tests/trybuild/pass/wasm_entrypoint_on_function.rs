use foundation_macros::wasm_entrypoint;

#[wasm_entrypoint(name = "auth_worker", desc = "Authentication worker")]
pub fn auth_handler() {
    // Function body
}

#[wasm_entrypoint(name = "data_processor", desc = "Batch data processing")]
fn process_data() {
    println!("processing");
}

fn main() {}
