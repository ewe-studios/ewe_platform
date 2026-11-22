//! Build the two classic scripts used by the browser host.
use std::prelude::rust_2021::*;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub const READABLE: &str = "foundation-ui-runtime.js";
pub const MINIFIED: &str = "foundation-ui-runtime.min.js";

/// Generate the browser scripts or compare them with the destination files.
pub fn generate(runtime: &Path, output: &Path, check: bool) -> io::Result<Vec<PathBuf>> {
    let bundle = foundation_codegen::javascript::bundle(&runtime.join("runtime-entry.js"))
        .map_err(|e| io::Error::other(format!("{e:#}")))?;
    if !check {
        fs::create_dir_all(output)?;
    }
    for (name, contents) in [(READABLE, bundle.readable), (MINIFIED, bundle.minified)] {
        let target = output.join(name);
        match fs::read(&target) {
            Ok(existing) if existing == contents => {}
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ if check => {
                return Err(io::Error::other(format!(
                    "Generated file differs: {}",
                    target.display()
                )));
            }
            _ => fs::write(&target, contents)?,
        }
    }
    Ok(bundle.inputs)
}
