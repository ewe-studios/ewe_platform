//! `genapi normalize <provider>` — raw vendor spec → canonical artefact.
//!
//! **WHY:** the generators need a canonical OpenAPI document (schemas in
//! `components/schemas`, `servers` present, `$ref`s rather than inline schemas),
//! and vendors do not ship one. Measured 2026-07-17: Linode (9.3 MB) and Hetzner
//! (3.4 MB) publish **fully bundled** specs — zero `$ref`s — and our extractor
//! names a response's type from its `ref_path`, so their endpoints would generate
//! as anonymous `serde_json::Value` blobs. Even Cloudflare, generated today, has
//! 3090 inline schemas (1224 of its 2442 request fns return `Value` or `()`).
//!
//! **WHAT:** a registry of `provider → normalizer → raw file`, and one command
//! that reads the committed raw spec, applies that provider's transforms,
//! **validates the result is canonical**, and writes the artefact plus its
//! manifest.
//!
//! **HOW:** file → file, no network (spec-56 decision 05). The raw spec is
//! committed under `artefacts/cloud_providers/raw/`, so normalization is
//! deterministic and offline, a transform bug is fixable without re-fetching, and
//! the diff between `raw/` and the artefact *is* what we did to the vendor's spec.
//! Refreshing a vendor is a separate, occasional act: drop a new raw file in,
//! re-run, review the diff.

use std::path::{Path, PathBuf};

use foundation_openapi::{
    canonicalize_operations, dangling_refs, ensure_servers, normalize_nullable_types,
    strip_doc_only, validate_canonical,
};
use serde_json::Value;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Which transforms a provider's spec needs.
///
/// Named for **what runs**, not for what the spec looks like: "DigitalOcean is
/// ref-based so it needs nothing" was exactly the assumption that turned out
/// false — it has 7814 refs *and* 58 inline schemas. The validator caught it; the
/// naming should not invite the guess again.
///
/// A provider is one registry entry; a new one is an entry plus — only if its
/// quirks are new — a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Normalizer {
    /// Validate only. For a spec that genuinely needs nothing — rare, and prove
    /// it rather than assume it.
    ValidateOnly,
    /// Hoist inline operation schemas into `components/schemas`.
    ///
    /// Safe on any spec: it only touches schemas that are written in place, so a
    /// mostly-ref-based spec (DigitalOcean) keeps its refs and gets its stragglers
    /// named.
    HoistInline,
    /// [`Self::HoistInline`], preceded by flattening OpenAPI 3.1's
    /// `"type": ["string", "null"]` arrays, which the generator does not model.
    HoistInlineFrom31,
}

/// A provider's spec and how to canonicalise it.
#[derive(Debug, Clone)]
pub struct ProviderSpec {
    /// Provider name — also the artefact directory.
    pub name: &'static str,
    /// The committed raw spec, relative to the workspace root.
    pub raw: &'static str,
    /// Where the vendor published it, for the manifest. Provenance, not a build
    /// dependency — nothing fetches this.
    pub source: &'static str,
    /// Base URL to insert when the spec declares no `servers`.
    pub base_url: &'static str,
    /// The transforms this spec needs.
    pub normalizer: Normalizer,
}

/// Every provider whose raw spec we canonicalise.
///
/// Adding one is an entry here plus its raw file under
/// `artefacts/cloud_providers/raw/`.
pub const PROVIDER_SPECS: &[ProviderSpec] = &[
    ProviderSpec {
        name: "hetzner",
        raw: "artefacts/cloud_providers/raw/hetzner.json",
        source: "https://docs.hetzner.cloud/cloud.spec.json",
        base_url: "https://api.hetzner.cloud/v1",
        // Bundled (0 $refs), and OpenAPI 3.1.2 — nullable type arrays.
        normalizer: Normalizer::HoistInlineFrom31,
    },
    ProviderSpec {
        name: "linode",
        raw: "artefacts/cloud_providers/raw/linode.json",
        source: "https://github.com/linode/linode-api-openapi",
        base_url: "https://api.linode.com",
        // Bundled: 0 $refs — every schema inlined per operation.
        normalizer: Normalizer::HoistInline,
    },
    ProviderSpec {
        name: "digitalocean",
        raw: "artefacts/cloud_providers/raw/digitalocean.json",
        source: "https://api-engineering.nyc3.digitaloceanspaces.com/spec-ci/DigitalOcean-public.v2.yaml",
        base_url: "https://api.digitalocean.com",
        // 7814 $refs (into responses/headers/parameters too) — but ALSO 58 inline
        // operation schemas. Ref-heavy is not the same as canonical, which is why
        // this runs the hoist rather than passing through.
        normalizer: Normalizer::HoistInline,
    },
];

