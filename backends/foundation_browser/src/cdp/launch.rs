//! Chromium launch configuration (spec-43 phase-1 §4).

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use foundation_context::Ctx;

use crate::error::{BrowserError, Result};
use crate::jsonrpc::RpcEngineConfig;
use crate::pause::WaitConfig;

/// Which browser to drive: Chromium over CDP, or Firefox over `WebDriver` `BiDi`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Browser {
    /// Chromium / Chrome (CDP).
    #[default]
    Chromium,
    /// Firefox (`WebDriver` `BiDi`).
    Firefox,
}

impl Browser {
    /// Candidate binary names searched on `PATH` (after the `*_BIN` env var).
    pub(crate) fn binaries(self) -> &'static [&'static str] {
        match self {
            Browser::Chromium => &["chromium", "google-chrome", "chromium-browser", "chrome"],
            Browser::Firefox => &["firefox", "firefox-developer-edition", "firefox-esr"],
        }
    }

    /// The env var overriding the binary path.
    pub(crate) fn bin_env(self) -> &'static str {
        match self {
            Browser::Chromium => "PRIMAL_TEST_CHROMIUM_BIN",
            Browser::Firefox => "PRIMAL_TEST_FIREFOX_BIN",
        }
    }

    /// The `mise` task that installs it (named in the not-installed error).
    pub(crate) fn mise_task(self) -> &'static str {
        match self {
            Browser::Chromium => "mise run test:browsers:chromium",
            Browser::Firefox => "mise run test:browsers:firefox",
        }
    }
}

/// The host operating system whose application paths a search uses.
///
/// It is a value, not a compile-time gate, so a test asserts the paths of every
/// operating system on one host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOs {
    /// Linux.
    Linux,
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
}

impl HostOs {
    /// The operating system this binary runs on.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Every location a browser search reads, in the order of invariant I4.
///
/// A test builds one directly, so it asserts each branch without changing the
/// process environment. [`BrowserSearch::from_env`] reads the real environment.
#[derive(Clone, Debug)]
pub struct BrowserSearch {
    /// Which browser to resolve.
    pub browser: Browser,
    /// The operating system whose application paths apply.
    pub host_os: HostOs,
    /// The path named by the browser's `*_BIN` env variable, when set.
    pub env_override: Option<PathBuf>,
    /// `FOUNDATION_CHROMIUM_DIR`, when set. The install task writes here.
    pub chromium_dir: Option<PathBuf>,
    /// `HOME` on Linux and macOS.
    pub home: Option<PathBuf>,
    /// `LOCALAPPDATA` on Windows. The cache lives under it.
    pub local_app_data: Option<PathBuf>,
    /// `PROGRAMFILES` on Windows.
    pub program_files: Option<PathBuf>,
    /// The directories of `PATH`.
    pub path_dirs: Vec<PathBuf>,
}

impl BrowserSearch {
    /// An empty search for `browser` on `host_os`.
    ///
    /// Every optional location is unset and `PATH` holds no directory, so a
    /// test sets only the branch it asserts.
    #[must_use]
    pub fn for_host(browser: Browser, host_os: HostOs) -> Self {
        Self {
            browser,
            host_os,
            env_override: None,
            chromium_dir: None,
            home: None,
            local_app_data: None,
            program_files: None,
            path_dirs: Vec::new(),
        }
    }

    /// Read the process environment for `browser` on this host.
    #[must_use]
    pub fn from_env(browser: Browser) -> Self {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let host_os = HostOs::current();
        Self {
            browser,
            host_os,
            env_override: var(browser.bin_env()),
            chromium_dir: var("FOUNDATION_CHROMIUM_DIR"),
            // `HOME` is a POSIX name; Windows uses `LOCALAPPDATA`, below.
            home: if host_os == HostOs::Windows {
                None
            } else {
                var("HOME")
            },
            local_app_data: var("LOCALAPPDATA"),
            program_files: var("PROGRAMFILES"),
            path_dirs: std::env::var_os("PATH")
                .map_or_else(Vec::new, |path| std::env::split_paths(&path).collect()),
        }
    }

    /// The executable name the install task writes into the cache directory.
    fn cached_binary_name(host_os: HostOs) -> &'static str {
        if host_os == HostOs::Windows {
            "chrome.exe"
        } else {
            "chrome"
        }
    }

    /// The candidates, in the order of invariant I4.
    ///
    /// An entry is an absolute path or a bare name that a directory of `PATH`
    /// holds. The Chromium cache and application paths come before the names.
    #[must_use]
    pub fn candidates(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(path) = &self.env_override {
            out.push(path.clone());
        }
        if self.browser == Browser::Chromium {
            let binary = Self::cached_binary_name(self.host_os);
            // 2. The cache the install task writes.
            if let Some(dir) = &self.chromium_dir {
                out.push(dir.join(binary));
            }
            match self.host_os {
                HostOs::Windows => {
                    if let Some(dir) = &self.local_app_data {
                        out.push(dir.join("foundation").join("chromium").join(binary));
                    }
                }
                HostOs::Linux | HostOs::MacOs => {
                    if let Some(home) = &self.home {
                        out.push(
                            home.join(".cache")
                                .join("foundation")
                                .join("chromium")
                                .join(binary),
                        );
                    }
                }
            }
            // 3. The OS application paths.
            match self.host_os {
                HostOs::MacOs => {
                    out.push(PathBuf::from(
                        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                    ));
                    out.push(PathBuf::from(
                        "/Applications/Chromium.app/Contents/MacOS/Chromium",
                    ));
                }
                HostOs::Windows => {
                    if let Some(dir) = &self.program_files {
                        out.push(
                            dir.join("Google")
                                .join("Chrome")
                                .join("Application")
                                .join(binary),
                        );
                    }
                }
                HostOs::Linux => {}
            }
        }
        // 4. The names on `PATH`.
        out.extend(self.browser.binaries().iter().map(PathBuf::from));
        out
    }

