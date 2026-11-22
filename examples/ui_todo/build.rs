//! Build the example application service for its server and mobile clients.
#[cfg(any(feature = "server", feature = "mobile"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    foundation_connectrpc_codegen::build::compile_service("proto", "todos", "todo_service.rs")?;
    Ok(())
}

#[cfg(not(any(feature = "server", feature = "mobile")))]
fn main() {}