/// Look a provider up in the registry.
#[must_use]
pub fn provider_spec(name: &str) -> Option<&'static ProviderSpec> {
    PROVIDER_SPECS.iter().find(|p| p.name == name)
}

/// What a normalize run did, for the manifest and the console.
#[derive(Debug, Clone, Default)]
pub struct NormalizeReport {
    /// Inline schemas hoisted into `components/schemas`.
    pub hoisted: usize,
    /// Hoisted names that collided and were suffixed.
    pub renamed: usize,
    /// Inline schemas before.
    pub inline_before: usize,
    /// `components/schemas` after.
    pub schemas_after: usize,
    /// The spec's `info.version`, for the pin to check against.
    pub spec_version: Option<String>,
    /// The spec's `openapi` version.
    pub openapi_version: Option<String>,
    /// Which transforms ran, in order.
    pub transforms: Vec<String>,
}

/// Canonicalise `provider`'s committed raw spec into its artefact directory.
///
/// # Errors
/// Fails when the raw spec is missing or unparseable, or — deliberately loudly —
/// when the transforms leave a spec that is **not canonical** or has dangling
/// refs. Running transforms and hoping is how a crate ends up generating quietly
/// wrong code.
pub fn normalize_provider(
    spec: &ProviderSpec,
    workspace_root: &Path,
    out_dir: &Path,
) -> Result<NormalizeReport, BoxedError> {
    let raw_path = workspace_root.join(spec.raw);
    let raw = std::fs::read_to_string(&raw_path)
        .map_err(|e| -> BoxedError { format!("cannot read raw spec {}: {e}", raw_path.display()).into() })?;
    let mut doc: Value = serde_json::from_str(&raw)
        .map_err(|e| -> BoxedError { format!("raw spec {} is not JSON: {e}", raw_path.display()).into() })?;

    let mut report = NormalizeReport {
        inline_before: inline_count(&doc),
        spec_version: doc
            .pointer("/info/version")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        openapi_version: doc.get("openapi").and_then(Value::as_str).map(ToString::to_string),
        ..Default::default()
    };

    // Every provider: drop what documents the API but generates nothing. It is
    // 34% of DigitalOcean's spec, and its `example`/`x-codeSamples` values carry
    // Slack webhook URLs — the vendor's own placeholders, but they trip secret
    // scanning on an artefact we have to commit.
    strip_doc_only(&mut doc);
    report.transforms.push("strip_doc_only".to_string());

    ensure_servers(&mut doc, spec.base_url);
    report.transforms.push("ensure_servers".to_string());

    match spec.normalizer {
        Normalizer::ValidateOnly => {}
        Normalizer::HoistInline => {
            let stats = canonicalize_operations(&mut doc);
            report.hoisted = stats.hoisted;
            report.renamed = stats.renamed;
            report.transforms.push("canonicalize_operations".to_string());
        }
        Normalizer::HoistInlineFrom31 => {
            normalize_nullable_types(&mut doc);
            report.transforms.push("normalize_nullable_types".to_string());
            let stats = canonicalize_operations(&mut doc);
            report.hoisted = stats.hoisted;
            report.renamed = stats.renamed;
            report.transforms.push("canonicalize_operations".to_string());
        }
    }

    // The proof, not the hope.
    if let Err(issues) = validate_canonical(&doc) {
        let mut msg = format!(
            "{}: still not canonical after {:?} — {} problem(s):\n",
            spec.name,
            report.transforms,
            issues.len()
        );
        for issue in issues.iter().take(5) {
            msg.push_str(&format!("  - {issue}\n"));
        }
        if issues.len() > 5 {
            msg.push_str(&format!("  … and {} more\n", issues.len() - 5));
        }
        return Err(msg.into());
    }
    let dangling = dangling_refs(&doc);
    if !dangling.is_empty() {
        return Err(format!(
            "{}: {} dangling $ref(s) after normalization, e.g. {:?}",
            spec.name,
            dangling.len(),
            &dangling[..dangling.len().min(3)]
        )
        .into());
    }

    report.schemas_after = foundation_openapi::component_count(&doc, "schemas");

    let provider_dir = out_dir.join(spec.name);
    std::fs::create_dir_all(&provider_dir)
        .map_err(|e| -> BoxedError { format!("cannot create {}: {e}", provider_dir.display()).into() })?;

    let canonical_path = provider_dir.join("openapi.json");
    let body = serde_json::to_string_pretty(&doc)
        .map_err(|e| -> BoxedError { format!("cannot serialize canonical spec: {e}").into() })?;
    std::fs::write(&canonical_path, &body)
        .map_err(|e| -> BoxedError { format!("cannot write {}: {e}", canonical_path.display()).into() })?;

    write_manifest(spec, &provider_dir, &report, &raw, &body)?;

    Ok(report)
}

