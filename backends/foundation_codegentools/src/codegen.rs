//! One declaration of what a crate generates, and the two verbs over it.
//!
//! **WHY:** [`foundation_openapi::Pipeline`] can already cut a spec down to the
//! slice a crate uses — but nothing called it, so `genapi generate` still fed the
//! whole document to the generator and every path came out the other side. This
//! is the seam that makes the selection *reach* the generator, and the
//! build.rs-callable form spec-56 feature 00 §4 asks for.
//!
//! **WHAT:** [`generate`] returns a [`Codegen`] — provider, spec, pins, selection,
//! destination — with [`Codegen::check`] (is the committed output still what this
//! declaration produces?) and [`Codegen::write`] (produce it).
//!
//! **HOW:** the declaration resolves through `Pipeline` (select → canonicalise →
//! prune), and the resolved spec — not the vendor's — is what
//! [`UnifiedGenerator`] sees. So selection is enforced at the only place it
//! cannot be forgotten: the generator is never *shown* the paths we did not ask
//! for.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let codegen = foundation_codegentools::generate()
//!     .provider("hetzner")
//!     .spec("artefacts/cloud_providers/hetzner/openapi.json")
//!     .api_version("v1")
//!     .spec_version("1.0.0")
//!     .include_paths(["/servers", "/servers/{id}", "/ssh_keys"])
//!     .crate_dir("backends/foundation_deployment_hetzner");
//!
//! codegen.check()?;   // a build script or CI: fails aloud, never writes
//! codegen.write()?;   // the CLI or a deliberate developer step
//! # Ok(())
//! # }
//! ```
//!
//! **Which verb belongs where** (feature 00 §5): these crates are publishable, so
//! a build script calling `write()` would leave a crates.io consumer needing an
//! unpublished spec artefact and writing into a read-only registry directory.
//! `check()` never writes — it generates into a temporary directory and compares.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use foundation_openapi::{AnalysisOptions, Pipeline, PipelineError, Selection, UnifiedGenerator};

/// Why a codegen run failed.
#[derive(Debug)]
pub enum CodegenError {
    /// The declaration is missing something it cannot default.
    Incomplete {
        /// Which builder method was never called.
        missing: &'static str,
    },
    /// Resolving the spec failed — unreadable, unparseable, a moved pin, or a
    /// result that is not canonical.
    Pipeline(PipelineError),
    /// The generator failed on the resolved spec.
    Generate(foundation_openapi::GenError),
    /// Reading or writing the output tree failed.
    Io {
        /// What we were touching.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// [`Codegen::check`] found the committed output is not what this declaration
    /// produces.
    Stale {
        /// The directory that is out of date.
        generated_dir: PathBuf,
        /// Files this declaration produces that are not committed.
        added: Vec<String>,
        /// Committed files this declaration no longer produces.
        removed: Vec<String>,
        /// Files whose contents differ.
        changed: Vec<String>,
    },
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete { missing } => {
                write!(f, "codegen declaration is missing .{missing}()")
            }
            Self::Pipeline(e) => write!(f, "{e}"),
            Self::Generate(e) => write!(f, "generation failed: {e}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Stale {
                generated_dir,
                added,
                removed,
                changed,
            } => {
                writeln!(
                    f,
                    "{} is stale — regenerate it (genapi generate):",
                    generated_dir.display()
                )?;
                // Name the files. "something changed" sends the reader looking;
                // the whole point of check() is that it says what.
                for (label, files) in [
                    ("would be added", added),
                    ("would be removed", removed),
                    ("differs", changed),
                ] {
                    for file in files.iter().take(10) {
                        writeln!(f, "  {label}: {file}")?;
                    }
                    if files.len() > 10 {
                        writeln!(f, "  … and {} more {label}", files.len() - 10)?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CodegenError {}

impl From<PipelineError> for CodegenError {
    fn from(e: PipelineError) -> Self {
        Self::Pipeline(e)
    }
}

/// What [`Codegen::write`] produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenReport {
    /// Where the code landed.
    pub generated_dir: PathBuf,
    /// Paths kept from the spec, after selection.
    pub paths_selected: usize,
    /// `components/schemas` in the resolved spec — the closure of what we kept.
    pub schemas: usize,
    /// Files written.
    pub files: usize,
}

/// Start a codegen declaration.
///
/// See the [module docs](self) for the shape and for which verb belongs where.
#[must_use]
pub fn generate() -> Codegen {
    Codegen::default()
}

/// What a crate generates, declared once.
#[derive(Debug, Clone)]
pub struct Codegen {
    provider: Option<String>,
    spec: Option<PathBuf>,
    api_version: Option<String>,
    spec_version: Option<String>,
    base_url: Option<String>,
    selection: Selection,
    crate_dir: Option<PathBuf>,
    provider_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    options: AnalysisOptions,
}

impl Default for Codegen {
    fn default() -> Self {
        Self {
            provider: None,
            spec: None,
            api_version: None,
            spec_version: None,
            base_url: None,
            selection: Selection::all(),
            crate_dir: None,
            provider_dir: None,
            output_dir: None,
            // The thresholds `genapi generate` has always defaulted to; a
            // declaration that says nothing about grouping gets what the CLI gives.
            options: AnalysisOptions {
                min_group_size: 10,
                max_group_size: 200,
            },
        }
    }
}

impl Codegen {
    /// The provider this generates for — names the module and the feature flags.
    #[must_use]
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// The canonical spec to generate from — an artefact written by
    /// `genapi normalize`, not a vendor's raw document.
    #[must_use]
    pub fn spec(mut self, path: impl Into<PathBuf>) -> Self {
        self.spec = Some(path.into());
        self
    }

    /// Pin the vendor's API version (Hetzner `v1`, Linode `v4`, DO `v2`).
    #[must_use]
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = Some(version.into());
        self
    }

    /// Pin the spec revision (`info.version`). A mismatch is fatal — see
    /// [`PipelineError::SpecVersionMismatch`].
    #[must_use]
    pub fn spec_version(mut self, version: impl Into<String>) -> Self {
        self.spec_version = Some(version.into());
        self
    }

    /// Base URL to use if the spec declares no `servers` entry.
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Generate only these paths. Globs are segment-aware: `*` matches one
    /// segment, `**` matches many.
    #[must_use]
    pub fn include_paths<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selection = self.selection.paths(patterns);
        self
    }

    /// Generate only operations carrying these tags.
    #[must_use]
    pub fn include_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selection = self.selection.tags(tags);
        self
    }

    /// Generate only these `operationId`s.
    #[must_use]
    pub fn include_operations<I, S>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selection = self.selection.operations(ids);
        self
    }

