//! Startup errors for the browser module override.
use std::path::Path;
use std::process::Command;

#[test]
fn invalid_configured_module_stops_before_announcing_an_address() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [
        root.join("missing.wasm"),
        root.join("Cargo.toml"),
        root.join("src"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ui_todo"))
            .args(["127.0.0.1:0", "--restore"])
            .env("EWE_TODO_MODULE", &path)
            .output()
            .expect("start the todo server");
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Open http"));
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("EWE_TODO_MODULE"), "{message}");
        assert!(
            message.contains(&*path.file_name().unwrap().to_string_lossy()),
            "{message}"
        );
    }
}
