//! The libkrun runner: one process becomes one virtual machine.
//!
//! The driver starts this program as `sandbox-krun <run_dir>/krun.json`. It
//! reads the configuration, loads libkrun at run time, and calls
//! `krun_start_enter`, which does not return on success. The process then runs
//! the guest program and exits with its code (feature 09, section 3).
//!
//! `sandbox-krun --probe [--libkrun <path>]` loads the library, resolves every
//! symbol, prints one JSON line and exits 0. The host check of the driver runs
//! it.
//!
//! This program and `src/native/libkrun/ffi.rs` are the only code that names a
//! `krun_*` symbol (invariant I1).

/// The exit code of a bad command line or a bad JSON file.
const EXIT_USAGE: i32 = 64;
/// The exit code of a libkrun call that returned an error.
const EXIT_CALL_FAILED: i32 = 70;
/// The exit code of a library or symbol that is missing.
const EXIT_LIBRARY: i32 = 71;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn main() {
    use foundation_sandbox::native::libkrun::ffi;
    use foundation_sandbox::shared::krun_config::KrunRunnerConfig;
    use std::path::PathBuf;

    fn run() -> Result<i32, (i32, String)> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(String::as_str) {
            Some("--probe") => probe(&args[1..]),
            Some(path) if !path.starts_with("--") => launch(path, &args[1..]),
            _ => Err((EXIT_USAGE, usage())),
        }
    }

    fn usage() -> String {
        "usage: sandbox-krun <krun.json> | --probe [--libkrun <path>]".to_string()
    }

    fn probe(rest: &[String]) -> Result<i32, (i32, String)> {
        let mut library: Option<PathBuf> = None;
        let mut index = 0;
        while index < rest.len() {
            match rest[index].as_str() {
                "--libkrun" => {
                    let Some(value) = rest.get(index + 1) else {
                        return Err((EXIT_USAGE, usage()));
                    };
                    library = Some(PathBuf::from(value));
                    index += 2;
                }
                other => {
                    return Err((EXIT_USAGE, format!("unknown argument {other}; {}", usage())));
                }
            }
        }
        let loaded = ffi::KrunLibrary::load(library.as_deref(), false)
            .map_err(|error| (EXIT_LIBRARY, error.to_string()))?;
        let answer = loaded
            .probe()
            .map_err(|error| (EXIT_LIBRARY, error.to_string()))?;
        let line = serde_json::to_string(&answer).map_err(|error| {
            (
                EXIT_CALL_FAILED,
                format!("encode the probe answer: {error}"),
            )
        })?;
        println!("{line}");
        Ok(0)
    }

    fn launch(path: &str, rest: &[String]) -> Result<i32, (i32, String)> {
        if let Some(other) = rest.first() {
            return Err((
                EXIT_USAGE,
                format!("unexpected argument {other}; {}", usage()),
            ));
        }
        let bytes = std::fs::read(path)
            .map_err(|error| (EXIT_USAGE, format!("cannot read {path}: {error}")))?;
        let config: KrunRunnerConfig = serde_json::from_slice(&bytes)
            .map_err(|error| (EXIT_USAGE, format!("bad JSON in {path}: {error}")))?;
        let loaded =
            ffi::KrunLibrary::load(config.libkrun_path.as_deref(), !config.disks.is_empty())
                .map_err(|error| (EXIT_LIBRARY, error.to_string()))?;
        loaded
            .run(&config)
            .map_err(|error| (EXIT_CALL_FAILED, error.to_string()))
    }

    let code = match run() {
        Ok(code) => code,
        Err((code, message)) => {
            eprintln!("sandbox-krun: {message}");
            code
        }
    };
    std::process::exit(code);
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn main() {
    eprintln!("sandbox-krun: libkrun runs on Linux and macOS only");
    std::process::exit(EXIT_LIBRARY);
}
