//! `ewe-manifest` — a thin CLI over the [`crate::manifest`] library API (F40).
//!
//! WHY: build scripts call the library automatically, but key rotation,
//! debugging a rejected manifest, and signing a bundle produced elsewhere are
//! manual jobs. Reimplementing any of that in the CLI would let the two
//! diverge, and a signature check that disagrees with the one on device is
//! worse than no check at all — so every subcommand here is a wrapper.
//!
//! WHAT: `init-keys`, `key-info`, `generate-manifests`, `sign-manifest`,
//! `verify-manifest`.
//!
//! HOW: hand-rolled argument parsing. The surface is five verbs with at most
//! two options; a parser dependency would outweigh the code it replaced.
//!
//! Native-only: there is no filesystem or key material to work with on wasm32.

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::manifest::{
    self, KeyPair, ManifestError, DEFAULT_MANIFEST_DOMAIN, KEYS_DIR, MANIFEST_FILENAME,
    PRIVATE_KEY_ENV, PRIVATE_KEY_FILE, PUBLIC_KEY_FILE,
};

const USAGE: &str = "\
ewe-manifest — OTA manifest keys, generation, signing, and verification

USAGE:
    ewe-manifest <COMMAND> [ARGS]

COMMANDS:
    init-keys [DIR]
        Create keys/ota_public.key + keys/ota_private.key if absent.
        Idempotent: existing keys are read, never regenerated.
        DIR defaults to the current directory.

    key-info [DIR]
        Print the public key and whether a private key is available.

    generate-manifests <PUBLIC_DIR> --version <SEMVER> [--domain <DOMAIN>]
        Write .ewe_manifest.json into every app directory under PUBLIC_DIR
        (an app is a directory containing index.html).

    sign-manifest <FILE>
        Print a base64 Ed25519 signature over FILE's canonical JSON.

    verify-manifest <FILE> [--key <FILE>]
        Check FILE's signature against a public key.
        Defaults to keys/ota_public.key.

ENVIRONMENT:
    EWE_OTA_PRIVATE_KEY   base64 private key seed; overrides the key file
    EWE_MANIFEST_DOMAIN   default CDN domain for generate-manifests
";

/// Run the CLI over already-split arguments (the binary name removed).
///
/// Returns the process exit code. Data goes to stdout so it can be piped;
/// diagnostics go to stderr.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let Some(command) = args.first().map(String::as_str) else {
        eprint!("{USAGE}");
        return 2;
    };

    let result = match command {
        "init-keys" => init_keys(args.get(1).map_or_else(|| PathBuf::from("."), PathBuf::from)),
        "key-info" => key_info(args.get(1).map_or_else(|| PathBuf::from("."), PathBuf::from)),
        "generate-manifests" => generate_manifests(&args[1..]),
        "sign-manifest" => sign_manifest(&args[1..]),
        "verify-manifest" => verify_manifest(&args[1..]),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            return 0;
        }
        other => Err(format!("unknown command: {other}\n\n{USAGE}")),
    };

    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("error: {message}");
            1
        }
    }
}

fn init_keys(dir: PathBuf) -> Result<(), String> {
    let keypair = manifest::ensure_keys(&dir).map_err(fmt_err)?;
    let keys = dir.join(KEYS_DIR);
    println!("{}/{PUBLIC_KEY_FILE} — OK", keys.display());
    if keypair.can_sign() {
        println!("{}/{PRIVATE_KEY_FILE} — OK", keys.display());
        println!("Keys are ready.");
    } else {
        println!("{}/{PRIVATE_KEY_FILE} — absent", keys.display());
        println!("Keys are ready for verification only; set {PRIVATE_KEY_ENV} to sign.");
    }
    Ok(())
}

