//! Generate the version and stage this application's package resources.
fn main() -> std::io::Result<()> {
    let out = std::env::var_os("OUT_DIR")
        .ok_or_else(|| std::io::Error::other("Cargo did not supply OUT_DIR"))?;
    let version = std::path::Path::new(&out).join("version.txt");
    let current = match std::fs::read_to_string(&version) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if current.as_deref() != Some(env!("CARGO_PKG_VERSION")) {
        std::fs::write(&version, env!("CARGO_PKG_VERSION"))?;
    }
    foundation_resources::build::stage_from_cargo()?;
    Ok(())
}
