//! Build contexts for `POST /build`.
//!
//! **WHY:** The daemon does not read a Dockerfile off the caller's disk — it has
//! no access to it. The whole build context (the Dockerfile plus every file a
//! `COPY`/`ADD` might reference) is uploaded as a **tar stream** in the request
//! body. Without one the endpoint has nothing to build.
//!
//! **WHAT:** [`ContextTar`] builds that stream: from a directory on disk, and/or
//! with an inline Dockerfile written in at a chosen name.
//!
//! **HOW:** `tar` crate, uncompressed (the daemon sniffs gzip/bzip2/xz too, but
//! plain tar keeps the local path cheap). Paths are stored relative to the
//! context root, which is what `COPY src dst` resolves against.

use std::io;
use std::path::Path;

/// A build context, ready to be sent as the body of `POST /build`.
#[derive(Debug, Clone, Default)]
pub struct ContextTar {
    bytes: Vec<u8>,
}

impl ContextTar {
    /// Tar up `dir` recursively, storing paths relative to it.
    ///
    /// # Errors
    /// Returns any IO error from walking or reading the directory.
    pub fn from_dir(dir: impl AsRef<Path>) -> io::Result<Self> {
        let mut builder = tar::Builder::new(Vec::new());
        builder.follow_symlinks(false);
        // "." keeps every entry relative to the context root.
        builder.append_dir_all(".", dir.as_ref())?;
        let bytes = builder.into_inner()?;
        Ok(Self { bytes })
    }

    /// A context whose only entry is `dockerfile_name`, holding `content`.
    ///
    /// For a Dockerfile that copies nothing from disk, this is the whole context
    /// — no temp directory needed.
    ///
    /// # Errors
    /// Returns any IO error from writing the in-memory archive.
    pub fn from_inline_dockerfile(dockerfile_name: &str, content: &str) -> io::Result<Self> {
        let mut builder = tar::Builder::new(Vec::new());
        append_bytes(&mut builder, dockerfile_name, content.as_bytes())?;
        let bytes = builder.into_inner()?;
        Ok(Self { bytes })
    }

    /// Tar up `dir`, then add (or replace) `dockerfile_name` with `content`.
    ///
    /// This is how an inline Dockerfile is combined with a real context: the
    /// Dockerfile never has to exist on disk, but `COPY` still sees `dir`.
    ///
    /// # Errors
    /// Returns any IO error from walking, reading, or writing the archive.
    pub fn from_dir_with_dockerfile(
        dir: impl AsRef<Path>,
        dockerfile_name: &str,
        content: &str,
    ) -> io::Result<Self> {
        let mut builder = tar::Builder::new(Vec::new());
        builder.follow_symlinks(false);
        builder.append_dir_all(".", dir.as_ref())?;
        // Appended last: tar allows duplicate names and the daemon takes the
        // final entry, so this overrides any Dockerfile already in `dir`.
        append_bytes(&mut builder, dockerfile_name, content.as_bytes())?;
        let bytes = builder.into_inner()?;
        Ok(Self { bytes })
    }

    /// The tar stream.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume this context, yielding the tar stream.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Size of the tar stream in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the tar stream is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Append `data` under `name` with a regular-file header.
fn append_bytes<W: io::Write>(
    builder: &mut tar::Builder<W>,
    name: &str,
    data: &[u8],
) -> io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, data)
}
