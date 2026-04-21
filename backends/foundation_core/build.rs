fn main() {
    println!("cargo::rustc-check-cfg=cfg(cranelift_backend)");

    // Detect if cranelift codegen-backend is configured in the workspace
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent());

    if let Some(root) = workspace_root {
        if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if content.contains("codegen-backend") && content.contains("cranelift") {
                println!("cargo::rustc-cfg=cranelift_backend");
            }
        }
    }
}