fn key_info(dir: PathBuf) -> Result<(), String> {
    let path = dir.join(KEYS_DIR).join(PUBLIC_KEY_FILE);
    if !path.exists() {
        return Err(format!(
            "{} does not exist — run `ewe-manifest init-keys` first",
            path.display()
        ));
    }
    let public = manifest::read_public_key(&path).map_err(fmt_err)?;
    let private = dir.join(KEYS_DIR).join(PRIVATE_KEY_FILE);

    println!("Public key:  {} (32 bytes, base64)", BASE64.encode(public));
    if std::env::var(PRIVATE_KEY_ENV).is_ok_and(|v| !v.trim().is_empty()) {
        println!("Private key: {PRIVATE_KEY_ENV} (present)");
    } else if private.exists() {
        println!("Private key: {} (present)", private.display());
    } else {
        println!("Private key: absent");
    }
    Ok(())
}

fn generate_manifests(args: &[String]) -> Result<(), String> {
    let public_dir = args
        .first()
        .map(PathBuf::from)
        .ok_or("generate-manifests needs a PUBLIC_DIR")?;
    let version = flag(args, "--version").ok_or("generate-manifests needs --version <SEMVER>")?;
    let domain = flag(args, "--domain").unwrap_or_else(manifest::manifest_domain_from_env);

    // Keys live beside the public dir's project root, which is its parent.
    let project_root = public_dir.parent().unwrap_or(Path::new("."));
    let keypair = manifest::ensure_keys(project_root).map_err(fmt_err)?;
    if !keypair.can_sign() {
        return Err(format!(
            "no private key available — every manifest is signed. \
             Set {PRIVATE_KEY_ENV}, or run `ewe-manifest init-keys` where \
             keys/ is writable."
        ));
    }

    let manifests = manifest::generate_all_manifests(&public_dir, &version, &domain, &keypair)
        .map_err(fmt_err)?;

    if manifests.is_empty() {
        println!("no app directories found under {}", public_dir.display());
        return Ok(());
    }
    for manifest in &manifests {
        for app in &manifest.apps {
            let n = app.files.len();
            println!(
                "{}/{MANIFEST_FILENAME} — {n} file{s}, signed",
                public_dir.join(&app.app_id).display(),
                s = if n == 1 { "" } else { "s" },
            );
        }
    }
    Ok(())
}

fn sign_manifest(args: &[String]) -> Result<(), String> {
    let path = args.first().map(PathBuf::from).ok_or("sign-manifest needs a FILE")?;
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("reading {}: {e}", path.display()))?;

    let project_root = path.parent().unwrap_or(Path::new("."));
    let keypair: KeyPair = manifest::ensure_keys(project_root).map_err(fmt_err)?;
    let seed = keypair
        .private
        .ok_or_else(|| format!("no private key available — set {PRIVATE_KEY_ENV}"))?;

    println!("{}", manifest::sign_manifest(&json, &seed).map_err(fmt_err)?);
    Ok(())
}

fn verify_manifest(args: &[String]) -> Result<(), String> {
    let path = args.first().map(PathBuf::from).ok_or("verify-manifest needs a FILE")?;
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("reading {}: {e}", path.display()))?;

    let key_path = flag(args, "--key").map_or_else(
        || {
            path.parent()
                .unwrap_or(Path::new("."))
                .join(KEYS_DIR)
                .join(PUBLIC_KEY_FILE)
        },
        PathBuf::from,
    );
    let public = manifest::read_public_key(&key_path).map_err(fmt_err)?;

    match manifest::verify_manifest(&json, &public) {
        Ok(manifest) => {
            println!("Signature:  VALID");
            println!("Public key: {}", key_path.display());
            println!("Source:     {}", manifest.source);
            println!("Sequence:   {}", manifest.sequence);
            println!("Domain:     {}", manifest.manifest_domain);
            for app in &manifest.apps {
                println!("App:        {app}");
            }
            Ok(())
        }
        // A failed verification is the CLI working correctly, so it reports
        // the reason and exits non-zero rather than pretending to succeed.
        Err(e) => Err(format!("Signature: INVALID — {e}")),
    }
}

/// Read `--name value` out of an argument list.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn fmt_err(e: ManifestError) -> String {
    e.to_string()
}

/// Re-exported so `main.rs` does not need to know the default domain.
pub const DEFAULT_DOMAIN: &str = DEFAULT_MANIFEST_DOMAIN;
