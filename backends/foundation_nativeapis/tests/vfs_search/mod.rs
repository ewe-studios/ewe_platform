use std::sync::Arc;

use foundation_core::valtron::valtron_test;

use foundation_nativeapis::{
    InCodeVfsSearcher, MemoryFs, VfsFileSystem, VfsSearchKind, VfsSearcher,
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
