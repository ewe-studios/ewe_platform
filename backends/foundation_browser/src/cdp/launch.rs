//! Chromium launch configuration (spec-43 phase-1 §4).

use std::borrow::Cow;

/// Which browser to drive. Phase 1 is Chromium-only; the enum reserves the
/// others so signatures don't churn when BiDi/Firefox lands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Browser {
    /// Chromium / Chrome (CDP).
    #[default]
    Chromium,
}

impl Browser {
    /// Candidate binary names searched on `PATH` (after the `*_BIN` env var).
    pub(crate) fn binaries(self) -> &'static [&'static str] {
        match self {
            Browser::Chromium => &["chromium", "google-chrome", "chromium-browser", "chrome"],
        }
    }

    /// The env var overriding the binary path.
    pub(crate) fn bin_env(self) -> &'static str {
        match self {
            Browser::Chromium => "PRIMAL_TEST_CHROMIUM_BIN",
        }
    }

    /// The `mise` task that installs it (named in the not-installed error).
    pub(crate) fn mise_task(self) -> &'static str {
        match self {
            Browser::Chromium => "mise run test:browsers:chromium",
        }
    }
}

/// How to launch the browser.
#[derive(Clone, Debug)]
pub struct LaunchConfig {
    /// Which browser.
    pub browser: Browser,
    /// Run headless (default true).
    pub headless: bool,
    /// Extra command-line args appended after our defaults.
    pub extra_args: Vec<Cow<'static, str>>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self { browser: Browser::Chromium, headless: true, extra_args: Vec::new() }
    }
}

impl LaunchConfig {
    /// Chromium with defaults.
    #[must_use]
    pub fn chromium() -> Self {
        Self::default()
    }

    /// Toggle headless.
    #[must_use]
    pub fn headless(mut self, on: bool) -> Self {
        self.headless = on;
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
