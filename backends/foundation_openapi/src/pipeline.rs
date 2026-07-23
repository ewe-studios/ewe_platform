//! One declaration of what a crate generates, and two verbs over it.
//!
//! **WHY:** a provider crate needs six endpoints out of a spec with hundreds, and
//! it needs the same slice named in one place — not spread between a CLI
//! invocation, a build script and someone's memory. And it needs to be told when
//! the vendor's spec moves under it rather than silently regenerating different
//! types.
//!
//! **WHAT:** [`Pipeline`] — provider, spec, version pins, selection — with
//! [`Pipeline::check`] (does the committed output still match? is the pin still
//! true?) and [`Pipeline::write`] (produce it).
//!
//! **HOW:** the four stages, in the order that matters:
//!
//! ```text
//! select  ->  canonicalise  ->  prune  ->  (generate)
//! ```
//!
//! Selecting first means canonicalisation only hoists schemas for paths we keep
//! (Linode: under 40 rather than 1042), and pruning last drops whatever the
//! hoisting did not make reachable.
//!
//! **Where the verbs belong:** `check()` is for a build script or CI — it never
//! writes. `write()` is for the CLI or a deliberate developer step. These crates
//! are publishable, so a build script that wrote into `src/` would leave a
//! crates.io consumer needing an unpublished spec and writing into a read-only
//! registry directory (spec-56 decision 05 / feature 00 §5).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::selection::Selection;

