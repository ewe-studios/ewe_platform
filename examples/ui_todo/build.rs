//! Build the service schema and package the application resources.
#[path = "src/theme.rs"]
mod theme;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(feature = "server", feature = "mobile"))]
    foundation_connectrpc_codegen::build::compile_service("proto", "todos", "todo_service.rs")?;
    foundation_resources::package::BrowserPackage::new(std::path::Path::new("web/app.js"))
        .with_module_path("module.wasm")
        .browser_enabled(cfg!(feature = "server"))
        .with_connect()
        .with_resources()
        .with_companion("mathalley-web", "mathalley")
        .with_generated("app-version.txt", env!("CARGO_PKG_VERSION"))
        .with_generated("theme.css", theme::todo_theme().to_css())
        .build()?;
    Ok(())
}
