//! Read workspace defaults and package settings.
use foundation_tasks::TaskError;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use toml::Table;

#[derive(Debug, Deserialize, Clone)]
pub struct ExpectedError {
    pub run: String,
    pub contains: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    pub skip: Option<String>,
    pub reason: Option<String>,
    pub fmt: String,
    pub build: String,
    pub clippy: String,
    pub test: String,
    pub wasm: String,
    pub runs: Vec<String>,
    pub clippy_runs: Vec<String>,
    pub wasm_runs: Vec<String>,
    pub test_args: Vec<String>,
    pub clippy_deny_warnings: bool,
    pub wasm_deny_warnings: bool,
    pub failure_lines: usize,
    /// Retain up to 8 KiB per diagnostic line by default.
    pub failure_line_bytes: usize,
    pub timeout_minutes: usize,
    pub container_options: String,
    pub env: BTreeMap<String, String>,
    pub wasm_expect_error: Option<ExpectedError>,
    macos_runs: Option<Vec<String>>,
    windows_runs: Option<Vec<String>>,
    macos_env: Option<BTreeMap<String, String>>,
    windows_env: Option<BTreeMap<String, String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            skip: None,
            reason: None,
            fmt: "report".into(),
            build: "enforce".into(),
            clippy: "enforce".into(),
            test: "enforce".into(),
            wasm: "off".into(),
            runs: vec![String::new()],
            clippy_runs: Vec::new(),
            wasm_runs: vec![String::new()],
            test_args: Vec::new(),
            clippy_deny_warnings: false,
            wasm_deny_warnings: false,
            failure_lines: 200,
            failure_line_bytes: 8192,
            timeout_minutes: 60,
            container_options: String::new(),
            env: BTreeMap::new(),
            wasm_expect_error: None,
            macos_runs: None,
            windows_runs: None,
            macos_env: None,
            windows_env: None,
        }
    }
}