    /// Replace the selection wholesale — for a caller that already has one.
    #[must_use]
    pub fn select(mut self, selection: Selection) -> Self {
        self.selection = selection;
        self
    }

    /// The crate to generate into: code lands in `<crate_dir>/src/generated/`.
    ///
    /// The ergonomic form, and what a provider crate wants — its `build.rs` calls
    /// `.crate_dir(env!("CARGO_MANIFEST_DIR"))`. Use [`Codegen::output_dir`] +
    /// [`Codegen::provider_dir`] for the monolith layout the CLI still drives.
    #[must_use]
    pub fn crate_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.crate_dir = Some(dir.into());
        self
    }

    /// The generator's output root — where it looks for `Cargo.toml` to add
    /// feature flags to.
    #[must_use]
    pub fn output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    /// Write files to this directory's `generated/` rather than
    /// `<output_dir>/<provider>/generated/`.
    #[must_use]
    pub fn provider_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.provider_dir = Some(dir.into());
        self
    }

    /// Grouping thresholds handed to the analyzer.
    #[must_use]
    pub fn group_sizes(mut self, min: usize, max: usize) -> Self {
        self.options = AnalysisOptions {
            min_group_size: min,
            max_group_size: max,
        };
        self
    }

    /// The slice of the spec this declaration generates: select → canonicalise →
    /// prune, with the pins verified.
    ///
    /// # Errors
    /// [`CodegenError::Incomplete`] for a declaration missing `provider`/`spec`,
    /// or [`CodegenError::Pipeline`] for anything the pipeline rejects.
    pub fn resolve(&self) -> Result<serde_json::Value, CodegenError> {
        Ok(self.pipeline()?.resolve()?)
    }

    fn pipeline(&self) -> Result<Pipeline, CodegenError> {
        let provider = self
            .provider
            .as_deref()
            .ok_or(CodegenError::Incomplete { missing: "provider" })?;
        let spec = self
            .spec
            .as_ref()
            .ok_or(CodegenError::Incomplete { missing: "spec" })?;

        let mut pipeline = Pipeline::new(provider, spec).select(self.selection.clone());
        if let Some(v) = &self.api_version {
            pipeline = pipeline.api_version(v);
        }
        if let Some(v) = &self.spec_version {
            pipeline = pipeline.spec_version(v);
        }
        if let Some(url) = &self.base_url {
            pipeline = pipeline.base_url(url);
        }
        Ok(pipeline)
    }

    /// Where generated code lands, given this declaration.
    ///
    /// # Errors
    /// [`CodegenError::Incomplete`] when the declaration names no destination.
    pub fn generated_dir(&self) -> Result<PathBuf, CodegenError> {
        let (output, provider_dir) = self.roots()?;
        Ok(self.provider_root(output, &provider_dir)?.join("generated"))
    }

    /// `(output_dir, provider_dir_override)` as the generator wants them.
    fn roots(&self) -> Result<(PathBuf, Option<PathBuf>), CodegenError> {
        if let Some(crate_dir) = &self.crate_dir {
            // A split-out provider crate: the crate root is the generator's
            // output root (its Cargo.toml is the one to add features to), and the
            // code goes beside the hand-written client in src/.
            return Ok((crate_dir.clone(), Some(crate_dir.join("src"))));
        }
        let output = self
            .output_dir
            .clone()
            .ok_or(CodegenError::Incomplete { missing: "crate_dir" })?;
        Ok((output, self.provider_dir.clone()))
    }

    fn provider_root(&self, output: PathBuf, provider_dir: &Option<PathBuf>) -> Result<PathBuf, CodegenError> {
        Ok(match provider_dir {
            Some(dir) => dir.clone(),
            None => {
                let provider = self
                    .provider
                    .as_deref()
                    .ok_or(CodegenError::Incomplete { missing: "provider" })?;
                output.join(provider)
            }
        })
    }

    /// Resolve and generate into `<crate_dir>/src/generated/`.
    ///
    /// For the CLI or a deliberate developer step — **not** a build script of a
    /// publishable crate (feature 00 §5).
    ///
    /// # Errors
    /// [`CodegenError`] for an incomplete declaration, anything the pipeline
    /// rejects (notably a moved spec pin), or a generator/IO failure.
    pub fn write(&self) -> Result<CodegenReport, CodegenError> {
        let resolved = self.resolve()?;
        let (output, provider_dir) = self.roots()?;
        self.run_generator(&resolved, output.clone(), provider_dir.clone())?;

        let generated_dir = self.provider_root(output, &provider_dir)?.join("generated");
        Ok(CodegenReport {
            paths_selected: resolved
                .get("paths")
                .and_then(serde_json::Value::as_object)
                .map_or(0, serde_json::Map::len),
            schemas: foundation_openapi::component_count(&resolved, "schemas"),
            files: read_tree(&generated_dir)?.len(),
            generated_dir,
        })
    }

    /// Resolve, generate into a temporary directory, and compare against what is
    /// committed. **Never writes** to the crate.
    ///
    /// This is the build-script and CI verb. It fails when the spec has moved
    /// under the pin, or when the committed `src/generated/` is not what this
    /// declaration produces.
    ///
    /// # Errors
    /// [`CodegenError::Stale`] naming the files that differ, plus anything
    /// [`Codegen::write`] can return.
    pub fn check(&self) -> Result<(), CodegenError> {
        let resolved = self.resolve()?;
        let generated_dir = self.generated_dir()?;

        // Generate into a scratch tree with the same shape, so the comparison is
        // like-for-like. The generator only touches Cargo.toml if it finds one,
        // and there is none here — so check() cannot edit the crate's manifest
        // either.
        let scratch = tempfile::tempdir().map_err(|source| CodegenError::Io {
            path: PathBuf::from("<temp>"),
            source,
        })?;
        let (_, provider_dir) = self.roots()?;
        let scratch_provider_dir = provider_dir.as_ref().map(|_| scratch.path().join("src"));
        self.run_generator(
            &resolved,
            scratch.path().to_path_buf(),
            scratch_provider_dir.clone(),
        )?;
        let fresh_dir = self
            .provider_root(scratch.path().to_path_buf(), &scratch_provider_dir)?
            .join("generated");

        let fresh = read_tree(&fresh_dir)?;
        let committed = read_tree(&generated_dir)?;

        let added: Vec<String> = fresh
            .keys()
            .filter(|f| !committed.contains_key(*f))
            .cloned()
            .collect();
        let removed: Vec<String> = committed
            .keys()
            .filter(|f| !fresh.contains_key(*f))
            .cloned()
            .collect();
        let changed: Vec<String> = fresh
            .iter()
            .filter(|(name, body)| committed.get(*name).is_some_and(|c| c != *body))
            .map(|(name, _)| name.clone())
            .collect();

        if added.is_empty() && removed.is_empty() && changed.is_empty() {
            Ok(())
        } else {
            Err(CodegenError::Stale {
                generated_dir,
                added,
                removed,
                changed,
            })
        }
    }

    /// Hand the **resolved** spec to the generator — never the vendor's document.
    fn run_generator(
        &self,
        resolved: &serde_json::Value,
        output: PathBuf,
        provider_dir: Option<PathBuf>,
    ) -> Result<(), CodegenError> {
        let provider = self
            .provider
            .as_deref()
            .ok_or(CodegenError::Incomplete { missing: "provider" })?;

        let mut generator = UnifiedGenerator::new(output);
        if let Some(dir) = provider_dir {
            generator = generator.with_provider_dir(dir);
        }
        let body = serde_json::to_string(resolved).expect("a resolved spec serializes");
        generator
            .generate(provider, &body, &self.options)
            .map_err(CodegenError::Generate)
    }
}

