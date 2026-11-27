//! Stage shared resources for the configured native project.
fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-env-changed=EWE_RESOURCE_TARGET");
    if matches!(std::env::var("EWE_RESOURCE_TARGET").as_deref(), Ok("apple" | "android")) {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or_else(|| std::io::Error::other("Missing OUT_DIR"))?);
        std::fs::write(out.join("version.txt"), env!("CARGO_PKG_VERSION"))?;
        foundation_resources::build::stage_from_cargo()?;
{% if MOBILE_LAUNCH %}        let package = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| std::io::Error::other("Missing CARGO_MANIFEST_DIR"))?;
        let public = std::path::PathBuf::from(package).join("../public");
        foundation_resources::build::launch::stage_from_cargo(&public)?;
{% endif %}    }
    Ok(())
}
