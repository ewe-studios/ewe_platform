use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::error::{VfsError, VfsResult};
use super::traits::{VfsDirectory, VfsFileSystem};
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

pub trait VfsSearcher: Send + Sync {
    fn is_available(&self) -> bool {
        true
    }

    /// # Errors
    /// Returns `VfsError::Backend` for invalid regex or I/O failures.
    fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>>;
}

impl VfsSearcher for Box<dyn VfsSearcher> {
    fn is_available(&self) -> bool {
        (**self).is_available()
    }

    fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        (**self).search(query, kind, roots)
    }
}

const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "__pycache__",
];

// ---------------------------------------------------------------------------
// InCodeVfsSearcher — walks VfsFileSystem directories with regex matching
// ---------------------------------------------------------------------------

pub struct InCodeVfsSearcher<F: VfsFileSystem> {
    fs: Arc<F>,
}

impl<F: VfsFileSystem> InCodeVfsSearcher<F> {
    #[must_use]
    pub fn new(fs: Arc<F>) -> Self {
        Self { fs }
    }
}

impl<F: VfsFileSystem + 'static> VfsSearcher for InCodeVfsSearcher<F> {
    fn search(
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
            match kind {
                VfsSearchKind::Grep | VfsSearchKind::MultiGrep => {
                    walk_grep(&*self.fs, root, &re, &mut matches);
                }
                VfsSearchKind::Find => {
                    walk_find(&*self.fs, root, &re, &mut matches);
                }
            }
        }

        if matches!(kind, VfsSearchKind::Grep | VfsSearchKind::MultiGrep) {
            matches
                .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }

        Ok(matches)
    }
}

fn walk_grep<F: VfsFileSystem>(
    fs: &F,
    dir: &str,
    re: &regex::Regex,
    matches: &mut Vec<VfsSearchMatch>,
) {
    let Ok(d) = fs.open_directory(dir) else {
        return;
    };
    let Ok(entries) = d.list() else {
        return;
    };
    for entry in entries {
        let child = if dir == "/" {
            format!("/{}", entry.name)
        } else {
            format!("{}/{}", dir, entry.name)
        };
        if entry.file_type == VfsFileType::Directory {
            if EXCLUDED_DIRS.contains(&entry.name.as_str()) {
                continue;
            }
            walk_grep(fs, &child, re, matches);
        } else {
            let Ok(data) = fs.read_file(&child) else {
                continue;
            };
            let Ok(content) = String::from_utf8(data) else {
                continue;
            };
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    #[allow(clippy::cast_possible_truncation)]
                    matches.push(VfsSearchMatch {
                        path: child.clone(),
                        line_number: (i + 1) as u32,
                        content: line.to_string(),
                        score: 1.0,
                    });
                }
            }
        }
    }
}

fn walk_find<F: VfsFileSystem>(
    fs: &F,
    dir: &str,
    re: &regex::Regex,
    matches: &mut Vec<VfsSearchMatch>,
) {
    let Ok(d) = fs.open_directory(dir) else {
        return;
    };
    let Ok(entries) = d.list() else {
        return;
    };
    for entry in entries {
        let child = if dir == "/" {
            format!("/{}", entry.name)
        } else {
            format!("{}/{}", dir, entry.name)
        };
        if re.is_match(&entry.name) {
            matches.push(VfsSearchMatch {
                path: child.clone(),
                line_number: 0,
                content: String::new(),
                score: 1.0,
            });
        }
        if entry.file_type == VfsFileType::Directory {
            if EXCLUDED_DIRS.contains(&entry.name.as_str()) {
                continue;
            }
            walk_find(fs, &child, re, matches);
        }
    }
}

// ---------------------------------------------------------------------------
// CliSearcher — native-only, tries rg then grep -rn
// ---------------------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
enum CliTool {
    Ripgrep,
    Grep,
}

#[cfg(not(target_family = "wasm"))]
pub struct CliSearcher {
    tool: CliTool,
}

