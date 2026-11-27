//! Construct native tool arguments from package settings.
use super::{CommandStep, Settings};
use crate::{CiEnvironment, GateRecipe, GroupRecipe, Package};
use foundation_tasks::TaskError;
use std::path::Path;

fn flags(spec: &str) -> Result<Vec<String>, TaskError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(Vec::new());
    }
    if spec.starts_with('-') {
        return shlex::split(spec)
            .ok_or_else(|| TaskError::operation(format!("invalid feature flags: {spec}")));
    }
    Ok(vec![
        "--features".into(),
        spec.split_whitespace().collect::<Vec<_>>().join(","),
    ])
}
fn cargo(args: &[&str]) -> CommandStep {
    CommandStep::new("cargo", args.iter().copied())
}
fn test(name: &str, profile: &str, run: &str, args: &[String]) -> Result<CommandStep, TaskError> {
    let mut cmd = cargo(&["test", "--profile", profile, "-p", name, "--no-fail-fast"]);
    cmd.cmd = cmd
        .cmd
        .args(flags(run)?)
        .args(["--", "--nocapture"])
        .args(args.iter().cloned());
    Ok(cmd)
}
fn wasm_runner(
    profile: &str,
    features: &str,
    flags: &[&str],
    default_features: bool,
) -> CommandStep {
    let mut c = cargo(&["run", "-p", "foundation_testbed"]);
    if !default_features {
        c.cmd = c.cmd.arg("--no-default-features");
    }
    c.cmd = c
        .cmd
        .args([
            "--features",
            features,
            "--profile",
            profile,
            "--bin",
            "wasm-testbed",
            "--",
        ])
        .args(flags.iter().copied());
    c
}
fn browser_cases(profile: &str, directory: &str) -> Vec<CommandStep> {
    vec![
        wasm_runner(
            profile,
            "wasm",
            &[
                "browser",
                "backends/foundation_testbed/integration/module",
                "--filter",
                "passes",
                "--headless",
            ],
            false,
        ),
        wasm_runner(
            profile,
            "wasm",
            &["browser", directory, "--headless"],
            false,
        ),
    ]
}
fn wasm_build(directory: &str) -> Vec<CommandStep> {
    let mut out = Vec::new();
    for (pkg, feature, tests) in [
        ("foundation_core", "std,js-wasmbindgen", true),
        ("foundation_netio", "wasm-fetch", true),
        ("foundation_wireguard", "wasm-bindgen-test", true),
        ("foundation_http", "wasm-bindgen-http", false),
        ("foundation_connectrpc", "rpc", false),
    ] {
        let mut c = cargo(&["check"]);
        if tests {
            c.cmd = c.cmd.arg("--tests");
        }
        c.cmd = c.cmd.args([
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            pkg,
            "--no-default-features",
            "--features",
            feature,
        ]);
        out.push(c);
    }
    let manifest = format!("{directory}/Cargo.toml");
    for manifest in [
        manifest.as_str(),
        "backends/foundation_db/integration/wasm_cf/Cargo.toml",
        "backends/foundation_testbed/integration/bindgen_failing_case/Cargo.toml",
    ] {
        out.push(cargo(&[
            "check",
            "--tests",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            manifest,
        ]));
        out.push(cargo(&["fmt", "--manifest-path", manifest, "--check"]));
    }
    out
}
fn wasm_suite(
    profile: &str,
    step: &str,
    os: &str,
    host: &CiEnvironment,
    directory: &str,
) -> Vec<CommandStep> {
    if matches!(os, "macos" | "windows") {
        return if os == "macos" && step == "test" {
            browser_cases(profile, directory)
        } else {
            Vec::new()
        };
    }
    let mut out = Vec::new();
    if step == "build" {
        return wasm_build(directory);
    } else if step == "test" {
        for (path, pass_only) in [
            ("backends/foundation_testbed/integration/module", true),
            (directory, false),
        ] {
            let mut c = wasm_runner(profile, "wasm-embedded-js", &["deno", path], false);
            if pass_only {
                c.cmd = c.cmd.args(["--filter", "passes"]);
            }
            out.push(c);
        }
        out.push(CommandStep::new(
            "mise",
            [
                "install",
                "aqua:denoland/deno",
                "github:wasm-bindgen/wasm-bindgen",
            ],
        ));
        for filter in ["wasm_js_yield_integration", "async_first_api"] {
            out.push(wasm_runner(
                profile,
                "wasm",
                &[
                    "test",
                    "bindgen-deno",
                    "./backends/foundation_core",
                    "--features",
                    "js-wasmbindgen,std",
                    "--filter",
                    filter,
                ],
                true,
            ));
        }
        out.push(wasm_runner(
            profile,
            "wasm",
            &[
                "test",
                "bindgen-wrangler",
                "backends/foundation_db/integration/wasm_cf",
            ],
            false,
        ));
        out.push(CommandStep::new(
            host.binary.as_os_str(),
            ["task", "run", "ci:prepare-chromium"],
        ));
        out.extend(browser_cases(profile, directory));
        out.push(wasm_runner(
            profile,
            "wasm",
            &[
                "test",
                "bindgen-web",
                "./backends/foundation_core",
                "--features",
                "js-wasmbindgen,std",
                "--headless",
            ],
            true,
        ));
        for name in ["bindgen-deno", "bindgen-web"] {
            let mut c = wasm_runner(
                profile,
                "wasm",
                &[
                    "test",
                    name,
                    "backends/foundation_testbed/integration/bindgen_failing_case",
                    "--filter",
                    "b_fails",
                ],
                true,
            );
            if name == "bindgen-web" {
                c.cmd = c.cmd.arg("--headless");
            }
            c.expected_failure = Some(String::new());
            out.push(c);
        }
    }
    out
}
fn example_commands(
    root: &Path,
    pkg: &Package,
    profile: &str,
    step: &str,
) -> Result<Vec<CommandStep>, TaskError> {
    let mut out = Vec::new();
    if step == "build" {
        let mut manifests = Vec::new();
        for entry in root
            .join(&pkg.dir)
            .read_dir()
            .map_err(TaskError::operation)?
        {
            let entry = entry.map_err(TaskError::operation)?;
            let manifest = entry.path().join("Cargo.toml");
            if entry.file_name().to_string_lossy().starts_with("cf-") && manifest.is_file() {
                manifests.push(manifest);
            }
        }
        manifests.sort();
        for path in manifests {
            let mut c = cargo(&[
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "--manifest-path",
            ]);
            c.cmd = c.cmd.arg(path);
            out.push(c);
        }
        for member in &pkg.build_members {
            out.push(cargo(&["build", "--profile", profile, "-p", member]));
        }
    }
    Ok(out)
}
fn gate_commands(recipe: GateRecipe, profile: &str, step: &str) -> Vec<CommandStep> {
    if step != "test" {
        return Vec::new();
    }
    match recipe {
        GateRecipe::DurableConformance => vec![
            // The reference server and client suites, and the counts the gate
            // requires.
            CommandStep::new("bash", ["ci/durable_conformance.sh"]),
            // The cross-implementation directions: our client against our
            // server, and the reference Rust client against our server.
            cargo(&[
                "test",
                "--profile",
                profile,
                "-p",
                "foundation_http",
                "--features",
                "multi",
                "--test",
                "durable_cross_client",
                "--test",
                "durable_reference_cross_run",
            ]),
        ],
    }
}

