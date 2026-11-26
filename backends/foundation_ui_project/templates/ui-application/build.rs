//! Stage shared resources for the configured native project.
fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-env-changed=EWE_RESOURCE_TARGET");
    if matches!(std::env::var("EWE_RESOURCE_TARGET").as_deref(), Ok("apple" | "android")) {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or_else(|| std::io::Error::other("Missing OUT_DIR"))?);
        std::fs::write(out.join("version.txt"), env!("CARGO_PKG_VERSION"))?;
        foundation_resources::build::stage_from_cargo()?;
    }
    Ok(())
}
