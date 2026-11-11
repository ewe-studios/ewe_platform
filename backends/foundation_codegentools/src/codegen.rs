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

/// Load a [`Selection`] from a JSON file — the machine-readable twin of the
/// declaration a `build.rs` carries (spec-56 feature 00 §4).
///
/// A split-out provider crate commits one as `codegen.json`, so
/// `genapi generate <provider>` honours the same slice its build script checks,
/// without the `--include-*` flags having to be retyped from the crate's doc
/// comment (`generate hetzner` used to ignore the crate's six declared
/// operations and regenerate all 151 endpoints).
///
/// The file is a JSON object with optional string arrays `paths`, `tags` and
/// `operations`. An omitted or empty array contributes nothing — an empty object
/// means "generate everything", exactly like an absent selection.
///
/// # Errors
/// [`CodegenError::Io`] when the file is unreadable, or not a JSON object whose
/// arrays hold strings.
pub fn selection_from_file(path: impl AsRef<Path>) -> Result<Selection, CodegenError> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|source| CodegenError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| CodegenError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{e}")),
    })?;

    let mut selection = Selection::all();
    if let Some(items) = value.get("paths").and_then(serde_json::Value::as_array) {
        selection = selection.paths(items.iter().filter_map(serde_json::Value::as_str));
    }
    if let Some(items) = value.get("tags").and_then(serde_json::Value::as_array) {
        selection = selection.tags(items.iter().filter_map(serde_json::Value::as_str));
    }
    if let Some(items) = value
        .get("operations")
        .and_then(serde_json::Value::as_array)
    {
        selection = selection.operations(items.iter().filter_map(serde_json::Value::as_str));
    }
    Ok(selection)
}

/// Where the vendored Cloudflare spec patches live, relative to the workspace
/// root (`overlays_in` reads every `*.json` there, in name order).
pub const CLOUDFLARE_OVERLAYS_DIR: &str = "artefacts/cloud_providers/cloudflare/overlays";

/// Cloudflare schemas that land in `foundation_cloudflare_types` even though one
/// group alone uses them: the D1 `/query` request and response, and the R2
/// object listing (added by `overlays/r2-list-objects.json`). `foundation_db`
/// depends on that leaf crate, not on a group crate.
pub const CLOUDFLARE_HOISTED_SCHEMAS: &[&str] = &[
    "d1_batch_query",
    "d1_query_database_response",
    "d1_raw_result_response",
    "r2_list_objects_response",
];

/// Cloudflare `oneOf` schemas generated as untagged enums: D1's one-query-or-
/// a-batch request body.
pub const CLOUDFLARE_TYPED_UNIONS: &[&str] = &["d1_batch_query"];

