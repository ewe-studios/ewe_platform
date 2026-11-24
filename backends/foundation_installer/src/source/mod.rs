//! Sources: the trait that gets software and the artifact it gives back.
//!
//! One adapter for each [`SourceSpec`](crate::manifest::SourceSpec) variant.
//! A downloaded file streams to a temporary file and is renamed into place
//! only after its SHA-256 matches (root rule 4).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use foundation_netio::PreparedRequestBuilder;
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{hex_lower, Entry};

pub mod apt;
pub mod cargo;
pub mod docker_image;
pub mod github_release;
pub mod mise;
pub mod url;

/// What a source adapter put on the host. Made by
/// [`SourceAdapter::resolve`]; read by
/// [`DeployTargetAdapter::apply`](crate::deploy::DeployTargetAdapter::apply)
/// and written to the install record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Artifact {
    /// A file that the installer downloaded and checked. The installer owns it.
    File {
        /// Example: `/var/lib/ewe/artifacts/agent-rpc/0.4.0/agent-rpc`.
        path: PathBuf,
        /// The SHA-256 that the installer checked.
        sha256: String,
    },
    /// A program that a package manager (cargo, apt, mise) installed into its
    /// own location. The package manager owns the file, not the installer.
    Tool {
        /// The program name. Example: `rg`.
        program: String,
        /// Where the package manager put it. Example: `/root/.cargo/bin/rg`.
        path: PathBuf,
    },
    /// A container image in the local Docker image store.
    Image(ImageRef),
}

impl Artifact {
    /// The class that the compatibility table uses: `file`, `tool` or `image`.
    #[must_use]
    pub fn class(&self) -> &'static str {
        match self {
            Self::File { .. } => "file",
            Self::Tool { .. } => "tool",
            Self::Image(_) => "image",
        }
    }

    /// The path of a file or tool artifact.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::File { path, .. } | Self::Tool { path, .. } => Some(path),
            Self::Image(_) => None,
        }
    }

    /// True when the artifact is still where the adapter put it.
    #[must_use]
    pub fn is_present(&self) -> bool {
        match self {
            Self::File { path, .. } | Self::Tool { path, .. } => path.exists(),
            // An image can only be checked with the Docker API.
            Self::Image(_) => true,
        }
    }
}

/// The name of one container image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    /// Example: `ghcr.io/example/vector-db`.
    pub repository: String,
    /// Example: `1.9.0`.
    pub tag: String,
    /// The digest that Docker reports after the pull, for example
    /// `sha256:4f3c...`.
    pub digest: String,
}

impl ImageRef {
    /// The `repository:tag` text that Docker takes.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
    }
}

/// Gets the software of one entry and returns what is now on the host.
///
/// One implementation for each [`SourceSpec`](crate::manifest::SourceSpec)
/// variant. `async_trait` makes the trait object-safe, so the installer can
/// hold `Arc<dyn SourceAdapter>`.
#[async_trait]
pub trait SourceAdapter: Send + Sync {
    /// The `kind` value that this adapter handles, for example
    /// `github-release`.
    fn kind(&self) -> &'static str;

    /// Get the software. A download streams to a temporary file in
    /// `InstallerConfig::artifact_dir` and is renamed into place only after
    /// the checksum matches.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Resolve`] when the software cannot be got and
    /// [`InstallError::ChecksumMismatch`] when the bytes are wrong.
    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError>;
}

/// Full the bytes at `url` into the artifact directory and check their
/// SHA-256.
///
/// The body is a stream: each chunk goes to disk and into the hash, so the
/// whole artifact never sits in memory. The file is renamed into place only
/// after the checksum matches.
pub(crate) async fn download_checked(
    entry: &Entry,
    ctx: &InstallContext,
    url: &str,
    expected: &str,
) -> Result<Artifact, InstallError> {
    let client = ctx.http()?.clone();
    let request = PreparedRequestBuilder::get(url).map_err(|err| InstallError::Resolve {
        entry: entry.name.to_string(),
        reason: format!("cannot build a request for {url}: {err}"),
    })?;
    let response =
        client
            .send_async(request.build())
            .await
            .map_err(|err| InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("cannot download {url}: {err}"),
            })?;

    let status = response.get_status().into_usize();
    if !(200..300).contains(&status) {
        return Err(InstallError::Resolve {
            entry: entry.name.to_string(),
            reason: format!("{url} answered with HTTP {status}"),
        });
    }

    let directory = ctx
        .config()
        .artifact_dir
        .join(entry.name.as_str())
        .join(&entry.version);
    std::fs::create_dir_all(&directory).map_err(|err| InstallError::Resolve {
        entry: entry.name.to_string(),
        reason: format!("cannot make {}: {err}", directory.display()),
    })?;
    let final_path = directory.join(entry.name.as_str());
    let temporary_path = directory.join(format!("{}.part", entry.name.as_str()));

    let mut file = std::fs::File::create(&temporary_path).map_err(|err| InstallError::Resolve {
        entry: entry.name.to_string(),
        reason: format!("cannot create {}: {err}", temporary_path.display()),
    })?;
    let mut hasher = Sha256::new();
    let mut body = response.take_async_body();
    while let Some(chunk) = body.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => {
                drop(file);
                remove_quietly(&temporary_path);
                return Err(InstallError::Resolve {
                    entry: entry.name.to_string(),
                    reason: format!("cannot read the body of {url}: {err}"),
                });
            }
        };
        hasher.update(&chunk);
        // A file write is a boundary call. The chunk is small and the write
        // never holds the whole artifact.
        if let Err(err) = std::io::Write::write_all(&mut file, &chunk) {
            drop(file);
            remove_quietly(&temporary_path);
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("cannot write {}: {err}", temporary_path.display()),
            });
        }
    }
    if let Err(err) = file.flush() {
        drop(file);
        remove_quietly(&temporary_path);
        return Err(InstallError::Resolve {
            entry: entry.name.to_string(),
            reason: format!("cannot flush {}: {err}", temporary_path.display()),
        });
    }
    drop(file);

    let actual = hex_lower(&hasher.finalize());
    let expected = expected.to_ascii_lowercase();
    if actual != expected {
        remove_quietly(&temporary_path);
        return Err(InstallError::ChecksumMismatch {
            entry: entry.name.to_string(),
            expected,
            actual,
        });
    }

    std::fs::rename(&temporary_path, &final_path).map_err(|err| {
        remove_quietly(&temporary_path);
        InstallError::Resolve {
            entry: entry.name.to_string(),
            reason: format!("cannot rename {}: {err}", temporary_path.display()),
        }
    })?;

    Ok(Artifact::File {
        path: final_path,
        sha256: expected,
    })
}

fn remove_quietly(path: &Path) {
    if let Err(err) = std::fs::remove_file(path) {
        tracing::debug!(
            error = %err,
            path = %path.display(),
            "cannot remove a temporary artifact"
        );
    }
}

/// Percent-encode one URL path segment. It keeps only the unreserved
/// characters of RFC 3986.
#[must_use]
pub fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            use core::fmt::Write as _;
            // Writing to a String cannot fail.
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}
