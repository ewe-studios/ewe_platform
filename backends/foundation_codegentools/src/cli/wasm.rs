//! WHY: Feature 15's CLI surface — refined, type-safe inspection and modification
//! of `.wasm`/`.wat` files using the owned `foundation_codegen::wasm` model
//! (wasmbin port), instead of reaching for external binary tools.
//!
//! WHAT: The `wasm` subcommand: `inspect` (dump module structure), `validate`
//! (decode-check), `convert` (wasm ⇄ wat by extension), and `edit`
//! (`add-custom-section`, `rename-export`) with `Lazy<T>` minimal-diff re-encoding.
//!
//! HOW: Loads either format into the typed `Module` (WAT via the opt-in `wat`
//! feature of `foundation_codegen`), applies the operation, and writes back in the
//! format implied by the output path's extension.

use std::path::Path;

use clap::{Arg, ArgMatches, Command};
use foundation_codegen::wasm::builtins::UnparsedBytes;
use foundation_codegen::wasm::sections::{payload, CustomSection, RawCustomSection, Section};
use foundation_codegen::wasm::visit::Visit;
use foundation_codegen::wasm::{wat, Module};

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[must_use]
pub fn command() -> Command {
    Command::new("wasm")
        .about("Type-safe .wasm/.wat inspection and modification")
        .arg_required_else_help(true)
        .subcommand(
            Command::new("inspect")
                .about("Dump module structure: sections, sizes, exports, imports")
                .arg(Arg::new("file").required(true).help("Path to a .wasm or .wat file")),
        )
        .subcommand(
            Command::new("validate")
                .about("Check that the file decodes cleanly into the typed model")
                .arg(Arg::new("file").required(true).help("Path to a .wasm or .wat file")),
        )
        .subcommand(
            Command::new("convert")
                .about("Convert between binary and text by extension (.wasm ⇄ .wat)")
                .arg(Arg::new("input").required(true).help("Input .wasm or .wat file"))
                .arg(Arg::new("output").required(true).help("Output .wasm or .wat file")),
        )
        .subcommand(
            Command::new("edit")
                .about("Type-safe edits with minimal-diff re-encoding")
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("add-custom-section")
                        .about("Append a custom section (name + data)")
                        .arg(Arg::new("file").required(true).help("Module to edit"))
                        .arg(Arg::new("name").long("name").required(true).help("Custom section name"))
                        .arg(Arg::new("data").long("data").required(true).help("Section payload (UTF-8 text)"))
                        .arg(Arg::new("output").long("output").short('o').help("Output path (defaults to in-place)")),
                )
                .subcommand(
                    Command::new("rename-export")
                        .about("Rename an export, keeping every other byte untouched")
                        .arg(Arg::new("file").required(true).help("Module to edit"))
                        .arg(Arg::new("from").long("from").required(true).help("Current export name"))
                        .arg(Arg::new("to").long("to").required(true).help("New export name"))
                        .arg(Arg::new("output").long("output").short('o').help("Output path (defaults to in-place)")),
                ),
        )
}