/// Every file under `dir`, keyed by its path relative to `dir`.
///
/// A missing directory reads as empty rather than an error: "nothing has been
/// generated yet" is a legitimate `check()` outcome, and it should report as
/// stale-with-additions, not as an IO failure.
fn read_tree(dir: &Path) -> Result<BTreeMap<String, String>, CodegenError> {
    let mut files = BTreeMap::new();
    if !dir.exists() {
        return Ok(files);
    }
    walk(dir, dir, &mut files)?;
    Ok(files)
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> Result<(), CodegenError> {
    let entries = std::fs::read_dir(dir).map_err(|source| CodegenError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| CodegenError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let body = std::fs::read_to_string(&path).map_err(|source| CodegenError::Io {
                path: path.clone(),
                source,
            })?;
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, body);
        }
    }
    Ok(())
}

// ── the build script a provider crate carries ────────────────────────────────

/// What [`Codegen::write_build_script`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildScriptOutcome {
    /// Written — the crate had none.
    Created(PathBuf),
    /// Overwritten, because the caller asked.
    Replaced(PathBuf),
    /// Left alone: one exists and `overwrite` was not set.
    Kept(PathBuf),
}

impl BuildScriptOutcome {
    /// Where the script is, whatever happened to it.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Created(p) | Self::Replaced(p) | Self::Kept(p) => p,
        }
    }
}