#[cfg(not(target_family = "wasm"))]
impl CliSearcher {
    #[must_use]
    pub fn detect() -> Option<Self> {
        if std::process::Command::new("rg")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return Some(Self {
                tool: CliTool::Ripgrep,
            });
        }
        if std::process::Command::new("grep")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return Some(Self {
                tool: CliTool::Grep,
            });
        }
        None
    }

    fn run_grep(&self, query: &str, roots: &[String]) -> VfsResult<Vec<VfsSearchMatch>> {
        let output = match self.tool {
            CliTool::Ripgrep => {
                let mut cmd = std::process::Command::new("rg");
                cmd.arg("-n").arg("--no-heading");
                for dir in EXCLUDED_DIRS {
                    cmd.arg("--glob").arg(format!("!{dir}"));
                }
                cmd.arg("--").arg(query);
                cmd.args(roots);
                cmd.output()
            }
            CliTool::Grep => {
                let mut cmd = std::process::Command::new("grep");
                cmd.arg("-rn").arg("-E");
                for dir in EXCLUDED_DIRS {
                    cmd.arg(format!("--exclude-dir={dir}"));
                }
                cmd.arg("--").arg(query);
                cmd.args(roots);
                cmd.output()
            }
        };

        let output = output.map_err(|e| VfsError::Backend {
            message: format!("CLI search failed: {e}"),
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_grep_output(&stdout))
    }

    fn run_find(&self, query: &str, roots: &[String]) -> VfsResult<Vec<VfsSearchMatch>> {
        let output = match self.tool {
            CliTool::Ripgrep => {
                let mut cmd = std::process::Command::new("rg");
                cmd.arg("--files");
                for dir in EXCLUDED_DIRS {
                    cmd.arg("--glob").arg(format!("!{dir}"));
                }
                cmd.args(roots);
                cmd.output()
            }
            CliTool::Grep => {
                let mut cmd = std::process::Command::new("find");
                for (i, root) in roots.iter().enumerate() {
                    if i > 0 {
                        cmd.arg("-o");
                    }
                    cmd.arg(root);
                }
                cmd.arg("-type").arg("f");
                cmd.output()
            }
        };

        let output = output.map_err(|e| VfsError::Backend {
            message: format!("CLI find failed: {e}"),
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let re = regex::Regex::new(query).map_err(|e| VfsError::Backend {
            message: format!("invalid regex: {e}"),
        })?;

        Ok(stdout
            .lines()
            .filter(|line| !line.is_empty() && re.is_match(line))
            .map(|line| VfsSearchMatch {
                path: line.to_string(),
                line_number: 0,
                content: String::new(),
                score: 1.0,
            })
            .collect())
    }
}

#[cfg(not(target_family = "wasm"))]
impl VfsSearcher for CliSearcher {
    fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        match kind {
            VfsSearchKind::Grep | VfsSearchKind::MultiGrep => self.run_grep(query, roots),
            VfsSearchKind::Find => self.run_find(query, roots),
        }
    }
}

#[cfg(not(target_family = "wasm"))]
fn parse_grep_output(stdout: &str) -> Vec<VfsSearchMatch> {
    let mut matches = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        // Format: path:line_number:content
        let mut parts = line.splitn(3, ':');
        let Some(path) = parts.next() else {
            continue;
        };
        let Some(line_num_str) = parts.next() else {
            continue;
        };
        let content = parts.next().unwrap_or("");
        let Ok(line_number) = line_num_str.parse::<u32>() else {
            continue;
        };
        matches.push(VfsSearchMatch {
            path: path.to_string(),
            line_number,
            content: content.to_string(),
            score: 1.0,
        });
    }
    matches
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

impl VfsSearcher for CascadingVfsSearcher {
    fn is_available(&self) -> bool {
        self.backends.iter().any(VfsSearcher::is_available)
    }

    fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        for backend in &self.backends {
            if backend.is_available() {
                return backend.search(query, kind, roots);
            }
        }
        Err(VfsError::Unsupported {
            operation: "no search backend available".into(),
        }
        .into())
    }
}

// ---------------------------------------------------------------------------
// FffSearcher — native-only, wraps fff-search engine (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(all(not(target_family = "wasm"), feature = "vfs-search-fff"))]
pub struct FffSearcher {
    picker: fff_search::SharedFilePicker,
    base_path: String,
}

