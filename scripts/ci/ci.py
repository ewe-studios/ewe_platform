#!/usr/bin/env python3
"""Per-package CI driver for the ewe_platform workspace.

GitHub Actions and local runs use this one script, so what CI does is never a
mystery:

    python3 scripts/ci/ci.py list                     # every package + its CI settings
    python3 scripts/ci/ci.py plan                     # GitHub matrix for the current event
    python3 scripts/ci/ci.py run foundation_ai        # fmt + clippy + tests for one package
    python3 scripts/ci/ci.py run foundation_ai --step test

On a macOS host (`sys.platform == "darwin"`) `run` does only the test step.
Every package is tested on macOS by default, with the feature sets of its `runs`
key minus the sets that name a feature in `linux-only-features`. The
`macos-runs` key replaces that default. A package opts out with
`macos-tests = "none"` and a reason. `plan` also writes the `macos-matrix`
output. A Windows host (`sys.platform == "win32"`) works the same way with the
`windows-runs` / `windows-tests` keys and the `windows-matrix` output.

`plan` also runs a guard. It fails when an opt-out has no reason, and when a
`macos-runs` / `windows-runs` entry names a feature the package does not have.
See `platform_guard`.

Per-package settings live in `ci/packages.toml`. Everything else (the package
list, which packages need the llama.cpp submodule, who depends on whom) comes
from `cargo metadata`, so a new crate is picked up without editing CI.

Standard library only (Python 3.11+ for `tomllib`).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG_PATH = ROOT / "ci" / "packages.toml"
IN_GITHUB = bool(os.environ.get("GITHUB_ACTIONS"))


def target_dir() -> str:
    """The cargo target directory for this worktree.

    Parallel worktrees must not share one ``CARGO_TARGET_DIR``. They build
    crates with the same hashes and overwrite each other's artifacts mid-run.
    Use ``<repo>/target`` by default. When ``CARGO_TARGET_DIR`` is set, use
    ``<CARGO_TARGET_DIR>/<checkout-name>`` instead, where ``<checkout-name>``
    is the basename of the repository root. Each worktree has its own checkout
    name, so its target directory stays separate.
    """
    base = os.environ.get("CARGO_TARGET_DIR")
    if base:
        return str(Path(base) / ROOT.name)
    return str(ROOT / "target")

# Crates whose build compiles the `tools/llama.cpp` submodule.
LLAMA_CRATES = {"infrastructure_llama_bindings", "infrastructure_llama_cpp"}

# A change under any of these can affect every package, so on a pull request it
# selects the whole workspace. Entries ending in "/" are directory prefixes.
GLOBAL_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "build.rs",
    ".cargo/",
    "ci/",
    "scripts/ci/",
    ".github/workflows/check.yaml",
    # Inputs of the CI image every package job runs in (ci/ewe-platform-image):
    # tool pins, submodules (a gitlink bump under tools/ changes the image),
    # and the workflow that builds it.
    "mise.toml",
    "mise.lock",
    ".gitmodules",
    "tools/",
    ".github/workflows/platform-images.yaml",
)

STEPS = ("fmt", "build", "clippy", "test", "wasm")
MODES = ("enforce", "report", "off")
PLATFORMS = ("linux", "macos", "windows")


def host_platform() -> str:
    """The platform this script runs on: "macos", "windows" or "linux".

    One `sys.platform` check is enough. The macOS and Windows jobs run this
    script on the runner itself, with no container, so no flag is needed.
    """
    if sys.platform == "darwin":
        return "macos"
    if sys.platform == "win32":
        return "windows"
    return "linux"



# ---------------------------------------------------------------------------
# Workspace model


@dataclass
class Package:
    name: str
    dir: str  # relative to the workspace root
    needs_llama: bool
    dependents: set[str] = field(default_factory=set)


@dataclass
class Settings:
    """CI settings for one package: `[defaults]` merged with `[packages.<name>]`."""

    skip: str | None
    fmt: str
    build: str
    clippy: str
    test: str
    runs: list[str]  # one `cargo test` per entry; each entry is its feature flags
    clippy_runs: list[str]  # one `cargo clippy` per entry; empty means `[runs[0]]`
    # True: every clippy run of the package gets `--no-deps -- -D warnings`, so a clippy
    # warning fails CI. Set it only for packages already at zero warnings.
    clippy_deny_warnings: bool
    wasm: str  # wasm32-unknown-unknown compile check mode
    wasm_runs: list[str]  # one `cargo check --target wasm32` per entry; feature flags
    # True: every wasm run of the package gets `RUSTFLAGS=-D warnings`, so a
    # warning fails the wasm gate. `cargo check` takes no trailing rustc args,
    # so the flag travels in the environment of that command only.
    wasm_deny_warnings: bool
    test_args: list[str]
    timeout_minutes: int
    # Most failure lines kept per command for the end-of-run summary. The
    # `failure-lines` setting; the default is 200.
    failure_lines: int
    reason: str | None
    # Extra environment for this package's cargo commands (the `env` table).
    env: dict[str, str] = field(default_factory=dict)
    # Extra Docker options for this package's `container:` job. Only the io_uring
    # packages set it: Docker's default seccomp profile blocks `io_uring_setup`.
    container_options: str = ""
    # A wasm32 build that must fail with a named compile error, as a table with
    # `run` (feature flags) and `contains` (the message). Checks the documented
    # minimum feature set instead of dropping the configuration.
    wasm_expect_error: dict | None = None
    # Feature sets for the macOS test job, in the same syntax as `runs`. The
    # default is `runs` minus the sets that name a `linux-only-features` entry.
    # The `macos-runs` key replaces it. None means the package is not tested on
    # macOS: it opted out, or every run is Linux-only.
    macos_runs: list[str] | None = None
    # Environment for the macOS test commands. The `macos-env` key replaces it.
    # Without the key it is `env` minus the Linux-only `EWE_REQUIRE_*` variables.
    macos_env: dict[str, str] = field(default_factory=dict)
    # The same keys for the Windows test job (`windows-runs`, `windows-env`).
    windows_runs: list[str] | None = None
    windows_env: dict[str, str] = field(default_factory=dict)
    # `macos-tests = "none"` / `windows-tests = "none"`: the package cannot be
    # tested on that platform. The value is the reason text; an empty string is
    # an opt-out without a reason, which `platform_guard` rejects.
    macos_opt_out: str | None = None
    windows_opt_out: str | None = None
    # The `macos-runs` / `windows-runs` keys as written, for the guard. None
    # means the default applies.
    macos_runs_override: list[str] | None = None
    windows_runs_override: list[str] | None = None

    def platform_runs(self, platform: str) -> list[str] | None:
        """The feature sets of the test job for `platform`, or None."""
        return {"macos": self.macos_runs, "windows": self.windows_runs}.get(platform)

    def platform_env(self, platform: str) -> dict[str, str]:
        """The environment for `platform`; the Linux `env` otherwise."""
        if platform == "macos":
            return self.macos_env
        if platform == "windows":
            return self.windows_env
        return self.env


@dataclass
class PlannedCommand:
    """One planned command: the argv and extra environment for just this run.

    Most planned commands are plain argv lists. A command that needs a variable
    for itself (the wasm deny gate sets `RUSTFLAGS`) carries it here, so the
    package-wide `env` stays unchanged.
    """

    argv: list[str]
    env: dict[str, str] = field(default_factory=dict)


def cargo_metadata() -> dict:
    try:
        out = subprocess.run(
            ["cargo", "metadata", "--format-version", "1"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except subprocess.CalledProcessError as err:
        # Without this the traceback names only the exit code. The reason
        # (missing submodule, unparsable manifest, offline registry) is in
        # stderr, and CI captured it away.
        raise SystemExit(
            f"cargo metadata failed with exit {err.returncode}:\n{err.stderr}"
        ) from err
    return json.loads(out)


def load_workspace() -> dict[str, Package]:
    meta = cargo_metadata()
    members = set(meta["workspace_members"])
    by_id = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

    def closure(pid: str) -> set[str]:
        seen: set[str] = set()
        stack = [pid]
        while stack:
            for dep in nodes[stack.pop()]["deps"]:
                if dep["pkg"] not in seen:
                    seen.add(dep["pkg"])
                    stack.append(dep["pkg"])
        return seen

    closures = {pid: closure(pid) for pid in members}
    packages: dict[str, Package] = {}
    for pid in members:
        meta_pkg = by_id[pid]
        dep_names = {by_id[d]["name"] for d in closures[pid]}
        packages[meta_pkg["name"]] = Package(
            name=meta_pkg["name"],
            dir=Path(meta_pkg["manifest_path"]).parent.relative_to(ROOT).as_posix(),
            needs_llama=bool((dep_names | {meta_pkg["name"]}) & LLAMA_CRATES),
        )

    # Reverse edges between workspace members: who is affected by a change.
    for pid in members:
        name = by_id[pid]["name"]
        for dep in closures[pid]:
            dep_name = by_id[dep]["name"]
            if dep_name in packages and dep_name != name:
                packages[dep_name].dependents.add(name)

    # The `examples/` directory holds self-contained workspaces (excluded from
    # this workspace), so cargo metadata never lists them. Add one synthetic
    # package that owns that directory; its build step (see `examples_commands`)
    # compiles every `examples/cf-*` to wasm32 and the native `answerme-agent`.
    # It builds answerme-agent, so it inherits that app's llama.cpp need.
    packages["examples"] = Package(
        name="examples",
        dir="examples",
        needs_llama=packages["answerme-agent"].needs_llama,
    )

    # The wasm runtime test suites live in wasm32-only test targets and in a
    # self-contained crate outside the root workspace, so cargo metadata's
    # library-only view never exercises them. Add one synthetic package whose
    # build step compiles those tests and whose test step runs the embedded-Deno
    # harnesses (feature 22 E4). See `wasm_tests_commands`.
    packages["wasm-tests"] = Package(
        name="wasm-tests",
        dir="backends/foundation_core/integration/wasm_tests",
        needs_llama=False,
    )
    return packages


def load_settings(packages: dict[str, Package]) -> dict[str, Settings]:
    config = tomllib.loads(CONFIG_PATH.read_text())
    defaults = config.get("defaults", {})
    overrides = config.get("packages", {})

    unknown = sorted(set(overrides) - set(packages))
    if unknown:
        raise SystemExit(f"ci/packages.toml names packages not in the workspace: {unknown}")

    return {
        name: settings_for(name, defaults, overrides.get(name, {}))
        for name in packages
    }


def run_features(run: str) -> list[str]:
    """The feature names one `runs` entry turns on.

    An entry is "" (defaults), "a b" or "a,b" (feature names), or raw cargo flags
    that start with "-" (`--features a,b` / `-F a`). Other flags name no feature.
    """
    run = run.strip()
    if not run.startswith("-"):
        return [f for f in run.replace(",", " ").split() if f]
    tokens, found, i = run.split(), [], 0
    while i < len(tokens):
        tok = tokens[i]
        if tok in ("--features", "-F") and i + 1 < len(tokens):
            found += tokens[i + 1].split(",")
            i += 1
        elif tok.startswith("--features="):
            found += tok.split("=", 1)[1].split(",")
        i += 1
    return [f for f in found if f]


def run_is_linux_only(run: str, linux_only: list[str]) -> bool:
    """True when the entry turns on any feature in `linux-only-features`."""
    return any(f in linux_only for f in run_features(run))


def max_parallel() -> dict[str, int]:
    """The most jobs of one matrix that run at once, per OS (`[defaults]` keys).

    Limits are configuration (CLAUDE.md rule 8). The defaults match
    ci/packages.toml: free GitHub plans run about 20 jobs at once, with at most
    5 macOS jobs, so the macOS and Windows defaults stay small.
    """
    defaults = tomllib.loads(CONFIG_PATH.read_text()).get("defaults", {})
    result = {}
    for plat, fallback in (("linux", 8), ("macos", 2), ("windows", 2)):
        value = defaults.get(f"max-parallel-{plat}", fallback)
        if not isinstance(value, int) or isinstance(value, bool) or value < 1:
            raise SystemExit(f"[defaults]: `max-parallel-{plat}` must be a positive integer")
        result[plat] = value
    return result


def settings_for(name: str, defaults: dict, package: dict) -> Settings:
    """Merge `[defaults]` with one `[packages.<name>]` table and validate it."""
    merged = {**defaults, **package}
    # `env` merges key by key, so a package adds to [defaults.env] rather
    # than replacing it.
    for table, where in ((defaults, "[defaults]"), (package, f"[packages.{name}]")):
        if not isinstance(table.get("env", {}), dict):
            raise SystemExit(f"{where}: `env` must be a table of strings")
    env = {**defaults.get("env", {}), **package.get("env", {})}
    for key, value in env.items():
        if not isinstance(value, str):
            raise SystemExit(f"{name}: env.{key} must be a string, got {value!r}")
    container_options = merged.get("container-options", "")
    if not isinstance(container_options, str):
        raise SystemExit(f"{name}: `container-options` must be a string")
    wasm_expect_error = merged.get("wasm-expect-error")
    if wasm_expect_error is not None:
        if not isinstance(wasm_expect_error, dict):
            raise SystemExit(f"{name}: `wasm-expect-error` must be a table")
        for key in ("run", "contains"):
            if not isinstance(wasm_expect_error.get(key), str):
                raise SystemExit(f"{name}: wasm-expect-error.{key} must be a string")
    linux_only = defaults.get("linux-only-features", [])
    if not isinstance(linux_only, list) or not all(isinstance(f, str) for f in linux_only):
        raise SystemExit("[defaults]: `linux-only-features` must be a list of strings")
    runs = list(merged.get("runs", [""])) or [""]
    platform_keys: dict[str, tuple] = {}
    for plat in ("macos", "windows"):
        runs_key = package.get(f"{plat}-runs")
        if runs_key is not None:
            if (
                not isinstance(runs_key, list)
                or not runs_key
                or not all(isinstance(r, str) for r in runs_key)
            ):
                raise SystemExit(f"{name}: `{plat}-runs` must be a non-empty list of strings")
        env_key = package.get(f"{plat}-env")
        if env_key is not None and (
            not isinstance(env_key, dict) or not all(isinstance(v, str) for v in env_key.values())
        ):
            raise SystemExit(f"{name}: `{plat}-env` must be a table of strings")
        opt_out = None
        if f"{plat}-tests" in package:
            if package[f"{plat}-tests"] != "none":
                raise SystemExit(f'{name}: `{plat}-tests` must be "none"')
            if runs_key is not None:
                raise SystemExit(f"{name}: `{plat}-tests` and `{plat}-runs` exclude each other")
            # A missing reason is not an error here: `platform_guard` reports it.
            opt_out = str(package.get(f"{plat}-reason") or package.get("reason") or "")
        # The default runs: the Linux list minus the Linux-only feature sets.
        if opt_out is not None:
            effective = None
        elif runs_key is not None:
            effective = list(runs_key)
        else:
            effective = [r for r in runs if not run_is_linux_only(r, linux_only)] or None
        effective_env = (
            dict(env_key)
            if env_key is not None
            else {k: v for k, v in env.items() if not k.startswith("EWE_REQUIRE_")}
        )
        platform_keys[plat] = (effective, effective_env, opt_out, runs_key)
    macos_runs, macos_env, macos_opt_out, macos_override = platform_keys["macos"]
    windows_runs, windows_env, windows_opt_out, windows_override = platform_keys["windows"]
    s = Settings(
        skip=merged.get("skip"),
        fmt=merged.get("fmt", "report"),
        build=merged.get("build", "enforce"),
        clippy=merged.get("clippy", "enforce"),
        test=merged.get("test", "enforce"),
        runs=list(merged.get("runs", [""])) or [""],
        clippy_runs=list(merged.get("clippy-runs", [])),
        clippy_deny_warnings=bool(merged.get("clippy-deny-warnings", False)),
        wasm=merged.get("wasm", "off"),
        wasm_runs=list(merged.get("wasm-runs", [""])) or [""],
        wasm_deny_warnings=bool(merged.get("wasm-deny-warnings", False)),
        test_args=list(merged.get("test-args", [])),
        timeout_minutes=int(merged.get("timeout-minutes", 60)),
        # Default 200 lines: enough for many failed tests, small for a log tail.
        failure_lines=int(merged.get("failure-lines", 200)),
        reason=merged.get("reason"),
        env=env,
        container_options=container_options,
        wasm_expect_error=wasm_expect_error,
        macos_runs=list(macos_runs) if macos_runs is not None else None,
        macos_env=dict(macos_env),
        windows_runs=list(windows_runs) if windows_runs is not None else None,
        windows_env=dict(windows_env),
        macos_opt_out=macos_opt_out,
        windows_opt_out=windows_opt_out,
        macos_runs_override=list(macos_override) if macos_override is not None else None,
        windows_runs_override=list(windows_override) if windows_override is not None else None,
    )
    for step in STEPS:
        if getattr(s, step) not in MODES:
            raise SystemExit(f"{name}: `{step}` must be one of {MODES}, got {getattr(s, step)!r}")
    return s


# ---------------------------------------------------------------------------
# Selection


def changed_files(base: str, head: str) -> list[str]:
    out = subprocess.run(
        ["git", "diff", "--name-only", f"{base}...{head}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return [line for line in out.splitlines() if line]


def is_global(path: str) -> bool:
    return any(path.startswith(g) if g.endswith("/") else path == g for g in GLOBAL_PATHS)


def owning_package(path: str, packages: dict[str, Package]) -> str | None:
    """The package whose directory contains `path` (the deepest one wins)."""
    best: str | None = None
    for pkg in packages.values():
        if pkg.dir in ("", ".") or not path.startswith(pkg.dir + "/"):
            continue
        if best is None or len(pkg.dir) > len(packages[best].dir):
            best = pkg.name
    return best


def affected(files: list[str], packages: dict[str, Package]) -> tuple[set[str], str]:
    """The packages a change touches, plus every workspace package that depends on them."""
    files = [f for f in files if not f.endswith(".md")]  # prose doesn't change what code does
    if any(is_global(f) for f in files):
        return set(packages), "a workspace-wide file changed"

    direct: set[str] = set()
    for path in files:
        if owner := owning_package(path, packages):
            direct.add(owner)

    selected = set(direct)
    for name in direct:
        selected |= packages[name].dependents
    return selected, f"{len(direct)} changed package(s) plus dependents"


def select(args: argparse.Namespace, packages: dict[str, Package]) -> tuple[set[str], str]:
    requested = (args.packages or os.environ.get("CI_PACKAGES") or "").strip()
    if requested and requested != "all":
        names = {p for p in requested.replace(",", " ").split() if p}
        missing = sorted(names - set(packages))
        if missing:
            raise SystemExit(f"unknown package(s): {missing}")
        return names, "requested explicitly"
    if requested == "all":
        return set(packages), "all packages requested"

    base = args.base or os.environ.get("CI_BASE_SHA", "")
    head = args.head or os.environ.get("CI_HEAD_SHA", "") or "HEAD"
    if base:
        return affected(changed_files(base, head), packages)
    return set(packages), "full run"


# ---------------------------------------------------------------------------
# Running one package


def feature_flags(spec: str) -> list[str]:
    """`""` → default features; `"a b"` → `--features a,b`; anything starting
    with `-` is passed through verbatim (e.g. `--no-default-features --features x`)."""
    spec = spec.strip()
    if not spec:
        return []
    if spec.startswith("-"):
        return shlex.split(spec)
    return ["--features", ",".join(spec.split())]


def examples_commands(profile: str) -> dict[str, list[list[str]]]:
    """Build commands for the synthetic `examples` package.

    The `examples/cf-*` directories are self-contained wasm32 workspaces (the
    workspace `[workspace] exclude`s them), so they are compiled directly by
    manifest rather than by `-p`. The `answerme-agent` app is a workspace
    member and gets a normal native build.
    """
    plan: dict[str, list[list[str]]] = {step: [] for step in STEPS}
    for manifest in sorted((ROOT / "examples").glob("cf-*/Cargo.toml")):
        plan["build"].append(
            ["cargo", "build", "--target", "wasm32-unknown-unknown", "--manifest-path", manifest.as_posix()]
        )
    plan["build"].append(["cargo", "build", "--profile", profile, "-p", "answerme-agent"])
    return plan


def wasm_tests_commands() -> dict[str, list[list[str]]]:
    """Commands for the synthetic `wasm-tests` package (feature 22 E4).

    (1) Compile every crate with wasm tests, plus the standalone wasm_tests
    crate outside the root workspace, with `--tests --target wasm32` so the
    test code itself is checked (the default wasm gate checks libraries only,
    which let the `wasm_bindgen_test` suites rot). (2) Run the two embedded-Deno
    runtime suites. `MISE_ENV=wasm` (set in `ci/packages.toml`) selects
    `mise.wasm.toml`, which pins only deno + wasm-bindgen and turns off
    auto_install.
    """
    plan: dict[str, list[list[str]]] = {step: [] for step in STEPS}
    plan["build"] = [
        ["cargo", "check", "--tests", "--target", "wasm32-unknown-unknown",
         "-p", "foundation_core", *feature_flags("--no-default-features --features std,js-wasmbindgen")],
        ["cargo", "check", "--tests", "--target", "wasm32-unknown-unknown",
         "-p", "foundation_netio", *feature_flags("--no-default-features --features wasm-fetch")],
        ["cargo", "check", "--tests", "--target", "wasm32-unknown-unknown",
         "-p", "foundation_wireguard", *feature_flags("--no-default-features --features wasm-bindgen-test")],
        ["cargo", "check", "--tests", "--target", "wasm32-unknown-unknown",
         "--manifest-path", "backends/foundation_core/integration/wasm_tests/Cargo.toml"],
        # Spec 70 feature 76: the standalone Cloudflare wasm store tests. The
        # wrangler run below compiles them too, but this step names a compile
        # break directly, before the harness starts.
        ["cargo", "check", "--tests", "--target", "wasm32-unknown-unknown",
         "--manifest-path", "backends/foundation_db/integration/wasm_cf/Cargo.toml"],
        # This package sets fmt = "off" (the workspace fmt step cannot see a
        # crate outside the workspace), so the standalone crate's formatting
        # is checked here.
        ["cargo", "fmt", "--manifest-path",
         "backends/foundation_db/integration/wasm_cf/Cargo.toml", "--check"],
        ["cargo", "fmt", "--manifest-path",
         "backends/foundation_core/integration/wasm_tests/Cargo.toml", "--check"],
    ]
    plan["test"] = [
        ["mise", "run", "test:wasm-testbed:deno"],
        ["mise", "run", "test:wasm-bindgen:deno"],
        # Spec 70 feature 76: the D1/R2/KV wasm stores on the real local
        # bindings (`wrangler dev --local`). The REST clients use the same
        # local D1/R2 state through the explorer API.
        ["mise", "run", "test:wasm-testbed:wrangler"],
    ]
    return plan


def test_command(name: str, run: str, profile: str, test_args: list[str]) -> list[str]:
    """One `cargo test` for one feature set."""
    cmd = ["cargo", "test", "--profile", profile, "-p", name, "--no-fail-fast", *feature_flags(run)]
    # `--nocapture` prints every test's output, so a failing or hanging
    # test leaves its own trace instead of a bare "FAILED" line.
    return cmd + ["--", "--nocapture", *test_args]


def commands(
    name: str, s: Settings, profile: str, platform: str = "linux"
) -> dict[str, list[list[str] | PlannedCommand]]:
    if platform in ("macos", "windows"):
        # The macOS and Windows jobs run tests only. Format, clippy and the wasm
        # gate stay on Linux. `cargo test` builds the targets, so no build step.
        host_plan: dict[str, list[list[str] | PlannedCommand]] = {step: [] for step in STEPS}
        host_runs = s.platform_runs(platform)
        if host_runs and s.test != "off":
            for run in host_runs:
                host_plan["test"].append(test_command(name, run, profile, s.test_args))
        return host_plan
    if name == "examples":
        return examples_commands(profile)
    if name == "wasm-tests":
        return wasm_tests_commands()
    plan: dict[str, list[list[str] | PlannedCommand]] = {step: [] for step in STEPS}
    if s.fmt != "off":
        plan["fmt"].append(["cargo", "fmt", "-p", name, "--check"])
    if name == "foundation_core" and s.build == "enforce":
        # Spec 70 feature 80: the executor wake functions are private to valtron.
        # One repository-wide scan, run with the package that owns them (in the
        # enforced `build` step), and the test of the scan itself.
        plan["build"].append([sys.executable, "scripts/ci/check_executor_wake.py"])
        # Spec 70 feature 117: a crate reads the time from the injected `Clock`,
        # never from the global. The allowlist names every remaining offender.
        plan["build"].append(
            [sys.executable, "scripts/ci/check_time_sources.py", "--check"]
        )
        plan["build"].append([sys.executable, "-m", "unittest", "discover", "-s", "scripts/ci/tests"])
    if s.build != "off":
        # Library, binaries, tests, examples and benches: compile errors show
        # up here, separately from test failures.
        plan["build"].append(
            ["cargo", "build", "--profile", profile, "-p", name, "--all-targets", *feature_flags(s.runs[0])]
        )
    if s.clippy != "off":
        for run in s.clippy_runs or [s.runs[0]]:
            cmd = ["cargo", "clippy", "--profile", profile, "-p", name, "--all-targets", *feature_flags(run)]
            if s.clippy_deny_warnings:
                cmd += ["--no-deps", "--", "-D", "warnings"]
            plan["clippy"].append(cmd)
    if s.test != "off":
        for run in s.runs:
            plan["test"].append(test_command(name, run, profile, s.test_args))
    if s.wasm != "off":
        # A wasm32 compile gate: no `--all-targets`/profile (wasm has no test
        # binaries or uat tuning), just `cargo check` on the wasm-relevant
        # feature set. Catches the `multi`-into-`!Send`-wasm leak and other
        # native-dep regressions that native-only builds never see.
        wasm_commands: list[list[str]] = []
        for run in s.wasm_runs:
            wasm_commands.append(
                ["cargo", "check", "--target", "wasm32-unknown-unknown", "-p", name, *feature_flags(run)]
            )
        if s.wasm_expect_error:
            # The configuration is unsupported, and the crate refuses it with a
            # `compile_error!`. Require that exact failure, so the message and
            # the documented minimum cannot drift apart.
            expect = s.wasm_expect_error
            check = [
                "cargo",
                "check",
                "--target",
                "wasm32-unknown-unknown",
                "-p",
                name,
                *feature_flags(expect["run"]),
            ]
            wasm_commands.append(
                [sys.executable, "scripts/ci/expect_compile_error.py", expect["contains"], "--", *check]
            )
        for cmd in wasm_commands:
            if s.wasm_deny_warnings:
                # `cargo check` rejects trailing rustc args, so the deny flag
                # goes through `RUSTFLAGS` for this command only.
                plan["wasm"].append(PlannedCommand(cmd, {"RUSTFLAGS": "-D warnings"}))
            else:
                plan["wasm"].append(cmd)
    return plan


def display_command(cmd: list[str]) -> str:
    """The command as one line for the log. Display only: no shell runs it.

    Every command runs as an argument list, so no POSIX shell is involved. The
    quoting follows the host (`shlex` on POSIX, `list2cmdline` on Windows).
    """
    if os.name == "nt":
        return subprocess.list2cmdline(cmd)
    return shlex.join(cmd)


def annotate(level: str, message: str) -> None:
    print(f"::{level}::{message}" if IN_GITHUB else f"{level}: {message}", flush=True)


# Lines that show why a test run failed. Kept for the end-of-run summary.
FAILURE_LINE = re.compile(r"^test .* \.\.\. FAILED$|^---- .* stdout ----$|panicked at")


def run_streamed(
    cmd: list[str], env: dict[str, str], timeout: float, limit: int
) -> tuple[int, list[str]]:
    """Run `cmd`, stream its merged output, and keep the failure lines.

    Returns (exit code, kept lines). Raises `subprocess.TimeoutExpired` after it
    kills the process when `timeout` seconds pass. At most `limit` lines are kept.
    """
    proc = subprocess.Popen(
        cmd,
        cwd=ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        # Cargo prints UTF-8 on every host. The Windows default code page
        # (cp1252) would garble or reject it.
        encoding="utf-8",
        errors="replace",
        bufsize=1,
    )
    kept: list[str] = []
    timer = threading.Timer(timeout, proc.kill)
    timer.start()
    try:
        after_panic = False
        assert proc.stdout is not None
        for raw in proc.stdout:
            sys.stdout.write(raw)
            sys.stdout.flush()
            text = raw.rstrip("\n")
            if FAILURE_LINE.search(text) or after_panic:
                if len(kept) < limit:
                    kept.append(text)
            # The line after `panicked at` holds the panic message.
            after_panic = "panicked at" in text
        code = proc.wait()
    finally:
        expired = not timer.is_alive()
        timer.cancel()
        if proc.poll() is None:
            proc.kill()
            proc.wait()
    if expired:
        raise subprocess.TimeoutExpired(cmd, timeout)
    return code, kept


def run_package(name: str, steps: list[str], profile: str, platform: str | None = None) -> int:
    platform = platform or host_platform()
    packages = load_workspace()
    if name not in packages:
        raise SystemExit(f"unknown package: {name}")
    s = load_settings(packages)[name]
    if s.skip:
        print(f"skipping {name}: {s.skip}")
        return 0

    if platform != "linux" and s.platform_runs(platform) is None:
        label = {"macos": "macOS", "windows": "Windows"}[platform]
        opt_out = s.macos_opt_out if platform == "macos" else s.windows_opt_out
        why = f"it opted out: {opt_out}" if opt_out else "every run names a Linux-only feature"
        print(f"skipping {name}: not tested on {label}; {why}")
        return 0

    plan = commands(name, s, profile, platform)
    # The package's `env` applies to every step. A value already set in the
    # environment wins, so a bigger machine can still override it locally.
    # On macOS and Windows the package's `<platform>-env` replaces `env`.
    package_env = s.platform_env(platform)
    env = {**package_env, **os.environ}
    env["CARGO_TARGET_DIR"] = target_dir()
    if package_env:
        print(f"{name}: env " + " ".join(f"{k}={v}" for k, v in sorted(package_env.items())), flush=True)
    print(f"{name}: target dir {env['CARGO_TARGET_DIR']}", flush=True)
    # One message per failed or timed-out command. Used for the summary below.
    failures: list[str] = []
    # Kept failure lines per failure note, printed under it in the summary.
    details: dict[str, list[str]] = {}
    timed_out = False
    # One hard deadline for the whole package. GitHub gives the same job
    # `timeout-minutes`; a local run had no kill switch, so a step that looped
    # (a hung test, a deadlocked build) never returned. Each command borrows
    # the time still left, and a command that overruns is fatal for the package.
    deadline = time.monotonic() + s.timeout_minutes * 60
    for step in steps:
        mode = getattr(s, step)
        for entry in plan[step]:
            if isinstance(entry, PlannedCommand):
                cmd, cmd_env = entry.argv, entry.env
            else:
                cmd, cmd_env = entry, {}
            line = display_command(cmd)
            if cmd_env:
                # Show the command-only environment in the log, so a reader can
                # see the deny flag that the runner applies.
                line = " ".join(
                    f"{k}={display_command([v])}" for k, v in sorted(cmd_env.items())
                ) + " " + line
            print(f"::group::{step}: {line}" if IN_GITHUB else f"$ {line}", flush=True)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                if IN_GITHUB:
                    print("::endgroup::", flush=True)
                note = f"{name}: `{line}` timed out after {s.timeout_minutes} minutes"
                failures.append(note)
                annotate("error", note)
                timed_out = True
                break
            try:
                code, kept = run_streamed(
                    cmd, {**env, **cmd_env}, remaining, s.failure_lines
                )
            except subprocess.TimeoutExpired:
                if IN_GITHUB:
                    print("::endgroup::", flush=True)
                note = f"{name}: `{line}` timed out after {s.timeout_minutes} minutes"
                failures.append(note)
                annotate("error", note)
                timed_out = True
                break
            if IN_GITHUB:
                print("::endgroup::", flush=True)
            if code == 0:
                continue
            if mode == "enforce":
                note = f"{name}: `{line}` failed"
                failures.append(note)
                details[note] = kept
                annotate("error", note)
            else:
                why = f" — {s.reason}" if s.reason else ""
                annotate("warning", f"{name}: `{line}` failed (report-only{why})")
        if timed_out:
            # A timeout ends the package, as it always did. Skip later steps.
            break

    if failures:
        # Why this summary: the per-command error line can scroll out of the log
        # tail. GitHub's log API returns only the last 5000 lines. A later command
        # can print thousands of lines and push the error out of that window.
        # This block prints last, so every failed command stays visible.
        steps_text = ", ".join(steps)
        header = f"{name}: {len(failures)} failed command(s) in step {steps_text}"
        if IN_GITHUB:
            print(f"::group::{header}", flush=True)
        else:
            print(header, flush=True)
        for note in failures:
            print(note, flush=True)
            for kept_line in details.get(note, []):
                print(f"    {kept_line}", flush=True)
        if IN_GITHUB:
            print("::endgroup::", flush=True)
        annotate("error", f"{header}: " + "; ".join(failures))

    return 1 if failures else 0


# ---------------------------------------------------------------------------
# Platform guard
#
# Every package is tested on macOS and Windows unless it opts out. The guard
# keeps the opt-outs and overrides honest. It runs in `plan`, so every CI run
# checks it.


def cargo_features(package_dir: Path) -> set[str] | None:
    """The feature names a package's Cargo.toml offers, or None without a manifest.

    That is the `[features]` table plus every optional dependency, which cargo
    turns into an implicit feature of the same name.
    """
    manifest = package_dir / "Cargo.toml"
    if not manifest.is_file():
        return None
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError:
        return None  # a manifest that is a template, not TOML: cannot check
    names = set(data.get("features", {}))

    def collect(tables: dict) -> None:
        for dep, spec in tables.items():
            if isinstance(spec, dict) and spec.get("optional"):
                names.add(dep)

    for key in ("dependencies", "build-dependencies"):
        collect(data.get(key, {}))
    for target in data.get("target", {}).values():
        collect(target.get("dependencies", {}))
    return names


def platform_guard(
    packages: dict[str, Package], settings: dict[str, Settings], root: Path = ROOT
) -> list[str]:
    """One error message per broken `macos-*` / `windows-*` setting.

    Two checks:
    - an opt-out (`<platform>-tests = "none"`) has a reason;
    - a `<platform>-runs` entry names only features the package has.

    The synthetic packages (`examples`, `wasm-tests`) have no manifest of their
    own, so the feature check skips them. Skipped packages are not checked.
    """
    errors = []
    for name in sorted(packages):
        s = settings[name]
        if s.skip:
            continue
        features = None if name in ("examples", "wasm-tests") else cargo_features(root / packages[name].dir)
        for plat, label in (("macos", "macOS"), ("windows", "Windows")):
            opt_out = s.macos_opt_out if plat == "macos" else s.windows_opt_out
            override = s.macos_runs_override if plat == "macos" else s.windows_runs_override
            if opt_out is not None and not opt_out.strip():
                errors.append(
                    f"{name}: `{plat}-tests = \"none\"` has no reason. Add a `{plat}-reason` "
                    f"(or `reason`) in ci/packages.toml that names the missing tool, library or "
                    f"service on {label}."
                )
            if features is None or override is None:
                continue
            for run in override:
                for feature in run_features(run):
                    if feature.split("/", 1)[0].removeprefix("dep:") not in features:
                        errors.append(
                            f"{name}: `{plat}-runs` entry {run!r} names feature {feature!r}, "
                            f"which {packages[name].dir}/Cargo.toml does not define."
                        )
    return errors


# ---------------------------------------------------------------------------
# CLI


def cmd_list(_: argparse.Namespace) -> int:
    packages = load_workspace()
    settings = load_settings(packages)
    for name in sorted(packages):
        p, s = packages[name], settings[name]
        status = f"SKIP: {s.skip}" if s.skip else f"fmt={s.fmt} build={s.build} clippy={s.clippy}{'+deny' if s.clippy_deny_warnings else ''} test={s.test} wasm={s.wasm}"
        runs = ", ".join(repr(r) for r in s.runs)
        env = "".join(f"  {k}={v}" for k, v in sorted(s.env.items()))
        print(f"{name:36} {status:42} runs=[{runs}]{'  +llama.cpp' if p.needs_llama else ''}{env}")
    return 0


def cmd_plan(args: argparse.Namespace) -> int:
    packages = load_workspace()
    settings = load_settings(packages)
    if problems := platform_guard(packages, settings):
        for problem in problems:
            annotate("error", problem)
        return 1
    selected, why = select(args, packages)

    include = [
        {
            "package": name,
            "llama": packages[name].needs_llama,
            "timeout": settings[name].timeout_minutes,
            "container-options": settings[name].container_options,
        }
        for name in sorted(selected)
        if not settings[name].skip
    ]
    skipped = sorted(n for n in selected if settings[n].skip)
    matrix = json.dumps({"include": include})
    # The macOS and Windows test matrices: every selected package that has a
    # run for that OS, so every package except the opted-out ones. GitHub allows
    # 256 jobs per matrix; the workspace has about 110 packages, so it fits. If
    # the workspace ever passes 256 packages, split the matrix by package.
    def platform_include(plat: str) -> list[dict]:
        return [
            {"package": item["package"], "llama": item["llama"], "timeout": item["timeout"]}
            for item in include
            if settings[item["package"]].platform_runs(plat) is not None
            and settings[item["package"]].test != "off"
        ]

    macos_include = platform_include("macos")
    macos_matrix = json.dumps({"include": macos_include})
    windows_include = platform_include("windows")
    windows_matrix = json.dumps({"include": windows_include})
    summary = f"{len(include)} package(s) selected ({why})"
    print(summary, file=sys.stderr)

    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a") as fh:
            fh.write(f"matrix={matrix}\ncount={len(include)}\n")
            fh.write(f"macos-matrix={macos_matrix}\nmacos-count={len(macos_include)}\n")
            fh.write(f"windows-matrix={windows_matrix}\nwindows-count={len(windows_include)}\n")
            for plat, limit in max_parallel().items():
                fh.write(f"max-parallel-{plat}={limit}\n")
    if step_summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(step_summary, "a") as fh:
            fh.write(f"### CI plan\n\n{summary}\n\n")
            for item in include:
                fh.write(f"- `{item['package']}`{' (needs llama.cpp)' if item['llama'] else ''}\n")
            if macos_include:
                names = ", ".join(f"`{i['package']}`" for i in macos_include)
                fh.write(f"\nmacOS tests: {names}\n\n")
            if windows_include:
                names = ", ".join(f"`{i['package']}`" for i in windows_include)
                fh.write(f"\nWindows tests: {names}\n\n")
            for name in skipped:
                fh.write(f"- ~~`{name}`~~ skipped: {settings[name].skip}\n")
    print(matrix)
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    return run_package(args.package, args.step or list(STEPS), args.profile)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("list", help="show every package and its CI settings").set_defaults(func=cmd_list)

    plan = sub.add_parser("plan", help="emit the GitHub Actions matrix")
    plan.add_argument("--packages", help="comma/space separated package names, or 'all'")
    plan.add_argument("--base", help="base commit for change detection (pull requests)")
    plan.add_argument("--head", help="head commit for change detection")
    plan.set_defaults(func=cmd_plan)

    run = sub.add_parser("run", help="run fmt / clippy / tests for one package")
    run.add_argument("package")
    run.add_argument("--step", action="append", choices=STEPS, help="repeatable; default: all steps")
    run.add_argument("--profile", default=os.environ.get("CI_PROFILE", "uat"))
    run.set_defaults(func=cmd_run)

    # Cargo output and package names can hold non-ASCII text. The Windows
    # console code page cannot print it, so write UTF-8 on every host.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