impl Codegen {
    /// Write a `build.rs` carrying this declaration into the crate.
    ///
    /// **Why the generator emits this at all:** feature 00 §4 — the slice a crate
    /// generates should be visible *next to the code that uses it*, not buried in
    /// someone's shell history. Before this, Hetzner's six `--include-operation`
    /// flags existed only in a doc comment, and nothing could reproduce them.
    ///
    /// **Why it does not clobber:** a `build.rs` is a source file people edit —
    /// they widen a selection, add a pin, add an unrelated build step. Silently
    /// replacing it on the next `genapi generate` would eat that. Same rule the
    /// generator already applies to a provider's `mod.rs`. Pass `overwrite` to
    /// replace it deliberately (`genapi generate <p> --buildrs-overwrite`).
    ///
    /// # Errors
    /// [`CodegenError::Incomplete`] without a `crate_dir`/`provider`/`spec`, or
    /// [`CodegenError::Io`] if the write fails.
    pub fn write_build_script(&self, overwrite: bool) -> Result<BuildScriptOutcome, CodegenError> {
        let crate_dir = self
            .crate_dir
            .clone()
            .ok_or(CodegenError::Incomplete { missing: "crate_dir" })?;
        let target = crate_dir.join("build.rs");

        if target.exists() && !overwrite {
            return Ok(BuildScriptOutcome::Kept(target));
        }

        let body = self.render_build_script(&crate_dir)?;
        std::fs::create_dir_all(&crate_dir).map_err(|source| CodegenError::Io {
            path: crate_dir.clone(),
            source,
        })?;
        // The script cannot compile without us. Writing it and leaving the crate
        // to discover that is the sort of half-done that makes people distrust
        // the generator.
        ensure_build_dependency(&crate_dir)?;
        let existed = target.exists();
        std::fs::write(&target, body).map_err(|source| CodegenError::Io {
            path: target.clone(),
            source,
        })?;

        Ok(if existed {
            BuildScriptOutcome::Replaced(target)
        } else {
            BuildScriptOutcome::Created(target)
        })
    }