#[cfg(all(not(target_family = "wasm"), feature = "vfs-search-fff"))]
impl FffSearcher {
    pub fn new(root: &str) -> Result<Self, VfsError> {
        let picker = fff_search::SharedFilePicker::default();

        fff_search::file_picker::FilePicker::new_with_shared_state(
            picker.clone(),
            fff_search::SharedFrecency::default(),
            fff_search::FilePickerOptions {
                base_path: root.into(),
                mode: fff_search::FFFMode::Ai,
                watch: false,
                ..Default::default()
            },
        )
        .map_err(|e| VfsError::Backend {
            message: format!("fff init failed: {e}"),
        })?;

        picker.wait_for_scan(std::time::Duration::from_secs(30));

        Ok(Self {
            picker,
            base_path: root.into(),
        })
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "vfs-search-fff"))]
impl VfsSearcher for FffSearcher {
    fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        _roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        let guard = self.picker.read().map_err(|e| VfsError::Backend {
            message: format!("fff picker lock failed: {e}"),
        })?;
        let picker = guard.as_ref().ok_or_else(|| VfsError::Backend {
            message: "fff picker not initialized".into(),
        })?;

        let parser = fff_search::QueryParser::default();
        let parsed = parser.parse(query);

        match kind {
            VfsSearchKind::Grep | VfsSearchKind::MultiGrep => {
                let options = fff_search::grep::GrepSearchOptions {
                    max_file_size: fff_search::grep::MAX_FFFILE_SIZE,
                    max_matches_per_file: 50,
                    smart_case: true,
                    file_offset: 0,
                    page_limit: 200,
                    mode: fff_search::grep::GrepMode::PlainText,
                    time_budget_ms: 5000,
                    before_context: 0,
                    after_context: 0,
                    classify_definitions: false,
                    trim_whitespace: true,
                    abort_signal: None,
                };

                let result = picker.grep(&parsed, &options);
                let base = std::path::Path::new(&self.base_path);
                let matches = result
                    .matches
                    .iter()
                    .map(|m| {
                        let file = &result.files[m.file_index];
                        let path = file.absolute_path(picker, base);
                        #[allow(clippy::cast_possible_truncation)]
                        VfsSearchMatch {
                            path: path.to_string_lossy().to_string(),
                            line_number: m.line_number as u32,
                            content: m.line_content.clone(),
                            score: m.fuzzy_score.map_or(1.0, |s| s as f32 / 1000.0),
                        }
                    })
                    .collect();

                Ok(matches)
            }
            VfsSearchKind::Find => {
                let options = fff_search::FuzzySearchOptions {
                    max_threads: 0,
                    current_file: None,
                    pagination: fff_search::PaginationArgs {
                        offset: 0,
                        limit: 200,
                    },
                    ..Default::default()
                };

                let result = picker.fuzzy_search(&parsed, None, options);
                let base = std::path::Path::new(&self.base_path);
                let matches = result
                    .items
                    .iter()
                    .map(|item| {
                        let path = item.absolute_path(picker, base);
                        VfsSearchMatch {
                            path: path.to_string_lossy().to_string(),
                            line_number: 0,
                            content: String::new(),
                            score: 1.0,
                        }
                    })
                    .collect();

                Ok(matches)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory functions
// ---------------------------------------------------------------------------

#[must_use]
pub fn vfs_searcher<F: VfsFileSystem + 'static>(fs: Arc<F>) -> Box<dyn VfsSearcher> {
    Box::new(InCodeVfsSearcher::new(fs))
}

#[must_use]
#[cfg(not(target_family = "wasm"))]
pub fn native_vfs_searcher<F: VfsFileSystem + 'static>(fs: Arc<F>) -> Box<dyn VfsSearcher> {
    let mut backends: Vec<Box<dyn VfsSearcher>> = Vec::new();

    #[cfg(feature = "vfs-search-fff")]
    if let Ok(fff) = FffSearcher::new(".") {
        backends.push(Box::new(fff));
    }

    if let Some(cli) = CliSearcher::detect() {
        backends.push(Box::new(cli));
    }

    backends.push(Box::new(InCodeVfsSearcher::new(fs)));

    Box::new(CascadingVfsSearcher::new(backends))
}