/// # Errors
///
/// Returns decode/encode/IO errors from the underlying operations.
pub fn run(args: &ArgMatches) -> Result<(), BoxedError> {
    match args.subcommand() {
        Some(("inspect", sub)) => run_inspect(sub),
        Some(("validate", sub)) => run_validate(sub),
        Some(("convert", sub)) => run_convert(sub),
        Some(("edit", sub)) => match sub.subcommand() {
            Some(("add-custom-section", sub)) => run_add_custom_section(sub),
            Some(("rename-export", sub)) => run_rename_export(sub),
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

fn is_wat(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("wat" | "wast")
    )
}

fn load(path: &Path) -> Result<Module, BoxedError> {
    if is_wat(path) {
        let text = std::fs::read_to_string(path)?;
        Ok(wat::from_wat(&text)?)
    } else {
        let bytes = std::fs::read(path)?;
        Ok(Module::decode_from(bytes.as_slice())?)
    }
}

fn save(module: &Module, path: &Path) -> Result<(), BoxedError> {
    if is_wat(path) {
        std::fs::write(path, wat::to_wat(module)?)?;
    } else {
        let bytes = module.encode_into(Vec::new())?;
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

fn run_inspect(args: &ArgMatches) -> Result<(), BoxedError> {
    let path = Path::new(args.get_one::<String>("file").expect("file is required"));
    let module = load(path)?;

    println!("module: {}", path.display());
    println!("sections: {}", module.sections.len());
    for section in &module.sections {
        match section {
            Section::Custom(blob) => match blob.try_contents() {
                Ok(custom) => println!("  custom         name={:?}", custom.name()),
                Err(err) => println!("  custom         <undecodable: {err}>"),
            },
            other => println!("  {:?}", other.kind()),
        }
    }

    if let Some(exports) = module.find_std_section::<payload::Export>() {
        let exports = exports.try_contents()?;
        println!("exports: {}", exports.len());
        for export in exports {
            println!("  {:24} {:?}", export.name, export.desc);
        }
    }
    if let Some(imports) = module.find_std_section::<payload::Import>() {
        let imports = imports.try_contents()?;
        println!("imports: {}", imports.len());
        for import in imports {
            println!("  {}.{}", import.path.module, import.path.name);
        }
    }
    Ok(())
}

fn run_validate(args: &ArgMatches) -> Result<(), BoxedError> {
    let path = Path::new(args.get_one::<String>("file").expect("file is required"));
    let module = load(path)?;
    // A deep visit forces EVERY Lazy/Blob payload to decode (function bodies
    // included), not just the section skeleton.
    if let Err(err) = module.visit(|_: &String| ()) {
        let err: foundation_codegen::wasm::io::DecodeError = err.into();
        return Err(err.into());
    }
    println!(
        "ok: {} ({} sections decode deeply)",
        path.display(),
        module.sections.len()
    );
    Ok(())
}

fn run_convert(args: &ArgMatches) -> Result<(), BoxedError> {
    let input = Path::new(args.get_one::<String>("input").expect("input is required"));
    let output = Path::new(args.get_one::<String>("output").expect("output is required"));
    let module = load(input)?;
    save(&module, output)?;
    println!("converted {} -> {}", input.display(), output.display());
    Ok(())
}

fn output_path<'p>(args: &'p ArgMatches, file: &'p Path) -> &'p Path {
    args.get_one::<String>("output")
        .map_or(file, |out| Path::new(out.as_str()))
}

fn run_add_custom_section(args: &ArgMatches) -> Result<(), BoxedError> {
    let file = Path::new(args.get_one::<String>("file").expect("file is required"));
    let name = args.get_one::<String>("name").expect("name is required");
    let data = args.get_one::<String>("data").expect("data is required");

    let mut module = load(file)?;
    module.sections.push(
        CustomSection::Other(RawCustomSection {
            name: name.clone(),
            data: UnparsedBytes {
                bytes: data.clone().into_bytes(),
            },
        })
        .into(),
    );
    let out = output_path(args, file);
    save(&module, out)?;
    println!("added custom section {name:?} -> {}", out.display());
    Ok(())
}

fn run_rename_export(args: &ArgMatches) -> Result<(), BoxedError> {
    let file = Path::new(args.get_one::<String>("file").expect("file is required"));
    let from = args.get_one::<String>("from").expect("from is required");
    let to = args.get_one::<String>("to").expect("to is required");

    let mut module = load(file)?;
    let exports = module
        .find_std_section_mut::<payload::Export>()
        .ok_or("module has no export section")?
        .try_contents_mut()?;
    let export = exports
        .iter_mut()
        .find(|export| export.name == *from)
        .ok_or_else(|| format!("no export named {from:?}"))?;
    export.name.clone_from(to);

    let out = output_path(args, file);
    save(&module, out)?;
    println!("renamed export {from:?} -> {to:?} in {}", out.display());
    Ok(())
}