pub(super) fn commands(
    root: &Path,
    pkg: &Package,
    s: &Settings,
    profile: &str,
    os: &str,
    step: &str,
    host: &CiEnvironment,
) -> Result<Vec<CommandStep>, TaskError> {
    let name = &pkg.name;
    let mut out = Vec::new();
    if let Some(recipe) = pkg.gate {
        return Ok(gate_commands(recipe, profile, step));
    }
    if pkg.recipe == Some(GroupRecipe::WasmRuntimeTests) {
        return Ok(wasm_suite(profile, step, os, host, &pkg.dir));
    }
    if pkg.recipe == Some(GroupRecipe::ManifestContract) {
        if step == "build" {
            let python = if os == "windows" { "python" } else { "python3" };
            for flag in ["--check-schema", "--check-copies"] {
                out.push(CommandStep::new(
                    python,
                    ["scripts/ci/manifest_contract.py", flag],
                ));
            }
        }
        return Ok(out);
    }
    if matches!(os, "macos" | "windows") {
        if step == "test" {
            for run in s.platform_runs(os).unwrap_or(&[]) {
                out.push(test(name, profile, run, &s.test_args)?);
            }
        }
        return Ok(out);
    }
    if pkg.recipe == Some(GroupRecipe::ExampleApplications) {
        return example_commands(root, pkg, profile, step);
    }
    match step {
        "fmt" => out.push(cargo(&["fmt", "-p", name, "--check"])),
        "build" => {
            if name == "foundation_core" && s.build == "enforce" {
                for guard in [
                    "ci:check-executor-wake",
                    "ci:check-time-sources",
                    "ci:check-clock-params",
                ] {
                    let mut cmd = CommandStep::new(host.binary.as_os_str(), ["task", "run", guard]);
                    cmd.cmd = cmd.cmd.arg("--check");
                    out.push(cmd);
                }
                out.push(CommandStep::new(
                    if os == "windows" { "python" } else { "python3" },
                    ["-m", "unittest", "discover", "-s", "scripts/ci/tests"],
                ));
            }
            let mut c = cargo(&["build", "--profile", profile, "-p", name, "--all-targets"]);
            c.cmd = c.cmd.args(flags(&s.runs[0])?);
            out.push(c);
        }
        "clippy" => {
            let runs = if s.clippy_runs.is_empty() {
                &s.runs[..1]
            } else {
                &s.clippy_runs[..]
            };
            for run in runs {
                let mut c = cargo(&["clippy", "--profile", profile, "-p", name, "--all-targets"]);
                c.cmd = c.cmd.args(flags(run)?);
                if s.clippy_deny_warnings {
                    c.cmd = c.cmd.args(["--no-deps", "--", "-D", "warnings"]);
                }
                out.push(c);
            }
        }
        "test" => {
            for run in &s.runs {
                out.push(test(name, profile, run, &s.test_args)?);
            }
        }
        "wasm" => {
            for (run, expected) in s.wasm_runs.iter().map(|r| (r, None)).chain(
                s.wasm_expect_error
                    .iter()
                    .map(|e| (&e.run, Some(e.contains.clone()))),
            ) {
                let mut c = cargo(&["check", "--target", "wasm32-unknown-unknown", "-p", name]);
                c.cmd = c.cmd.args(flags(run)?);
                c.deny = s.wasm_deny_warnings;
                c.expected_failure = expected;
                out.push(c);
            }
        }
        _ => return Err(TaskError::operation(format!("unknown check step {step}"))),
    }
    Ok(out)
}