    /// Resolve the first candidate that a file backs.
    ///
    /// # Errors
    /// [`BrowserError::BrowserNotInstalled`] when no candidate exists. The error
    /// lists every location searched, so a user sees the gap (S7).
    pub fn resolve(&self) -> Result<PathBuf> {
        let candidates = self.candidates();
        match Self::first_existing(&candidates, &self.path_dirs) {
            Some(path) => Ok(path),
            None => Err(BrowserError::BrowserNotInstalled {
                binary: self.browser.binaries()[0].to_string(),
                mise_task: self.browser.mise_task().to_string(),
                searched: candidates,
            }),
        }
    }

    /// The first candidate that a file backs.
    ///
    /// An absolute path resolves when it is a file. A bare name resolves through
    /// the first directory of `path_dirs` that holds it.
    #[must_use]
    pub fn first_existing(candidates: &[PathBuf], path_dirs: &[PathBuf]) -> Option<PathBuf> {
        for candidate in candidates {
            let bare = candidate.is_relative() && candidate.components().count() == 1;
            if bare {
                for dir in path_dirs {
                    let full = dir.join(candidate);
                    if full.is_file() {
                        return Some(full);
                    }
                }
            } else if candidate.is_file() {
                return Some(candidate.clone());
            }
        }
        None
    }
}

/// The browser binary for `config`, resolved in the order of invariant I4.
///
/// The testbed's preflight and [`BrowserProcess::launch`](crate::supervisor::BrowserProcess::launch)
/// call this same function, so they agree on the binary a run uses.
///
/// # Errors
/// [`BrowserError::BrowserNotInstalled`] when no candidate exists.
pub fn resolve_browser(config: &LaunchConfig) -> Result<PathBuf> {
    BrowserSearch::from_env(config.browser).resolve()
}

/// How to launch the browser.
#[derive(Clone)]
pub struct LaunchConfig {
    /// Which browser.
    pub browser: Browser,
    /// Run headless (default true).
    pub headless: bool,
    /// Extra command-line args appended after our defaults.
    pub extra_args: Vec<Cow<'static, str>>,
    /// The context for launch and retry deadlines.
    ///
    /// [`BrowserDriver::launch`](crate::BrowserDriver::launch) panics when it is
    /// absent. Set it with [`LaunchConfig::ctx`].
    pub ctx: Option<Ctx>,
    /// The intervals of every launch wait loop. See [`WaitConfig`].
    pub wait: WaitConfig,
    /// The limits of the protocol connection (timeouts and the idle pause).
    pub engine: RpcEngineConfig,
    /// The budget of one navigation, in clock time.
    ///
    /// The default is 30 s: Chrome loaded the smoke test's `data:` page in
    /// under a second on macOS, and a full page with a wasm module takes about
    /// a second. A dead navigation must still fail in bounded time.
    pub navigate_timeout: Duration,
}

impl fmt::Debug for LaunchConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchConfig")
            .field("browser", &self.browser)
            .field("headless", &self.headless)
            .field("extra_args", &self.extra_args)
            .field("ctx", &self.ctx.is_some())
            .field("wait", &self.wait)
            .field("engine", &self.engine)
            .field("navigate_timeout", &self.navigate_timeout)
            .finish()
    }
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            browser: Browser::Chromium,
            headless: true,
            extra_args: Vec::new(),
            ctx: None,
            wait: WaitConfig::default(),
            engine: RpcEngineConfig::default(),
            navigate_timeout: Duration::from_secs(30),
        }
    }
}

impl LaunchConfig {
    /// Chromium with defaults.
    #[must_use]
    pub fn chromium() -> Self {
        Self::default()
    }

    /// Firefox (`WebDriver` `BiDi`) with defaults.
    #[must_use]
    pub fn firefox() -> Self {
        Self {
            browser: Browser::Firefox,
            headless: true,
            extra_args: Vec::new(),
            ctx: None,
            wait: WaitConfig::default(),
            engine: RpcEngineConfig::default(),
            navigate_timeout: Duration::from_secs(30),
        }
    }

    /// Set the context every launch and retry deadline reads.
    #[must_use]
    pub fn ctx(mut self, ctx: Ctx) -> Self {
        self.ctx = Some(ctx);
        self
    }

    /// Set the intervals of the launch wait loops.
    #[must_use]
    pub fn wait(mut self, wait: WaitConfig) -> Self {
        self.wait = wait;
        self
    }

    /// Toggle headless.
    #[must_use]
    pub fn headless(mut self, on: bool) -> Self {
        self.headless = on;
        self
    }

    /// Set the limits of the protocol connection.
    #[must_use]
    pub fn engine(mut self, engine: RpcEngineConfig) -> Self {
        self.engine = engine;
        self
    }

    /// Set the budget of one navigation.
    #[must_use]
    pub fn navigate_timeout(mut self, timeout: Duration) -> Self {
        self.navigate_timeout = timeout;
        self
    }

    /// The hermetic flag set (subset of Playwright's `chromiumSwitches`). The
    /// caller appends `--user-data-dir` and `--remote-debugging-port=0`.
    pub(crate) fn base_args(&self) -> Vec<String> {
        let mut args = vec![
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-gpu".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-extensions".to_string(),
            "--remote-allow-origins=*".to_string(),
            "--no-sandbox".to_string(),
        ];
        if self.headless {
            args.push("--headless=new".to_string());
        }
        args.extend(self.extra_args.iter().map(ToString::to_string));
        args
    }
}