/// Why a pipeline run failed.
#[derive(Debug)]
pub enum PipelineError {
    /// The spec file could not be read.
    SpecUnreadable {
        /// Path we tried.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },
    /// The spec file is not JSON.
    SpecUnparseable {
        /// Path we tried.
        path: PathBuf,
        /// The underlying parse error.
        source: serde_json::Error,
    },
    /// The spec's `info.version` is not what this crate pinned.
    ///
    /// Deliberately fatal: silently regenerating against a moved spec would change
    /// the client's types with nobody choosing to. Bump the pin, regenerate, read
    /// the diff.
    SpecVersionMismatch {
        /// What the crate pinned.
        pinned: String,
        /// What the spec on disk says.
        found: String,
    },
    /// The spec declares no `info.version` to check the pin against.
    SpecVersionMissing {
        /// What the crate pinned.
        pinned: String,
    },
    /// The transformed spec is still not canonical — a transform did not do its job.
    NotCanonical(Vec<crate::transform::NotCanonical>),
    /// Our own transforms left a `$ref` pointing at something that is not there.
    ///
    /// Only refs **we** broke — a vendor shipping a spec that references a schema
    /// it never bundles is their bug, not a reason to refuse to generate
    /// (Cloudflare ships 57 such refs). Those are reported, not fatal.
    DanglingRefs {
        /// Refs that resolved in the vendor's document and do not resolve in ours.
        introduced: Vec<String>,
    },
    /// `check` found the committed output no longer matches the declaration.
    Stale {
        /// What differs.
        detail: String,
    },
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpecUnreadable { path, source } => {
                write!(f, "cannot read spec {}: {source}", path.display())
            }
            Self::SpecUnparseable { path, source } => {
                write!(f, "spec {} is not valid JSON: {source}", path.display())
            }
            Self::SpecVersionMismatch { pinned, found } => write!(
                f,
                "spec version moved: this crate pins {pinned}, the spec says {found}. \
                 Upgrading is deliberate — bump the pin, regenerate, and read the diff."
            ),
            Self::SpecVersionMissing { pinned } => write!(
                f,
                "this crate pins spec version {pinned}, but the spec declares no info.version"
            ),
            Self::NotCanonical(issues) => {
                writeln!(f, "the spec is not canonical after transforms:")?;
                for issue in issues.iter().take(10) {
                    writeln!(f, "  - {issue}")?;
                }
                if issues.len() > 10 {
                    writeln!(f, "  … and {} more", issues.len() - 10)?;
                }
                Ok(())
            }
            Self::DanglingRefs { introduced } => write!(
                f,
                "our transforms left {} $ref(s) unresolvable — they resolved in the vendor's \
                 document and do not resolve in ours: {:?}",
                introduced.len(),
                &introduced[..introduced.len().min(5)]
            ),
            Self::Stale { detail } => write!(f, "generated output is stale: {detail}"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// What a crate generates, declared once.
#[derive(Debug, Clone)]
pub struct Pipeline {
    provider: String,
    spec_path: PathBuf,
    api_version: Option<String>,
    spec_version: Option<String>,
    base_url: Option<String>,
    selection: Selection,
}

impl Pipeline {
    /// Start a declaration for `provider`, reading `spec_path`.
    #[must_use]
    pub fn new(provider: impl Into<String>, spec_path: impl Into<PathBuf>) -> Self {
        Self {
            provider: provider.into(),
            spec_path: spec_path.into(),
            api_version: None,
            spec_version: None,
            base_url: None,
            selection: Selection::all(),
        }
    }

    /// Pin the vendor's API version (Linode `v4`, `DigitalOcean` `v2`, Hetzner `v1`).
    ///
    /// It is fixed into the client so callers never pass it — an API version is
    /// not a per-call decision. Linode needs this most: its paths carry the
    /// version as a **path parameter** (`/{apiVersion}/linode/instances`), so
    /// every generated call would otherwise take it as an argument.
    #[must_use]
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = Some(version.into());
        self
    }

    /// Pin the spec revision this crate was generated from (`info.version`).
    ///
    /// A mismatch is **fatal**, not a warning: regenerating against a document
    /// that moved would change the client's types with nobody choosing to.
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

    /// The slice to generate. Left unset, everything is generated.
    #[must_use]
    pub fn select(mut self, selection: Selection) -> Self {
        self.selection = selection;
        self
    }

    /// The provider this generates for.
    #[must_use]
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// The pinned API version, if any.
    #[must_use]
    pub fn pinned_api_version(&self) -> Option<&str> {
        self.api_version.as_deref()
    }

    /// Read the spec, verify the pin, and run select → canonicalise → prune.
    ///
    /// The result is canonical and self-contained, or this is an error — a
    /// half-transformed spec generates quietly-wrong code, which is worse than a
    /// failed build.
    ///
    /// # Errors
    /// [`PipelineError`] for an unreadable/unparseable spec, a moved spec version,
    /// output that is not canonical, or dangling refs left by pruning.
    pub fn resolve(&self) -> Result<Value, PipelineError> {
        let raw = std::fs::read_to_string(&self.spec_path).map_err(|source| {
            PipelineError::SpecUnreadable {
                path: self.spec_path.clone(),
                source,
            }
        })?;
        let mut spec: Value =
            serde_json::from_str(&raw).map_err(|source| PipelineError::SpecUnparseable {
                path: self.spec_path.clone(),
                source,
            })?;

        self.verify_spec_version(&spec)?;

        // What was already broken before we touched it. A vendor's spec can
        // reference a schema it never bundles (Cloudflare: 57), and refusing to
        // generate over their bug would mean we simply cannot support them. The
        // invariant we can hold is narrower and is the one that matters: *we* must
        // not break a ref that used to resolve.
        let inherited: BTreeSet<String> = crate::prune::dangling_refs(&spec).into_iter().collect();

        // 1. select — before anything else, so the stages below only ever touch
        //    the handful of paths we keep.
        crate::prune::prune_to_selection(&mut spec, &self.selection);

        // 2. canonicalise — the generator names types from $ref, so inline
        //    schemas must be hoisted or they generate as anonymous blobs.
        if let Some(base) = &self.base_url {
            crate::transform::ensure_servers(&mut spec, base);
        }
        // Responses first: DigitalOcean hides every response behind a $ref into
        // components/responses, which our Response model cannot express — so the
        // operation looks like it returns nothing. Rewiring these before hoisting
        // means `canonicalize_operations` sees ordinary `content` for every vendor.
        crate::transform::resolve_response_refs(&mut spec);
        // Same story for parameters: DO refs all 833 of them, and our Parameter
        // model cannot express a $ref — so `droplets_list` generated with no
        // arguments, losing `?tag_name=`, which is how a deployment enumerates its
        // own droplets.
        crate::transform::resolve_parameter_refs(&mut spec);
        crate::transform::canonicalize_operations(&mut spec);

        // 3. prune again — hoisting rewires reachability, and nothing else drops
        //    what the vendor left orphaned.
        crate::prune::prune_to_selection(&mut spec, &self.selection);

        crate::transform::validate_canonical(&spec).map_err(PipelineError::NotCanonical)?;

        let introduced: Vec<String> = crate::prune::dangling_refs(&spec)
            .into_iter()
            .filter(|r| !inherited.contains(r))
            .collect();
        if !introduced.is_empty() {
            return Err(PipelineError::DanglingRefs { introduced });
        }
        if !inherited.is_empty() {
            // Not fatal, but never silent: these generate as untyped blobs, and
            // someone should know whose fault that is.
            tracing::warn!(
                provider = %self.provider,
                count = inherited.len(),
                sample = ?inherited.iter().take(3).collect::<Vec<_>>(),
                "spec references schemas it does not define; those fields generate untyped"
            );
        }

        Ok(spec)
    }

    /// The pin, checked against the spec on disk.
    fn verify_spec_version(&self, spec: &Value) -> Result<(), PipelineError> {
        let Some(pinned) = &self.spec_version else {
            return Ok(()); // nothing pinned, nothing to betray
        };
        let found = spec
            .pointer("/info/version")
            .and_then(Value::as_str)
            .ok_or_else(|| PipelineError::SpecVersionMissing {
                pinned: pinned.clone(),
            })?;

        if found == pinned {
            Ok(())
        } else {
            Err(PipelineError::SpecVersionMismatch {
                pinned: pinned.clone(),
                found: found.to_string(),
            })
        }
    }

    /// Resolve, and compare against what is committed at `out_dir`.
    ///
    /// For a build script or CI: it **never writes**. Fails when the pin no longer
    /// matches the spec, or when the committed output is not what this declaration
    /// produces.
    ///
    /// # Errors
    /// [`PipelineError::Stale`] when the output differs, plus anything
    /// [`Pipeline::resolve`] can return.
    pub fn check(&self, out_dir: &Path) -> Result<(), PipelineError> {
        let resolved = self.resolve()?;
        let target = out_dir.join(format!("{}.canonical.json", self.provider));

        let committed = std::fs::read_to_string(&target).map_err(|_| PipelineError::Stale {
            detail: format!("{} does not exist — run the generator", target.display()),
        })?;
        let committed: Value =
            serde_json::from_str(&committed).map_err(|source| PipelineError::SpecUnparseable {
                path: target.clone(),
                source,
            })?;

        if committed == resolved {
            Ok(())
        } else {
            Err(PipelineError::Stale {
                detail: format!(
                    "{} does not match this declaration — regenerate it",
                    target.display()
                ),
            })
        }
    }

    /// Resolve and write the canonical spec into `out_dir`.
    ///
    /// For the CLI or a deliberate developer step — **not** a build script of a
    /// publishable crate (feature 00 §5).
    ///
    /// # Errors
    /// [`PipelineError`] from [`Pipeline::resolve`], or an IO failure writing.
    pub fn write(&self, out_dir: &Path) -> Result<PathBuf, PipelineError> {
        let resolved = self.resolve()?;
        let target = out_dir.join(format!("{}.canonical.json", self.provider));

        std::fs::create_dir_all(out_dir).map_err(|source| PipelineError::SpecUnreadable {
            path: out_dir.to_path_buf(),
            source,
        })?;
        let body = serde_json::to_string_pretty(&resolved).expect("a resolved spec serializes");
        std::fs::write(&target, body).map_err(|source| PipelineError::SpecUnreadable {
            path: target.clone(),
            source,
        })?;

        Ok(target)
    }
}
