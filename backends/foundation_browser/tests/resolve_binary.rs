//! The browser search order (spec 88 invariant I4).
//!
//! The order is: the `*_BIN` env variable, the cache the install task writes,
//! the operating system application paths, then the names on `PATH`. Each test
//! builds a [`BrowserSearch`] directly, so no test changes the process
//! environment that another test reads.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use foundation_browser::cdp::launch::{BrowserSearch, HostOs};
use foundation_browser::{Browser, BrowserError};

/// A unique temporary directory, removed when the test ends.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "foundation-browser-resolve-{}-{sequence}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("make the temporary directory");
        Self { path }
    }

    /// Write an empty file at `relative` and return its full path.
    fn file(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("make the parent directory");
        }
        std::fs::write(&path, b"").expect("write the candidate file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A search with only the cache directory under a temporary home.
fn search_with_home(home: &std::path::Path, host_os: HostOs) -> BrowserSearch {
    let mut search = BrowserSearch::for_host(Browser::Chromium, host_os);
    search.home = Some(home.to_path_buf());
    search
}

#[test]
fn the_bin_env_variable_wins_over_every_other_location() {
    let temp = TempDir::new("env-wins");
    let override_path = temp.file("override/chrome");
    let cached = temp.file(".cache/foundation/chromium/chrome");
    let mut search = search_with_home(&temp.path, HostOs::Linux);
    search.env_override = Some(override_path.clone());

    assert_eq!(
        search.resolve().expect("the env override exists"),
        override_path
    );
    assert_ne!(override_path, cached);
}

#[test]
fn the_foundation_chromium_dir_precedes_the_home_cache() {
    let temp = TempDir::new("chromium-dir");
    let named = temp.file("named/chrome");
    let _cached = temp.file(".cache/foundation/chromium/chrome");
    let mut search = search_with_home(&temp.path, HostOs::Linux);
    search.chromium_dir = Some(temp.path.join("named"));

    assert_eq!(search.resolve().expect("the named cache exists"), named);
}

#[test]
fn the_home_cache_resolves_when_it_holds_the_binary() {
    let temp = TempDir::new("home-cache");
    let cached = temp.file(".cache/foundation/chromium/chrome");
    let search = search_with_home(&temp.path, HostOs::Linux);

    assert_eq!(
        search.resolve().expect("the cache holds the binary"),
        cached
    );
}

#[test]
fn the_macos_application_paths_follow_the_cache() {
    let home = PathBuf::from("/Users/test");
    let search = search_with_home(&home, HostOs::MacOs);
    let candidates = search.candidates();

    assert_eq!(
        candidates[0],
        home.join(".cache")
            .join("foundation")
            .join("chromium")
            .join("chrome")
    );
    assert_eq!(
        candidates[1],
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    );
    assert_eq!(
        candidates[2],
        PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium")
    );
    assert_eq!(candidates[3], PathBuf::from("chromium"));
}

#[test]
fn windows_searches_local_app_data_then_program_files() {
    let local = PathBuf::from(r"C:\Users\test\AppData\Local");
    let program_files = PathBuf::from(r"C:\Program Files");
    let mut search = BrowserSearch::for_host(Browser::Chromium, HostOs::Windows);
    search.local_app_data = Some(local.clone());
    search.program_files = Some(program_files.clone());
    let candidates = search.candidates();

    assert_eq!(
        candidates[0],
        local.join("foundation").join("chromium").join("chrome.exe")
    );
    assert_eq!(
        candidates[1],
        program_files
            .join("Google")
            .join("Chrome")
            .join("Application")
            .join("chrome.exe")
    );
    assert_eq!(candidates[2], PathBuf::from("chromium"));
}

#[test]
fn linux_has_no_application_paths() {
    let home = PathBuf::from("/home/test");
    let search = search_with_home(&home, HostOs::Linux);
    let candidates = search.candidates();

    assert_eq!(
        candidates,
        vec![
            home.join(".cache")
                .join("foundation")
                .join("chromium")
                .join("chrome"),
            PathBuf::from("chromium"),
            PathBuf::from("google-chrome"),
            PathBuf::from("chromium-browser"),
            PathBuf::from("chrome"),
        ]
    );
}

#[test]
fn a_bare_name_resolves_through_the_path_directories() {
    let temp = TempDir::new("path-name");
    let binary = temp.file("bin/chrome");
    let mut search = BrowserSearch::for_host(Browser::Chromium, HostOs::Linux);
    search.path_dirs = vec![temp.path.join("bin")];

    assert_eq!(search.resolve().expect("PATH holds the name"), binary);
}

#[test]
fn the_first_existing_candidate_wins() {
    let temp = TempDir::new("first-existing");
    let earlier = temp.file("earlier/chrome");
    let _later = temp.file("later/chrome");
    let mut search = BrowserSearch::for_host(Browser::Chromium, HostOs::Linux);
    search.env_override = Some(earlier.clone());
    search.chromium_dir = Some(temp.path.join("later"));

    assert_eq!(search.resolve().expect("the earlier file exists"), earlier);
}

#[test]
fn a_firefox_search_reads_only_the_names_on_path() {
    let mut search = BrowserSearch::for_host(Browser::Firefox, HostOs::MacOs);
    search.home = Some(PathBuf::from("/Users/test"));

    assert_eq!(
        search.candidates(),
        vec![
            PathBuf::from("firefox"),
            PathBuf::from("firefox-developer-edition"),
            PathBuf::from("firefox-esr"),
        ]
    );
}

#[test]
fn a_missing_browser_names_every_searched_path_and_the_install_task() {
    let temp = TempDir::new("missing");
    let search = search_with_home(&temp.path, HostOs::Linux);
    let candidates = search.candidates();

    let error = search.resolve().expect_err("no candidate exists");
    let message = error.to_string();
    match error {
        BrowserError::BrowserNotInstalled {
            binary,
            mise_task,
            searched,
        } => {
            assert_eq!(binary, "chromium");
            assert_eq!(mise_task, "mise run test:browsers:chromium");
            assert_eq!(searched, candidates);
        }
        other => panic!("expected BrowserNotInstalled, got {other}"),
    }
    assert!(
        message.contains("mise run test:browsers:chromium"),
        "{message}"
    );
    assert!(
        message.contains(&candidates[0].display().to_string()),
        "the message must name the first searched path: {message}"
    );
}
