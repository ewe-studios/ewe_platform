//! Search over a VFS: grep and find. `InCodeVfsSearcher` walks any
//! `AsyncVfsFileSystem`; on native targets `CliSearcher` (rg / grep) and
//! `FffSearcher` search the host file system directly.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use std::io::BufRead;
#[cfg(not(target_family = "wasm"))]
use std::process::{Command, Stdio};
#[cfg(not(target_family = "wasm"))]
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[cfg(not(target_family = "wasm"))]
use foundation_core::valtron::{BackgroundJobConfig, BackgroundJobRegistry, Pipe};
#[cfg(not(target_family = "wasm"))]
use foundation_errstacks::ErrorTrace;

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

const EXCLUDED_DIRS: &[&str] = &[
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
            matches
                .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
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
// SearchConfig — limits for the native search backends
// ---------------------------------------------------------------------------

/// Limits for the native search backends (rule 8: limits are config, not
/// constants).
///
/// The fff searcher uses every field. The CLI searcher uses
/// [`Self::max_matches_per_file`] (mapped to `--max-count`); `rg` and `grep`
/// have no page or time limit to set.
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchConfig {
    max_matches_per_file: usize,
    page_limit: usize,
    time_budget: Duration,
    scan_timeout: Duration,
}

#[cfg(not(target_family = "wasm"))]
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_matches_per_file: Self::DEFAULT_MAX_MATCHES_PER_FILE,
            page_limit: Self::DEFAULT_PAGE_LIMIT,
            time_budget: Self::DEFAULT_TIME_BUDGET,
            scan_timeout: Self::DEFAULT_SCAN_TIMEOUT,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl SearchConfig {
    /// Default matches kept per file: 50. fff-search's default is 200; 50
    /// keeps one hot file from crowding a page of results.
    pub const DEFAULT_MAX_MATCHES_PER_FILE: usize = 50;

    /// Default page size: 200. fff-search paginates at 200 items; Caddy's
    /// default request-body limit is a similar small, bounded page.
    pub const DEFAULT_PAGE_LIMIT: usize = 200;

    /// Default grep time budget: 5 s. fff-search stops a grep past this
    /// budget; nginx's default `proxy_connect_timeout` is 60 s, so 5 s keeps
    /// one query from holding a worker long.
    pub const DEFAULT_TIME_BUDGET: Duration = Duration::from_secs(5);

    /// Default scan wait: 30 s. The fff file scan must finish within this;
    /// nginx's default `proxy_connect_timeout` is 60 s, so 30 s fails a stuck
    /// scan in half that time.
    pub const DEFAULT_SCAN_TIMEOUT: Duration = Duration::from_secs(30);

    /// A config with the documented defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the matches kept per file.
    #[must_use]
    pub fn with_max_matches_per_file(mut self, limit: usize) -> Self {
        self.max_matches_per_file = limit;
        self
    }

    /// Set the page size.
    #[must_use]
    pub fn with_page_limit(mut self, limit: usize) -> Self {
        self.page_limit = limit;
        self
    }

    /// Set the grep time budget.
    #[must_use]
    pub fn with_time_budget(mut self, budget: Duration) -> Self {
        self.time_budget = budget;
        self
    }

    /// Set the initial file-scan wait.
    #[must_use]
    pub fn with_scan_timeout(mut self, timeout: Duration) -> Self {
        self.scan_timeout = timeout;
        self
    }

    /// Matches kept per file.
    #[must_use]
    pub fn max_matches_per_file(&self) -> usize {
        self.max_matches_per_file
    }

    /// Page size.
    #[must_use]
    pub fn page_limit(&self) -> usize {
        self.page_limit
    }

    /// Grep time budget.
    #[must_use]
    pub fn time_budget(&self) -> Duration {
        self.time_budget
    }

    /// Initial file-scan wait.
    #[must_use]
    pub fn scan_timeout(&self) -> Duration {
        self.scan_timeout
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
    config: SearchConfig,
    pool: Arc<BackgroundJobRegistry>,
}

#[cfg(not(target_family = "wasm"))]
impl CliSearcher {
    /// Detect `rg` or `grep` with the default search limits.
    #[must_use]
    pub fn detect() -> Option<Self> {
        Self::detect_with_config(SearchConfig::default())
    }

    /// Detect `rg` or `grep` with the given search limits.
    #[must_use]
    pub fn detect_with_config(config: SearchConfig) -> Option<Self> {
        let tool = detect_tool()?;
        let pool = BackgroundJobRegistry::with_config(BackgroundJobConfig::new(1)).ok()?;
        Some(Self { tool, config, pool })
    }

    /// The limits this searcher uses.
    #[must_use]
    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    async fn run_grep(&self, query: &str, roots: &[String]) -> VfsResult<Vec<VfsSearchMatch>> {
        let max = self.config.max_matches_per_file();
        let cmd = match self.tool {
            CliTool::Ripgrep => {
                let mut cmd = Command::new("rg");
                cmd.arg("-n").arg("--no-heading");
                cmd.arg("--max-count").arg(max.to_string());
                for dir in EXCLUDED_DIRS {
                    cmd.arg("--glob").arg(format!("!{dir}"));
                }
                cmd.arg("--").arg(query);
                cmd.args(roots);
                cmd
            }
            CliTool::Grep => {
                let mut cmd = Command::new("grep");
                cmd.arg("-rn").arg("-E");
                cmd.arg(format!("--max-count={max}"));
                for dir in EXCLUDED_DIRS {
                    cmd.arg(format!("--exclude-dir={dir}"));
                }
                cmd.arg("--").arg(query);
                cmd.args(roots);
                cmd
            }
        };
        run_cli_streaming(&self.pool, cmd, parse_grep_line).await
    }

    async fn run_find(&self, query: &str, roots: &[String]) -> VfsResult<Vec<VfsSearchMatch>> {
        let cmd = match self.tool {
            CliTool::Ripgrep => {
                let mut cmd = Command::new("rg");
                cmd.arg("--files");
                for dir in EXCLUDED_DIRS {
                    cmd.arg("--glob").arg(format!("!{dir}"));
                }
                cmd.args(roots);
                cmd
            }
            CliTool::Grep => {
                let mut cmd = Command::new("find");
                for (i, root) in roots.iter().enumerate() {
                    if i > 0 {
                        cmd.arg("-o");
                    }
                    cmd.arg(root);
                }
                cmd.arg("-type").arg("f");
                cmd
            }
        };
        let re = regex::Regex::new(query).map_err(|e| VfsError::Backend {
            message: format!("invalid regex: {e}"),
        })?;
        run_cli_streaming(&self.pool, cmd, move |line| {
            if line.is_empty() || !re.is_match(line) {
                return None;
            }
            Some(VfsSearchMatch {
                path: line.to_string(),
                line_number: 0,
                content: String::new(),
                score: 1.0,
            })
        })
        .await
    }
}

/// Detects a CLI search tool with a fast `--version` status check.
#[cfg(not(target_family = "wasm"))]
fn detect_tool() -> Option<CliTool> {
    if Command::new("rg")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        return Some(CliTool::Ripgrep);
    }
    if Command::new("grep")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        return Some(CliTool::Grep);
    }
    None
}

