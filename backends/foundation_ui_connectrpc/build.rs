//! Generate the action service and messages from the same descriptor set.
use foundation_connectrpc_codegen::build::compile_service;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    compile_service("proto", "actions", "action_service.rs")?;
    compile_service("tests", "watch", "notes_service.rs")
}
