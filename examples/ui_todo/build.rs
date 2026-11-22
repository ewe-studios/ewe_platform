//! Build the example application service for its server and mobile clients.
#[cfg(feature = "server")]
#[path = "../../backends/foundation_wasm_ui/src/runtime_build.rs"]
mod runtime_build;

fn write_changed(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    match std::fs::metadata(path) {
        Ok(info)
            if info.len() == contents.len() as u64
                && std::fs::read(path)? == contents.as_bytes() =>
        {
            return Ok(())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::fs::write(path, contents)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(feature = "server", feature = "mobile"))]
    foundation_connectrpc_codegen::build::compile_service("proto", "todos", "todo_service.rs")?;
    #[cfg(feature = "server")]
    {
        let runtime = std::path::Path::new("../../backends/foundation_wasm_ui/runtimes");
        let output = std::env::var_os("OUT_DIR").ok_or("Cargo did not set OUT_DIR")?;
        for input in runtime_build::generate(runtime, std::path::Path::new(&output), false)? {
            println!("cargo:rerun-if-changed={}", input.display());
        }
        println!("cargo:rerun-if-changed=../../backends/foundation_wasm_ui/src/runtime_build.rs");
        println!("cargo:rerun-if-changed=build.rs");
    }
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Cargo did not set OUT_DIR")?);
    write_changed(&output.join("app-version.txt"), env!("CARGO_PKG_VERSION"))?;
    foundation_resources::build::stage_from_cargo()?;
    Ok(())
}
