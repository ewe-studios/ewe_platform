//! Adapt generated WebAssembly bindings for an application host.
use std::{format, path::Path, writeln};

/// Give each host its own binding functions, memory views, and finalizers.
///
/// # Errors
/// Returns an error for invalid generated bindings or a failed file operation.
pub fn bindings(input: &Path, output: &Path) -> std::io::Result<()> {
    use std::{
        fs,
        io::{self, Write},
    };
    let leaf = input
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("Expected a generated JavaScript filename"))?;
    let dir = input
        .parent()
        .ok_or_else(|| io::Error::other("Missing binding directory"))?;
    let temp = tempfile::Builder::new()
        .prefix("bindings-")
        .suffix(".js")
        .tempfile_in(dir)?;
    let module = serde_json::to_string(&format!("./{leaf}"))?;
    fs::write(
        temp.path(),
        format!("import * as functions from {module}; receiveBindings(functions);"),
    )?;
    let bundle = foundation_codegen::javascript::bundle(temp.path())
        .map_err(|e| io::Error::other(format!("{e:#}")))?;
    let mut file = fs::File::create(output)?;
    writeln!(file, "export const importName = {module};")?;
    file.write_all(b"export function createBindings() {\nlet bindings;\nfunction receiveBindings(value) { bindings = value; }\n")?;
    file.write_all(&bundle.readable)?;
    file.write_all(b"\nreturn bindings;\n}\n")
}