/// The child process runs on a background worker; the valtron worker only
/// awaits the result pipe.
#[cfg(not(target_family = "wasm"))]
#[async_trait]
impl VfsSearcher for CliSearcher {
    async fn search(
        &self,
        query: &str,
        kind: VfsSearchKind,
        roots: &[String],
    ) -> VfsResult<Vec<VfsSearchMatch>> {
        match kind {
            VfsSearchKind::Grep | VfsSearchKind::MultiGrep => self.run_grep(query, roots).await,
            VfsSearchKind::Find => self.run_find(query, roots).await,
        }
    }
}

/// Runs `cmd` on the offload pool and maps each stdout line as it streams.
#[cfg(not(target_family = "wasm"))]
async fn run_cli_streaming<F>(
    pool: &Arc<BackgroundJobRegistry>,
    cmd: Command,
    map: F,
) -> VfsResult<Vec<VfsSearchMatch>>
where
    F: Fn(&str) -> Option<VfsSearchMatch> + Send + 'static,
{
    let (tx, rx) = Pipe::new();
    pool.submit(move || {
        let result = run_and_collect(cmd, map);
        let _ = tx.try_send(result);
    })
    .map_err(|e| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("CLI search queue is full or closed: {e}"),
        })
    })?;
    match rx.receive().await {
        Some(result) => result,
        None => Err(ErrorTrace::new(VfsError::Backend {
            message: "CLI search worker dropped the result channel".to_string(),
        })),
    }
}