    /// The `build.rs` source for this declaration.
    fn render_build_script(&self, crate_dir: &Path) -> Result<String, CodegenError> {
        use std::fmt::Write as _;

        let provider = self
            .provider
            .as_deref()
            .ok_or(CodegenError::Incomplete { missing: "provider" })?;
        let spec = self
            .spec
            .as_ref()
            .ok_or(CodegenError::Incomplete { missing: "spec" })?;
        let spec_from_crate = relative_to(crate_dir, spec);

        let mut out = String::new();
        writeln!(out, "//! Keeps `src/generated/` honest against the vendor's spec.").unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(
            out,
            "//! **WHY:** this file *is* the declaration of what this crate generates — the"
        )
        .unwrap();
        writeln!(
            out,
            "//! slice, and the versions it was generated from. Keeping it here rather than in"
        )
        .unwrap();
        writeln!(
            out,
            "//! the generator means the surface is visible next to the code that uses it, and"
        )
        .unwrap();
        writeln!(out, "//! anyone can reproduce it (spec-56 feature 00 §4).").unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(out, "//! **WHAT:** verifies the pins still hold, and regenerates when the").unwrap();
        writeln!(out, "//! committed output no longer matches this declaration.").unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(out, "//! **HOW:** the three outcomes are deliberately different:").unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(out, "//! | `check()` says | meaning | what happens |").unwrap();
        writeln!(out, "//! |---|---|---|").unwrap();
        writeln!(out, "//! | `Ok` | output is current | nothing |").unwrap();
        writeln!(out, "//! | `Stale` | the declaration moved | **regenerate** |").unwrap();
        writeln!(
            out,
            "//! | anything else | the spec moved under the pin, or is unreadable | **fail the build** |"
        )
        .unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(
            out,
            "//! A moved spec is fatal on purpose: regenerating against a document that changed"
        )
        .unwrap();
        writeln!(
            out,
            "//! would rewrite this crate's types with nobody choosing to. Bump `spec_version`,"
        )
        .unwrap();
        writeln!(out, "//! rebuild, and read the diff.").unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(
            out,
            "//! Regenerate by hand with:  cargo run --bin genapi --features cli -- generate {provider}"
        )
        .unwrap();
        writeln!(out).unwrap();
        writeln!(out, "use std::path::Path;").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "fn main() {{").unwrap();
        writeln!(
            out,
            "    let manifest = std::env::var(\"CARGO_MANIFEST_DIR\").expect(\"CARGO_MANIFEST_DIR\");"
        )
        .unwrap();
        writeln!(
            out,
            "    let spec = Path::new(&manifest).join({:?});",
            spec_from_crate
        )
        .unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "    // The spec artefact is not published to crates.io, so a consumer building"
        )
        .unwrap();
        writeln!(
            out,
            "    // this crate from the registry has no spec to check against — and the"
        )
        .unwrap();
        writeln!(
            out,
            "    // committed `src/generated/` is exactly what they should be compiling anyway."
        )
        .unwrap();
        writeln!(out, "    // Nothing to do, and failing here would break their build.").unwrap();
        writeln!(out, "    if !spec.exists() {{").unwrap();
        writeln!(out, "        return;").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "    // Rerun only when the spec or this declaration changes. Without these, cargo"
        )
        .unwrap();
        writeln!(
            out,
            "    // watches every file in the package — and since this script writes into"
        )
        .unwrap();
        writeln!(out, "    // `src/`, that would rebuild forever.").unwrap();
        writeln!(out, "    println!(\"cargo:rerun-if-changed={{}}\", spec.display());").unwrap();
        writeln!(out, "    println!(\"cargo:rerun-if-changed=build.rs\");").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "    let codegen = foundation_codegentools::generate()").unwrap();
        writeln!(out, "        .provider({provider:?})").unwrap();
        writeln!(out, "        .spec(&spec)").unwrap();
        if let Some(v) = &self.api_version {
            writeln!(out, "        .api_version({v:?})").unwrap();
        }
        if let Some(v) = &self.spec_version {
            writeln!(out, "        .spec_version({v:?})").unwrap();
        }
        if let Some(url) = &self.base_url {
            writeln!(out, "        .base_url({url:?})").unwrap();
        }
        for (method, values) in [
            ("include_paths", self.selection.selected_path_patterns()),
            ("include_tags", self.selection.selected_tags()),
            ("include_operations", self.selection.selected_operations()),
        ] {
            if !values.is_empty() {
                writeln!(out, "        .{method}([").unwrap();
                for v in values {
                    writeln!(out, "            {v:?},").unwrap();
                }
                writeln!(out, "        ])").unwrap();
            }
        }
        writeln!(out, "        .crate_dir(&manifest);").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "    match codegen.check() {{").unwrap();
        writeln!(out, "        // Already what this declaration produces.").unwrap();
        writeln!(out, "        Ok(()) => {{}}").unwrap();
        writeln!(out, "        // The selection or the spec's shape moved — regenerate.").unwrap();
        writeln!(
            out,
            "        Err(foundation_codegentools::CodegenError::Stale {{ .. }}) => {{"
        )
        .unwrap();
        writeln!(
            out,
            "            codegen.write().expect(\"regenerate {provider}'s API surface\");"
        )
        .unwrap();
        writeln!(
            out,
            "            println!(\"cargo:warning=regenerated {provider}'s src/generated/ from {{}}\", spec.display());"
        )
        .unwrap();
        writeln!(out, "        }}").unwrap();
        writeln!(
            out,
            "        // A moved pin, an unreadable spec, a spec that will not canonicalise."
        )
        .unwrap();
        writeln!(out, "        Err(e) => panic!(\"{{e}}\"),").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}").unwrap();

        Ok(out)
    }
}