impl Settings {
    fn validate_modes(&self, name: &str) -> Result<(), TaskError> {
        for (key, mode) in [
            ("fmt", &self.fmt),
            ("build", &self.build),
            ("clippy", &self.clippy),
            ("test", &self.test),
            ("wasm", &self.wasm),
        ] {
            if !matches!(mode.as_str(), "enforce" | "report" | "off") {
                return Err(TaskError::operation(format!(
                    "{name}: invalid {key} mode: {mode}"
                )));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn platform_runs(&self, platform: &str) -> Option<&[String]> {
        match platform {
            "macos" => self.macos_runs.as_deref(),
            "windows" => self.windows_runs.as_deref(),
            _ => None,
        }
    }
    #[must_use]
    pub fn platform_env(&self, platform: &str) -> &BTreeMap<String, String> {
        match platform {
            "macos" => self.macos_env.as_ref(),
            "windows" => self.windows_env.as_ref(),
            _ => None,
        }
        .unwrap_or(&self.env)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Policy {
    pub defaults: Table,
    pub packages: BTreeMap<String, Table>,
    pub groups: BTreeMap<String, Group>,
    pub gates: BTreeMap<String, Gate>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GroupRecipe {
    ExampleApplications,
    WasmRuntimeTests,
    ManifestContract,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Group {
    pub directory: std::path::PathBuf,
    pub recipe: GroupRecipe,
    pub build_members: Vec<String>,
}

/// A check outside the Cargo package matrix: a named set of commands that runs
/// in its own workflow job, selected from the changed packages and paths.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GateRecipe {
    DurableConformance,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Gate {
    #[serde(default)]
    pub description: String,
    pub recipe: GateRecipe,
    /// Workspace packages whose change selects the gate.
    #[serde(default)]
    pub packages: Vec<String>,
    /// Repository-relative path prefixes whose change selects the gate.
    #[serde(default)]
    pub paths: Vec<String>,
}

pub(crate) fn features(run: &str) -> Vec<&str> {
    if !run.trim_start().starts_with('-') {
        return run
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|x| !x.is_empty())
            .collect();
    }
    let mut words = run.split_whitespace();
    let mut names = Vec::new();
    while let Some(flag) = words.next() {
        let args = if matches!(flag, "--features" | "-F") {
            words.next()
        } else {
            flag.strip_prefix("--features=")
        };
        if let Some(args) = args {
            names.extend(args.split(',').filter(|s| !s.is_empty()));
        }
    }
    names
}

impl Policy {
    pub fn positive(&self, key: &str, default: usize) -> Result<usize, TaskError> {
        self.defaults.get(key).map_or(Ok(default), |v| {
            v.as_integer()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    TaskError::operation(format!("[defaults]: {key} must be a positive integer"))
                })
        })
    }
    pub fn settings(
        &self,
        name: &str,
        available: Option<&BTreeSet<String>>,
    ) -> Result<Settings, TaskError> {
        let mut merged = self.defaults.clone();
        let package = self.packages.get(name).cloned().unwrap_or_default();
        merged.extend(package.clone());
        let mut s: Settings = toml::Value::Table(merged)
            .try_into()
            .map_err(TaskError::operation)?;
        s.validate_modes(name)?;
        if s.runs.is_empty() {
            s.runs.push(String::new());
        }
        if s.wasm_runs.is_empty() {
            s.wasm_runs.push(String::new());
        }
        if let Some(v) = self.defaults.get("env") {
            let mut env: BTreeMap<String, String> =
                v.clone().try_into().map_err(TaskError::operation)?;
            env.extend(s.env);
            s.env = env;
        }
        let linux: Vec<String> = self
            .defaults
            .get("linux-only-features")
            .cloned()
            .map(toml::Value::try_into)
            .transpose()
            .map_err(TaskError::operation)?
            .unwrap_or_default();
        for (os, runs, env) in [
            ("macos", &mut s.macos_runs, &mut s.macos_env),
            ("windows", &mut s.windows_runs, &mut s.windows_env),
        ] {
            let key = format!("{os}-runs");
            if let Some(override_runs) = runs.as_ref() {
                if override_runs.is_empty() {
                    return Err(TaskError::operation(format!("{name}: empty {key}")));
                }
                if s.skip.is_none() {
                    for feature in override_runs.iter().flat_map(|run| features(run)) {
                        let base = feature
                            .split('/')
                            .next()
                            .unwrap_or(feature)
                            .trim_start_matches("dep:");
                        if available.is_some_and(|names| !names.contains(base)) {
                            return Err(TaskError::operation(format!(
                                "{name}: {key} names undefined feature {feature}"
                            )));
                        }
                    }
                }
            }
            if let Some(opt) = package.get(&format!("{os}-tests")) {
                if opt.as_str() != Some("none") || runs.is_some() {
                    return Err(TaskError::operation(format!(
                        "{name}: invalid {os}-tests or conflicting {key}"
                    )));
                }
                let reason = package
                    .get(&format!("{os}-reason"))
                    .and_then(toml::Value::as_str)
                    .filter(|r| !r.is_empty())
                    .or(s.reason.as_deref());
                if s.skip.is_none() && reason.is_none_or(|r| r.trim().is_empty()) {
                    return Err(TaskError::operation(format!(
                        "{name}: {os}-tests opt-out needs a reason"
                    )));
                }
            } else if runs.is_none() {
                let filtered: Vec<_> = s
                    .runs
                    .iter()
                    .filter(|r| !features(r).iter().any(|f| linux.iter().any(|x| x == f)))
                    .cloned()
                    .collect();
                if !filtered.is_empty() {
                    *runs = Some(filtered);
                }
            }
            if env.is_none() {
                *env = Some(
                    s.env
                        .iter()
                        .filter(|(k, _)| !k.starts_with("EWE_REQUIRE_"))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                );
            }
        }
        Ok(s)
    }
}