/// Spawns `cmd`, reads its stdout line by line, and waits for the child. The
/// body is streamed: no `read_to_end`, so long output is never held whole.
#[cfg(not(target_family = "wasm"))]
fn run_and_collect<F>(mut cmd: Command, map: F) -> VfsResult<Vec<VfsSearchMatch>>
where
    F: Fn(&str) -> Option<VfsSearchMatch>,
{
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            ErrorTrace::new(VfsError::Backend {
                message: format!("CLI search spawn failed: {e}"),
            })
        })?;
    let stdout = child.stdout.take().expect("stdout was piped");
    let reader = std::io::BufReader::new(stdout);
    let mut matches = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(line) => {
                if let Some(m) = map(&line) {
                    matches.push(m);
                }
            }
            Err(_) => break,
        }
    }
    child.wait().map_err(|e| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("CLI search wait failed: {e}"),
        })
    })?;
    Ok(matches)
}

/// One `path:line_number:content` line into a match, or `None` when malformed.
#[cfg(not(target_family = "wasm"))]
fn parse_grep_line(line: &str) -> Option<VfsSearchMatch> {
    if line.is_empty() {
        return None;
    }
    let mut parts = line.splitn(3, ':');
    let path = parts.next()?;
    let line_num_str = parts.next()?;
    let content = parts.next().unwrap_or("");
    let line_number = line_num_str.parse::<u32>().ok()?;
    Some(VfsSearchMatch {
        path: path.to_string(),
        line_number,
        content: content.to_string(),
        score: 1.0,
    })
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
// FffSearcher — native-only, wraps fff-search engine (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(all(not(target_family = "wasm"), feature = "vfs-search-fff"))]
pub struct FffSearcher {
    picker: fff_search::SharedFilePicker,
    base_path: String,
    config: SearchConfig,
}

#[cfg(all(not(target_family = "wasm"), feature = "vfs-search-fff"))]
impl FffSearcher {
    /// Create an fff searcher under `root` with the default limits.
    ///
    /// # Errors
    /// Returns `VfsError::Backend` when the fff picker cannot start.
    pub fn new(root: &str) -> Result<Self, VfsError> {
        Self::with_config(root, SearchConfig::default())
    }

    /// Create an fff searcher under `root` with the given limits.
    ///
    /// # Errors
    /// Returns `VfsError::Backend` when the fff picker cannot start.
    pub fn with_config(root: &str, config: SearchConfig) -> Result<Self, VfsError> {
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

        picker.wait_for_scan(config.scan_timeout());

        Ok(Self {
            picker,
            base_path: root.into(),
            config,
        })
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "vfs-search-fff"))]
#[async_trait]
impl VfsSearcher for FffSearcher {
    async fn search(
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
                let time_budget_ms =
                    u64::try_from(self.config.time_budget().as_millis()).unwrap_or(u64::MAX);
                let options = fff_search::grep::GrepSearchOptions {
                    max_file_size: fff_search::grep::MAX_FFFILE_SIZE,
                    max_matches_per_file: self.config.max_matches_per_file(),
                    smart_case: true,
                    file_offset: 0,
                    page_limit: self.config.page_limit(),
                    mode: fff_search::grep::GrepMode::PlainText,
                    time_budget_ms,
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
                            score: m.fuzzy_score.map_or(1.0, |s| f32::from(s) / 1000.0),
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
                        limit: self.config.page_limit(),
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
pub fn vfs_searcher<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Box<dyn VfsSearcher> {
    Box::new(InCodeVfsSearcher::new(fs))
}

#[must_use]
#[cfg(not(target_family = "wasm"))]
pub fn native_vfs_searcher<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Box<dyn VfsSearcher> {
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
