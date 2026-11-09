//! Target-agnostic search over a VFS: grep and find. `InCodeVfsSearcher` walks
//! any `AsyncVfsFileSystem`. The host-process backends (`CliSearcher` and
//! `FffSearcher`) live in `native::vfs::search`, because they run external
//! programs and are not wasm-compatible.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::async_traits::{AsyncVfsDirectory, AsyncVfsFileSystem};
use super::error::{VfsError, VfsResult};
use super::types::VfsFileType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VfsSearchKind {
    Grep,
    Find,
    MultiGrep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsSearchMatch {
    pub path: String,
    pub line_number: u32,
    pub content: String,
    pub score: f32,
}

#[async_trait]
pub trait VfsSearcher: Send + Sync {
    fn is_available(&self) -> bool {
        true
    }

    /// # Errors
    /// Returns `VfsError::Backend` for invalid regex or I/O failures.
    async fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>>;
}

#[async_trait]
impl VfsSearcher for Box<dyn VfsSearcher> {
    fn is_available(&self) -> bool {
        (**self).is_available()
    }

    async fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        (**self).search(query, kind, roots).await
    }
}

pub(crate) const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "__pycache__",
];

// ---------------------------------------------------------------------------
// InCodeVfsSearcher — walks AsyncVfsFileSystem directories with regex matching
// ---------------------------------------------------------------------------

pub struct InCodeVfsSearcher<F: AsyncVfsFileSystem> {
    fs: Arc<F>,
}

impl<F: AsyncVfsFileSystem> InCodeVfsSearcher<F> {
    #[must_use]
    pub fn new(fs: Arc<F>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl<F: AsyncVfsFileSystem + 'static> VfsSearcher for InCodeVfsSearcher<F> {
    async fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        let re = regex::Regex::new(query).map_err(|e| VfsError::Backend {
            message: format!("invalid regex: {e}"),
        })?;

        let mut matches = Vec::new();
        for root in roots {
            walk(&*self.fs, root.clone(), &re, kind, &mut matches).await;
        }

        if matches!(kind, VfsSearchKind::Grep | VfsSearchKind::MultiGrep) {
            matches.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(matches)
    }
}

type WalkFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Walks `dir` depth first. `Grep` matches file lines, `Find` matches entry
/// names. A directory or file that cannot be read is skipped.
fn walk<'a, F: AsyncVfsFileSystem>(
    fs: &'a F,
    dir: String,
    re: &'a regex::Regex,
    kind: VfsSearchKind,
    matches: &'a mut Vec<VfsSearchMatch>,
) -> WalkFuture<'a> {
    Box::pin(async move {
        let Ok(d) = fs.open_directory_async(dir.clone()).await else {
            return;
        };
        let Ok(entries) = d.list_async().await else {
            return;
        };
        for entry in entries {
            let child = if dir == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", dir, entry.name)
            };
            let is_dir = entry.file_type == VfsFileType::Directory;
            if kind == VfsSearchKind::Find && re.is_match(&entry.name) {
                matches.push(VfsSearchMatch {
                    path: child.clone(),
                    line_number: 0,
                    content: String::new(),
                    score: 1.0,
                });
            }
            if is_dir {
                if EXCLUDED_DIRS.contains(&entry.name.as_str()) {
                    continue;
                }
                walk(fs, child, re, kind, matches).await;
            } else if kind != VfsSearchKind::Find {
                grep_file(fs, child, re, matches).await;
            }
        }
    })
}

async fn grep_file<F: AsyncVfsFileSystem>(
    fs: &F,
    path: String,
    re: &regex::Regex,
    matches: &mut Vec<VfsSearchMatch>,
) {
    let Ok(data) = fs.read_file_async(path.clone()).await else {
        return;
    };
    let Ok(content) = String::from_utf8(data) else {
        return;
    };
    for (i, line) in content.lines().enumerate() {
        if re.is_match(line) {
            #[allow(clippy::cast_possible_truncation)]
            matches.push(VfsSearchMatch {
                path: path.clone(),
                line_number: (i + 1) as u32,
                content: line.to_string(),
                score: 1.0,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// CascadingVfsSearcher — tries backends in order
// ---------------------------------------------------------------------------

pub struct CascadingVfsSearcher {
    backends: Vec<Box<dyn VfsSearcher>>,
}

impl CascadingVfsSearcher {
    #[must_use]
    pub fn new(backends: Vec<Box<dyn VfsSearcher>>) -> Self {
        Self { backends }
    }
}

#[async_trait]
impl VfsSearcher for CascadingVfsSearcher {
    fn is_available(&self) -> bool {
        self.backends.iter().any(VfsSearcher::is_available)
    }

    async fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        for backend in &self.backends {
            if backend.is_available() {
                return backend.search(query, kind, roots).await;
            }
        }
        Err(VfsError::Unsupported {
            operation: "no search backend available".into(),
        }
        .into())
    }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

#[must_use]
pub fn vfs_searcher<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Box<dyn VfsSearcher> {
    Box::new(InCodeVfsSearcher::new(fs))
}
