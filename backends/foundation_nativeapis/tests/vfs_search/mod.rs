use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::valtron_test;
use futures_lite::future::{yield_now, zip};

use foundation_nativeapis::{
    CliSearcher, InCodeVfsSearcher, MemoryFs, SearchConfig, VfsFileSystem, VfsSearchKind,
    VfsSearcher,
};

fn make_fs() -> Arc<MemoryFs> {
    let fs = MemoryFs::new();
    fs.mkdir_all("/src").unwrap();
    fs.write_file("/hello.txt", b"hello world\ngoodbye world\n")
        .unwrap();
    fs.write_file("/src/main.rs", b"fn main() {\n    println!(\"hi\");\n}\n")
        .unwrap();
    fs.write_file("/src/lib.rs", b"pub mod utils;\npub fn add(a: i32, b: i32) -> i32 { a + b }\n")
        .unwrap();
    fs.write_file("/readme.md", b"# Project\nA sample project.\n")
        .unwrap();
    Arc::new(fs)
}

#[valtron_test]
async fn grep_finds_matching_lines() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("hello", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    assert!(!matches.is_empty());
    assert_eq!(matches[0].line_number, 1);
    assert!(matches[0].content.contains("hello"));
    assert!(matches[0].path.contains("hello.txt"));
}

#[valtron_test]
async fn grep_no_match_returns_empty() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("nonexistent_xyz_123", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    assert!(matches.is_empty());
}

#[valtron_test]
async fn find_matches_filenames() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search(r"\.rs$", VfsSearchKind::Find, &["/".into()]).await
        .unwrap();
    assert_eq!(matches.len(), 2);
    let paths: Vec<&str> = matches.iter().map(|m| m.path.as_str()).collect();
    assert!(paths.contains(&"/src/main.rs"));
    assert!(paths.contains(&"/src/lib.rs"));
}

#[valtron_test]
async fn find_matches_by_name_pattern() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("readme", VfsSearchKind::Find, &["/".into()]).await
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert!(matches[0].path.contains("readme.md"));
}

#[valtron_test]
async fn grep_searches_subdirectory_root() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("fn", VfsSearchKind::Grep, &["/src".into()]).await
        .unwrap();
    assert!(matches.len() >= 2);
    assert!(matches.iter().all(|m| m.path.starts_with("/src/")));
}

#[valtron_test]
async fn grep_multiple_roots() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("world", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    assert_eq!(matches.len(), 2);
}

#[valtron_test]
async fn invalid_regex_returns_error() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let result = searcher.search("[invalid", VfsSearchKind::Grep, &["/".into()]).await;
    assert!(result.is_err());
}

#[valtron_test]
async fn multigrep_behaves_like_grep() {
    let fs = make_fs();
    let searcher = InCodeVfsSearcher::new(fs);
    let grep = searcher
        .search("println", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    let multi = searcher
        .search("println", VfsSearchKind::MultiGrep, &["/".into()]).await
        .unwrap();
    assert_eq!(grep.len(), multi.len());
}

#[valtron_test]
async fn excluded_dirs_are_skipped() {
    let fs = MemoryFs::new();
    fs.mkdir_all("/src").unwrap();
    fs.mkdir_all("/target/debug").unwrap();
    fs.mkdir_all("/node_modules/pkg").unwrap();
    fs.write_file("/src/main.rs", b"fn main() {}\n").unwrap();
    fs.write_file("/target/debug/build.rs", b"fn main() {}\n")
        .unwrap();
    fs.write_file("/node_modules/pkg/index.js", b"function main() {}\n")
        .unwrap();
    let fs = Arc::new(fs);
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("main", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert!(matches[0].path.contains("src/main.rs"));
}

#[valtron_test]
async fn vfs_searcher_factory_returns_working_searcher() {
    let fs = make_fs();
    let searcher = foundation_nativeapis::vfs_searcher(fs);
    assert!(searcher.is_available());
    let matches = searcher
        .search("hello", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    assert!(!matches.is_empty());
}

#[test]
fn search_match_fields_accessible() {
    let m = foundation_nativeapis::VfsSearchMatch {
        path: "/src/main.rs".into(),
        line_number: 42,
        content: "fn main()".into(),
        score: 1.0,
    };
    assert_eq!(m.path, "/src/main.rs");
    assert_eq!(m.line_number, 42);
    assert_eq!(m.content, "fn main()");
    assert!((m.score - 1.0).abs() < f32::EPSILON);
}

#[valtron_test]
async fn empty_fs_grep_returns_empty() {
    let fs = Arc::new(MemoryFs::new());
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("anything", VfsSearchKind::Grep, &["/".into()]).await
        .unwrap();
    assert!(matches.is_empty());
}

#[valtron_test]
async fn empty_fs_find_returns_empty() {
    let fs = Arc::new(MemoryFs::new());
    let searcher = InCodeVfsSearcher::new(fs);
    let matches = searcher
        .search("anything", VfsSearchKind::Find, &["/".into()]).await
        .unwrap();
    assert!(matches.is_empty());
}

// ── E7: native search limits and CLI process streaming ──

/// The search limits live on a config with documented defaults.
#[test]
fn search_limits_come_from_config() {
    let default = SearchConfig::new();
    assert_eq!(default.max_matches_per_file(), 50);
    assert_eq!(default.page_limit(), 200);
    assert_eq!(default.time_budget(), Duration::from_secs(5));
    assert_eq!(default.scan_timeout(), Duration::from_secs(30));

    let configured = SearchConfig::new()
        .with_max_matches_per_file(7)
        .with_page_limit(13)
        .with_time_budget(Duration::from_millis(1_200))
        .with_scan_timeout(Duration::from_secs(9));
    assert_eq!(configured.max_matches_per_file(), 7);
    assert_eq!(configured.page_limit(), 13);
    assert_eq!(configured.time_budget(), Duration::from_millis(1_200));
    assert_eq!(configured.scan_timeout(), Duration::from_secs(9));
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vfs_search_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The CLI searcher streams the child's stdout off the valtron worker: a
/// ticker keeps ticking while `rg`/`grep` runs.
#[valtron_test]
async fn cli_search_streams_results() {
    let Some(searcher) = CliSearcher::detect() else {
        // Neither rg nor grep is installed; nothing to exercise.
        return;
    };

    let tmp = TempDir::new("cli_search");
    std::fs::write(tmp.path().join("a.txt"), "hello world\n").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "hello there\n").unwrap();

    let ticks = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (ticks, done) = (Arc::clone(&ticks), Arc::clone(&done));
        async move {
            while !done.load(Ordering::SeqCst) {
                ticks.fetch_add(1, Ordering::SeqCst);
                yield_now().await;
            }
        }
    };
    let search = {
        let done = Arc::clone(&done);
        async move {
            let result = searcher
                .search(
                    "hello",
                    VfsSearchKind::Grep,
                    &[tmp.path().to_string_lossy().into_owned()],
                )
                .await;
            done.store(true, Ordering::SeqCst);
            result
        }
    };

    let (result, ()) = zip(search, ticker).await;
    let matches = result.expect("CLI grep succeeded");
    assert_eq!(matches.len(), 2, "one match per file");
    assert!(
        ticks.load(Ordering::SeqCst) > 0,
        "the ticker must tick while the CLI search runs, saw {}",
        ticks.load(Ordering::SeqCst)
    );
}