/// Apply the Cloudflare wire-type settings to `codegen`: the overlays, the
/// hoisted schemas and the typed unions above. One function, so the CLI and the
/// crate's `build.rs` declare the same thing. `workspace_root` is the directory
/// that holds `artefacts/`.
///
/// # Errors
/// [`CodegenError::Io`] when the overlay directory cannot be listed.
pub fn cloudflare_wire_types(
    codegen: Codegen,
    workspace_root: &Path,
) -> Result<Codegen, CodegenError> {
    Ok(codegen
        .overlays_in(workspace_root.join(CLOUDFLARE_OVERLAYS_DIR))?
        .hoist_to_shared(CLOUDFLARE_HOISTED_SCHEMAS.iter().copied())
        .typed_unions(CLOUDFLARE_TYPED_UNIONS.iter().copied()))
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
    /// Split mode (feature 22 E3): one crate per generated group, created
    /// under this directory (a workspace `backends/` root).
    groups_dir: Option<PathBuf>,
    /// Split mode: the crate that receives the cross-group shared types.
    shared_crate_dir: Option<PathBuf>,
    options: AnalysisOptions,
    /// Vendored spec patches merged before generation, in order.
    overlays: Vec<PathBuf>,
    /// Schemas whose `oneOf` is emitted as an untagged enum.
    typed_unions: Vec<String>,
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
            groups_dir: None,
            shared_crate_dir: None,
            // The thresholds `genapi generate` has always defaulted to; a
            // declaration that says nothing about grouping gets what the CLI gives.
            options: AnalysisOptions {
                min_group_size: 10,
                max_group_size: 200,
                hoist_to_shared: Vec::new(),
            },
            overlays: Vec::new(),
            typed_unions: Vec::new(),
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
        self.options.min_group_size = min;
        self.options.max_group_size = max;
        self
    }

    /// Merge a vendored overlay file into the spec before generation.
    ///
    /// Overlays apply in the order added. See `foundation_openapi::overlay` for
    /// the file format: it must say what it adds and where that came from.
    #[must_use]
    pub fn overlay(mut self, path: impl Into<PathBuf>) -> Self {
        self.overlays.push(path.into());
        self
    }

    /// Merge every `*.json` overlay in `dir`, in file-name order. A missing
    /// directory adds nothing.
    ///
    /// # Errors
    /// [`CodegenError::Io`] when `dir` exists and cannot be listed.
    pub fn overlays_in(mut self, dir: impl AsRef<Path>) -> Result<Self, CodegenError> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Ok(self);
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|source| CodegenError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        self.overlays.extend(files);
        Ok(self)
    }

    /// Emit these spec schemas (`d1_batch_query`) and everything they reference
    /// into the shared types module, even when one group alone uses them.
    #[must_use]
    pub fn hoist_to_shared<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.options
            .hoist_to_shared
            .extend(names.into_iter().map(Into::into));
        self
    }

    /// Emit the `oneOf` of these spec schemas as untagged enums instead of
    /// `serde_json::Value`.
    #[must_use]
    pub fn typed_unions<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.typed_unions.extend(names.into_iter().map(Into::into));
        self
    }

    /// Split the generated groups into one crate per group (feature 22 E3).
    ///
    /// `groups_dir` names the workspace root the group crates are created under
    /// (one `foundation_<provider>_<group>/` per group); `shared_crate_dir` names
    /// the leaf crate that receives the cross-group shared types. The provider
    /// crate keeps only a feature-gated re-export shim.
    #[must_use]
    pub fn split_groups(
        mut self,
        groups_dir: impl Into<PathBuf>,
        shared_crate_dir: impl Into<PathBuf>,
    ) -> Self {
        self.groups_dir = Some(groups_dir.into());
        self.shared_crate_dir = Some(shared_crate_dir.into());
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
        let provider = self.provider.as_deref().ok_or(CodegenError::Incomplete {
            missing: "provider",
        })?;
        let spec = self
            .spec
            .as_ref()
            .ok_or(CodegenError::Incomplete { missing: "spec" })?;

        let mut pipeline = Pipeline::new(provider, spec).select(self.selection.clone());
        for overlay in &self.overlays {
            pipeline = pipeline.overlay(overlay);
        }
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
        let output = self.output_dir.clone().ok_or(CodegenError::Incomplete {
            missing: "crate_dir",
        })?;
        Ok((output, self.provider_dir.clone()))
    }

    #[allow(clippy::ref_option)] // Takes the optional field by reference, as the callers hold it.
    #[allow(clippy::needless_pass_by_value)] // Signature kept; the output path is owned by the caller's clone.
    fn provider_root(
        &self,
        output: PathBuf,
        provider_dir: &Option<PathBuf>,
    ) -> Result<PathBuf, CodegenError> {
        Ok(if let Some(dir) = provider_dir {
            dir.clone()
        } else {
            let provider = self.provider.as_deref().ok_or(CodegenError::Incomplete {
                missing: "provider",
            })?;
            output.join(provider)
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
        self.run_generator(
            &resolved,
            &GeneratorTargets {
                output: output.clone(),
                provider_dir: provider_dir.clone(),
                groups_dir: self.groups_dir.clone(),
                shared_crate_dir: self.shared_crate_dir.clone(),
                sibling_crates: self.groups_dir.clone(),
                shared_only: false,
            },
        )?;

        let generated_dir = self.provider_root(output, &provider_dir)?.join("generated");
        let files = if let (Some(groups_dir), Some(_)) = (&self.groups_dir, &self.shared_crate_dir)
        {
            self.count_split_files(groups_dir, &generated_dir)?
        } else {
            read_tree(&generated_dir)?.len()
        };
        Ok(CodegenReport {
            paths_selected: resolved
                .get("paths")
                .and_then(serde_json::Value::as_object)
                .map_or(0, serde_json::Map::len),
            schemas: foundation_openapi::component_count(&resolved, "schemas"),
            files,
            generated_dir,
        })
    }

    /// How many files split mode wrote: the umbrella shim, the shared crate's
    /// single `generated.rs`, and every generated group crate under
    /// `groups_dir`. Only crates whose name carries this provider's prefix are
    /// counted, so hand-written neighbours in a shared `backends/` root do not
    /// inflate the report.
    fn count_split_files(
        &self,
        groups_dir: &Path,
        generated_dir: &Path,
    ) -> Result<usize, CodegenError> {
        let provider = self.provider.as_deref().ok_or(CodegenError::Incomplete {
            missing: "provider",
        })?;
        let provider_safe = provider.replace(['-', '/'], "_");
        let prefix = format!("foundation_{provider_safe}_");
        let mut count = read_tree(generated_dir)?.len() + 1; // shim + shared/generated.rs
        let entries = std::fs::read_dir(groups_dir).map_err(|source| CodegenError::Io {
            path: groups_dir.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| CodegenError::Io {
                path: groups_dir.to_path_buf(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && entry.path().is_dir() {
                count += read_tree(&entry.path())?.len();
            }
        }
        Ok(count)
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
    #[allow(clippy::too_many_lines)] // Long by design: one flow per subcommand; splitting it is a refactor, not a lint fix.
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

        let mut fresh = BTreeMap::new();
        let mut committed = BTreeMap::new();

        if let (Some(groups_dir), Some(shared_crate_dir)) =
            (&self.groups_dir, &self.shared_crate_dir)
        {
            // Split mode writes three places: the umbrella shim, the shared types
            // crate, and one crate per group. Redirect all three into the scratch
            // tree, then compare each against its committed counterpart. The
            // shared types crate also holds hand-written `lib.rs`/`Cargo.toml`,
            // so only its single generated file is compared.
            let scratch_groups = scratch.path().join("groups");
            let scratch_shared = scratch.path().join("shared_crate");
            self.run_generator(
                &resolved,
                &GeneratorTargets {
                    output: scratch.path().to_path_buf(),
                    provider_dir: scratch_provider_dir,
                    groups_dir: Some(scratch_groups.clone()),
                    shared_crate_dir: Some(scratch_shared.clone()),
                    // The scratch tree has no sibling crates in it, so the group
                    // manifests read their versions from the shipped workspace.
                    sibling_crates: self.groups_dir.clone(),
                    shared_only: false,
                },
            )?;

            merge_prefixed(
                "shim",
                &read_tree(&scratch.path().join("src/generated"))?,
                &mut fresh,
            );
            merge_prefixed("shim", &read_tree(&generated_dir)?, &mut committed);

            merge_prefixed(
                "shared",
                &read_file_tree(&scratch_shared.join("src/generated.rs"), "src/generated.rs")?,
                &mut fresh,
            );
            merge_prefixed(
                "shared",
                &read_file_tree(
                    &shared_crate_dir.join("src/generated.rs"),
                    "src/generated.rs",
                )?,
                &mut committed,
            );

            // Group crates: the fresh tree under the scratch root is exactly the
            // set this declaration produces. The committed side is read from the
            // real `groups_dir`, but only for crates carrying this provider's
            // prefix — a shared `backends/` root holds many other crates.
            let provider = self.provider.as_deref().ok_or(CodegenError::Incomplete {
                missing: "provider",
            })?;
            let provider_safe = provider.replace(['-', '/'], "_");
            let prefix = format!("foundation_{provider_safe}_");

            for (rel, body) in read_tree(&scratch_groups)? {
                fresh.insert(format!("groups/{rel}"), body);
            }
            let entries = std::fs::read_dir(groups_dir).map_err(|source| CodegenError::Io {
                path: groups_dir.clone(),
                source,
            })?;
            for entry in entries {
                let entry = entry.map_err(|source| CodegenError::Io {
                    path: groups_dir.clone(),
                    source,
                })?;
                let name = entry.file_name().to_string_lossy().into_owned();
                // The shared types crate also carries the `foundation_<provider>_`
                // prefix but is not a group crate; it is compared separately above.
                if name.starts_with(&prefix)
                    && entry.path().as_path() != shared_crate_dir.as_path()
                    && entry.path().is_dir()
                {
                    for (sub, body) in read_tree(&entry.path())? {
                        committed.insert(format!("groups/{name}/{sub}"), body);
                    }
                }
            }
        } else {
            self.run_generator(
                &resolved,
                &GeneratorTargets {
                    output: scratch.path().to_path_buf(),
                    provider_dir: scratch_provider_dir.clone(),
                    groups_dir: None,
                    shared_crate_dir: None,
                    sibling_crates: None,
                    shared_only: false,
                },
            )?;
            let fresh_dir = self
                .provider_root(scratch.path().to_path_buf(), &scratch_provider_dir)?
                .join("generated");
            fresh = read_tree(&fresh_dir)?;
            committed = read_tree(&generated_dir)?;
        }

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

    /// Like [`Codegen::check`], but compares only the shared types crate's
    /// `generated.rs`.
    ///
    /// The shared module holds every hoisted schema, which is the part of the
    /// Cloudflare wire types that the D1 and R2 code depends on, so the
    /// wire-type test uses this. The hoisting depends on every group, so every
    /// group is still analysed and built. Only the rendering, writing and
    /// formatting of the group crates and the umbrella shim are skipped. The
    /// build script keeps the full [`Codegen::check`].
    ///
    /// # Errors
    /// [`CodegenError::Incomplete`] when the declaration is not in split mode
    /// (see [`Codegen::split_groups`]); [`CodegenError::Stale`] when the shared
    /// module differs; otherwise anything [`Codegen::check`] can return.
    pub fn check_shared_types(&self) -> Result<(), CodegenError> {
        const REL: &str = "src/generated.rs";
        let (Some(_), Some(shared_crate_dir)) = (&self.groups_dir, &self.shared_crate_dir) else {
            return Err(CodegenError::Incomplete {
                missing: "split_groups",
            });
        };
        let resolved = self.resolve()?;
        let scratch = tempfile::tempdir().map_err(|source| CodegenError::Io {
            path: PathBuf::from("<temp>"),
            source,
        })?;
        let scratch_shared = scratch.path().join("shared_crate");
        self.run_generator(
            &resolved,
            &GeneratorTargets {
                output: scratch.path().to_path_buf(),
                provider_dir: Some(scratch.path().join("src")),
                groups_dir: Some(scratch.path().join("groups")),
                shared_crate_dir: Some(scratch_shared.clone()),
                sibling_crates: self.groups_dir.clone(),
                shared_only: true,
            },
        )?;

        let fresh = read_file_tree(&scratch_shared.join(REL), REL)?;
        let committed = read_file_tree(&shared_crate_dir.join(REL), REL)?;
        match (fresh.get(REL), committed.get(REL)) {
            (Some(f), Some(c)) if f == c => Ok(()),
            (Some(_), None) => Err(CodegenError::Stale {
                generated_dir: shared_crate_dir.join("src"),
                added: vec![REL.to_string()],
                removed: Vec::new(),
                changed: Vec::new(),
            }),
            (None, Some(_)) => Err(CodegenError::Stale {
                generated_dir: shared_crate_dir.join("src"),
                added: Vec::new(),
                removed: vec![REL.to_string()],
                changed: Vec::new(),
            }),
            (Some(_), Some(_)) => Err(CodegenError::Stale {
                generated_dir: shared_crate_dir.join("src"),
                added: Vec::new(),
                removed: Vec::new(),
                changed: vec![REL.to_string()],
            }),
            (None, None) => Ok(()),
        }
    }

    /// Hand the **resolved** spec to the generator — never the vendor's document.
    fn run_generator(
        &self,
        resolved: &serde_json::Value,
        targets: &GeneratorTargets,
    ) -> Result<(), CodegenError> {
        let provider = self.provider.as_deref().ok_or(CodegenError::Incomplete {
            missing: "provider",
        })?;

        let mut generator = UnifiedGenerator::new(targets.output.clone())
            .with_typed_unions(self.typed_unions.iter());
        if let Some(dir) = &targets.provider_dir {
            generator = generator.with_provider_dir(dir.clone());
        }
        if let (Some(groups_dir), Some(shared_crate_dir)) =
            (&targets.groups_dir, &targets.shared_crate_dir)
        {
            generator = generator
                .split_group_crates(groups_dir.clone(), shared_crate_dir.clone())
                .with_sibling_crates(
                    targets
                        .sibling_crates
                        .clone()
                        .unwrap_or_else(|| groups_dir.clone()),
                );
        }
        if targets.shared_only {
            generator = generator.shared_types_only();
        }
        let body = serde_json::to_string(resolved).expect("a resolved spec serializes");
        generator
            .generate(provider, &body, &self.options)
            .map_err(CodegenError::Generate)
    }
}

/// Where one [`Codegen::run_generator`] call writes, and where the group
/// manifests read their sibling versions from.
struct GeneratorTargets {
    /// The crate root the umbrella shim is rendered into.
    output: PathBuf,
    /// The umbrella crate's provider directory, when it overrides the default.
    provider_dir: Option<PathBuf>,
    /// The workspace root the group crates are written under.
    groups_dir: Option<PathBuf>,
    /// The crate that receives the shared types module.
    shared_crate_dir: Option<PathBuf>,
    /// The workspace root that holds the group crates' sibling crates. The
    /// shipped root, even when `groups_dir` is a scratch tree: a scratch tree
    /// has no siblings in it, and the manifests need their versions.
    sibling_crates: Option<PathBuf>,
    /// Render the shared types module only.
    shared_only: bool,
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

/// Read a single file into a one-entry tree keyed by `rel`. A missing file
/// reads as empty — the same "not generated yet" convention as [`read_tree`].
fn read_file_tree(path: &Path, rel: &str) -> Result<BTreeMap<String, String>, CodegenError> {
    let mut tree = BTreeMap::new();
    if path.exists() {
        let body = std::fs::read_to_string(path).map_err(|source| CodegenError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        tree.insert(rel.to_string(), body);
    }
    Ok(tree)
}

/// Copy every `(rel, body)` of `tree` into `out` under the `prefix/` namespace,
/// so several generated trees can share one comparison without their relative
/// paths colliding.
fn merge_prefixed(
    prefix: &str,
    tree: &BTreeMap<String, String>,
    out: &mut BTreeMap<String, String>,
) {
    for (rel, body) in tree {
        out.insert(format!("{prefix}/{rel}"), body.clone());
    }
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
        let crate_dir = self.crate_dir.clone().ok_or(CodegenError::Incomplete {
            missing: "crate_dir",
        })?;
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
    #[allow(clippy::too_many_lines)] // Long by design: one flow per subcommand; splitting it is a refactor, not a lint fix.
    fn render_build_script(&self, crate_dir: &Path) -> Result<String, CodegenError> {
        use std::fmt::Write as _;

        let provider = self.provider.as_deref().ok_or(CodegenError::Incomplete {
            missing: "provider",
        })?;
        let spec = self
            .spec
            .as_ref()
            .ok_or(CodegenError::Incomplete { missing: "spec" })?;
        let spec_from_crate = relative_to(crate_dir, spec);

        let mut out = String::new();
        writeln!(
            out,
            "//! Keeps `src/generated/` honest against the vendor's spec."
        )
        .unwrap();
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
        writeln!(
            out,
            "//! **WHAT:** verifies the pins still hold, and regenerates when the"
        )
        .unwrap();
        writeln!(
            out,
            "//! committed output no longer matches this declaration."
        )
        .unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(
            out,
            "//! **HOW:** the three outcomes are deliberately different:"
        )
        .unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(out, "//! | `check()` says | meaning | what happens |").unwrap();
        writeln!(out, "//! |---|---|---|").unwrap();
        writeln!(out, "//! | `Ok` | output is current | nothing |").unwrap();
        writeln!(
            out,
            "//! | `Stale` | the declaration moved | **regenerate** |"
        )
        .unwrap();
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
            "    let spec = Path::new(&manifest).join({spec_from_crate:?});"
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
        writeln!(
            out,
            "    // Nothing to do, and failing here would break their build."
        )
        .unwrap();
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
        writeln!(
            out,
            "    println!(\"cargo:rerun-if-changed={{}}\", spec.display());"
        )
        .unwrap();
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
        if let (Some(groups_dir), Some(shared_crate_dir)) =
            (&self.groups_dir, &self.shared_crate_dir)
        {
            let groups_from_crate = relative_to(crate_dir, groups_dir);
            let shared_from_crate = relative_to(crate_dir, shared_crate_dir);
            writeln!(
                out,
                "        .split_groups(Path::new(&manifest).join({groups_from_crate:?}), Path::new(&manifest).join({shared_from_crate:?}))"
            )
            .unwrap();
        }
        writeln!(out, "        .crate_dir(&manifest);").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "    match codegen.check() {{").unwrap();
        writeln!(out, "        // Already what this declaration produces.").unwrap();
        writeln!(out, "        Ok(()) => {{}}").unwrap();
        writeln!(
            out,
            "        // The selection or the spec's shape moved — regenerate."
        )
        .unwrap();
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
const BUILD_DEP_VERSION: &str = "0.1.3";

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