/// Provenance: where it came from, what we did, and the pins.
fn write_manifest(
    spec: &ProviderSpec,
    provider_dir: &Path,
    report: &NormalizeReport,
    raw: &str,
    canonical: &str,
) -> Result<(), BoxedError> {
    let manifest = serde_json::json!({
        "provider": spec.name,
        "source": spec.source,
        "raw": spec.raw,
        "normalized_at": now_rfc3339(),
        // What feature 00's pin validates against — read from here rather than
        // re-parsing a 9.3 MB document.
        "spec_version": report.spec_version,
        "openapi": report.openapi_version,
        // Otherwise "why does this differ from the vendor's?" is unanswerable.
        "transforms": report.transforms,
        "hoisted": report.hoisted,
        "renamed": report.renamed,
        "schemas": report.schemas_after,
        // Detects the vendor moving the spec, and the artefact being hand-edited.
        "raw_sha256": sha256_hex(raw.as_bytes()),
        "canonical_sha256": sha256_hex(canonical.as_bytes()),
        "spec_files": [format!("{}/openapi.json", spec.name)],
    });

    let path = provider_dir.join("_manifest.json");
    let body = serde_json::to_string_pretty(&manifest)
        .map_err(|e| -> BoxedError { format!("cannot serialize manifest: {e}").into() })?;
    std::fs::write(&path, body)
        .map_err(|e| -> BoxedError { format!("cannot write {}: {e}", path.display()).into() })?;
    Ok(())
}

/// Inline operation schemas, before we start — for the report.
fn inline_count(doc: &Value) -> usize {
    validate_canonical(doc).err().map_or(0, |issues| {
        issues
            .iter()
            .filter(|i| matches!(i, foundation_openapi::NotCanonical::InlineSchema { .. }))
            .count()
    })
}

fn now_rfc3339() -> String {
    // Seconds since the epoch is enough provenance and costs no dependency.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("{secs}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

// ── CLI ──────────────────────────────────────────────────────────────────────

/// The `normalize` subcommand.
#[must_use]
pub fn command() -> clap::Command {
    clap::Command::new("normalize")
        .about("Turn a provider's committed raw spec into a canonical OpenAPI artefact")
        .arg(
            clap::Arg::new("provider")
                .help("Provider name, or `all`")
                .required(true)
                .value_name("PROVIDER"),
        )
        .arg(
            clap::Arg::new("workspace-root")
                .long("workspace-root")
                .help("Workspace root the raw paths are relative to (default: cwd)")
                .value_name("DIR"),
        )
        .arg(
            clap::Arg::new("out-dir")
                .long("out-dir")
                .help("Artefact directory (default: artefacts/cloud_providers)")
                .value_name("DIR"),
        )
}

/// Run `normalize`.
///
/// # Errors
/// Returns an error for an unknown provider, or any failure from
/// [`normalize_provider`].
pub fn run(matches: &clap::ArgMatches) -> Result<(), BoxedError> {
    let provider = matches
        .get_one::<String>("provider")
        .map(String::as_str)
        .unwrap_or("all");
    let workspace_root = matches
        .get_one::<String>("workspace-root")
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let out_dir = matches.get_one::<String>("out-dir").map_or_else(
        || workspace_root.join("artefacts/cloud_providers"),
        PathBuf::from,
    );

    let targets: Vec<&ProviderSpec> = if provider == "all" {
        PROVIDER_SPECS.iter().collect()
    } else {
        vec![provider_spec(provider).ok_or_else(|| -> BoxedError {
            format!(
                "unknown provider `{provider}` — known: {}",
                PROVIDER_SPECS
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .into()
        })?]
    };

    for spec in targets {
        let report = normalize_provider(spec, &workspace_root, &out_dir)?;
        println!(
            "{}: inline {} -> 0 | hoisted {} (renamed {}) | schemas {} | spec_version {} | {:?}",
            spec.name,
            report.inline_before,
            report.hoisted,
            report.renamed,
            report.schemas_after,
            report.spec_version.as_deref().unwrap_or("<none>"),
            report.transforms,
        );
    }

    Ok(())
}