/// Add `foundation_codegentools` to the crate's `[build-dependencies]`.
///
/// Idempotent: an existing entry is left exactly as the crate wrote it — someone
/// may have pinned a version or added features, and clobbering that would be the
/// same mistake as clobbering the `build.rs` itself.
fn ensure_build_dependency(crate_dir: &Path) -> Result<(), CodegenError> {
    let manifest_path = crate_dir.join("Cargo.toml");
    let raw = std::fs::read_to_string(&manifest_path).map_err(|source| CodegenError::Io {
        path: manifest_path.clone(),
        source,
    })?;
    let mut manifest: toml_edit::DocumentMut = raw.parse().map_err(|e| CodegenError::Io {
        path: manifest_path.clone(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}")),
    })?;

    let deps = manifest["build-dependencies"].or_insert(toml_edit::table());
    if deps.get(BUILD_DEP).is_some() {
        return Ok(());
    }

    // A path for building in-repo, a version so the crate stays publishable.
    let mut entry = toml_edit::InlineTable::new();
    entry.insert("path", format!("../{BUILD_DEP}").into());
    entry.insert("version", BUILD_DEP_VERSION.into());
    deps[BUILD_DEP] = toml_edit::value(entry);

    std::fs::write(&manifest_path, manifest.to_string()).map_err(|source| CodegenError::Io {
        path: manifest_path,
        source,
    })
}

/// The crate a generated `build.rs` calls into.
const BUILD_DEP: &str = "foundation_codegentools";

/// Kept in step with this crate's own `version` — a generated manifest entry that
/// names a version we do not publish would break `cargo package` for every
/// provider at once.
const BUILD_DEP_VERSION: &str = "0.1.0";

/// `spec`, expressed relative to `crate_dir`.
///
/// The CLI is run from the workspace root, so it holds paths like
/// `artefacts/cloud_providers/hetzner/openapi.json`. Cargo runs a build script
/// with `CARGO_MANIFEST_DIR` set to the *crate*, so the path has to be rewritten
/// or the script looks for the spec inside the crate and silently finds nothing —
/// which reads exactly like the published-crate case and skips.
fn relative_to(crate_dir: &Path, spec: &Path) -> String {
    if spec.is_absolute() {
        return spec.to_string_lossy().into_owned();
    }
    // Both are workspace-relative: climb out of the crate, then descend.
    let up = crate_dir.components().count();
    let mut out = PathBuf::new();
    for _ in 0..up {
        out.push("..");
    }
    out.push(spec);
    out.to_string_lossy().into_owned()
}
