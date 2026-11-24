fn main() -> std::io::Result<()> {
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("wasm") { return Ok(()); }
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or_else(|| std::io::Error::other("Missing OUT_DIR"))?);
    std::fs::write(out.join("title.txt"), app_identity::TITLE)?;
    foundation_resources::package::BrowserPackage::new(std::path::Path::new("client/app.js"))
        .with_npm_runtime(std::path::Path::new("package-lock.json"))
        .build()
}
