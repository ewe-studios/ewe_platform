//! Build the example application service for its server and mobile clients.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(feature = "server", feature = "mobile"))]
    foundation_connectrpc_codegen::build::compile_service("proto", "todos", "todo_service.rs")?;
    Ok(())
}
