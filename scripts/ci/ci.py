#!/usr/bin/env python3
"""Per-package CI driver for the ewe_platform workspace.

GitHub Actions and local runs use this one script, so what CI does is never a
mystery:

    python3 scripts/ci/ci.py list                     # every package + its CI settings
    python3 scripts/ci/ci.py plan                     # GitHub matrix for the current event
    python3 scripts/ci/ci.py run foundation_ai        # fmt + clippy + tests for one package
    python3 scripts/ci/ci.py run foundation_ai --step test

Per-package settings live in `ci/packages.toml`. Everything else (the package
list, which packages need the llama.cpp submodule, who depends on whom) comes
from `cargo metadata`, so a new crate is picked up without editing CI.

Standard library only (Python 3.11+ for `tomllib`).
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
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
    test_args: list[str]
    timeout_minutes: int
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

    settings: dict[str, Settings] = {}
    for name in packages:
        package = overrides.get(name, {})
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
            test_args=list(merged.get("test-args", [])),
            timeout_minutes=int(merged.get("timeout-minutes", 60)),
            reason=merged.get("reason"),
            env=env,
            container_options=container_options,
            wasm_expect_error=wasm_expect_error,
        )
        for step in STEPS:
            if getattr(s, step) not in MODES:
                raise SystemExit(f"{name}: `{step}` must be one of {MODES}, got {getattr(s, step)!r}")
        settings[name] = s
    return settings


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


def commands(name: str, s: Settings, profile: str) -> dict[str, list[list[str]]]:
    if name == "examples":
        return examples_commands(profile)
    if name == "wasm-tests":
        return wasm_tests_commands()
    plan: dict[str, list[list[str]]] = {step: [] for step in STEPS}
    if s.fmt != "off":
        plan["fmt"].append(["cargo", "fmt", "-p", name, "--check"])
    if name == "foundation_core" and s.build == "enforce":
        # Spec 70 feature 80: the executor wake functions are private to valtron.
        # One repository-wide scan, run with the package that owns them (in the
        # enforced `build` step), and the test of the scan itself.
        plan["build"].append([sys.executable, "scripts/ci/check_executor_wake.py"])
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
            cmd = ["cargo", "test", "--profile", profile, "-p", name, "--no-fail-fast", *feature_flags(run)]
            # `--nocapture` prints every test's output, so a failing or hanging
            # test leaves its own trace instead of a bare "FAILED" line.
            cmd += ["--", "--nocapture", *s.test_args]
            plan["test"].append(cmd)
    if s.wasm != "off":
        # A wasm32 compile gate: no `--all-targets`/profile (wasm has no test
        # binaries or uat tuning), just `cargo check` on the wasm-relevant
        # feature set. Catches the `multi`-into-`!Send`-wasm leak and other
        # native-dep regressions that native-only builds never see.
        for run in s.wasm_runs:
            plan["wasm"].append(
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
            plan["wasm"].append(
                [sys.executable, "scripts/ci/expect_compile_error.py", expect["contains"], "--", *check]
            )
    return plan


def annotate(level: str, message: str) -> None:
    print(f"::{level}::{message}" if IN_GITHUB else f"{level}: {message}", flush=True)


def run_package(name: str, steps: list[str], profile: str) -> int:
    packages = load_workspace()
    if name not in packages:
        raise SystemExit(f"unknown package: {name}")
    s = load_settings(packages)[name]
    if s.skip:
        print(f"skipping {name}: {s.skip}")
        return 0

    plan = commands(name, s, profile)
    # The package's `env` applies to every step. A value already set in the
    # environment wins, so a bigger machine can still override it locally.
    env = {**s.env, **os.environ}
    env["CARGO_TARGET_DIR"] = target_dir()
    if s.env:
        print(f"{name}: env " + " ".join(f"{k}={v}" for k, v in sorted(s.env.items())), flush=True)
    print(f"{name}: target dir {env['CARGO_TARGET_DIR']}", flush=True)
    failures: list[str] = []
    # One hard deadline for the whole package. GitHub gives the same job
    # `timeout-minutes`; a local run had no kill switch, so a step that looped
    # (a hung test, a deadlocked build) never returned. Each command borrows
    # the time still left, and a command that overruns is fatal for the package.
    deadline = time.monotonic() + s.timeout_minutes * 60
    for step in steps:
        mode = getattr(s, step)
        for cmd in plan[step]:
            line = shlex.join(cmd)
            print(f"::group::{step}: {line}" if IN_GITHUB else f"$ {line}", flush=True)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                if IN_GITHUB:
                    print("::endgroup::", flush=True)
                failures.append(line)
                annotate("error", f"{name}: `{line}` timed out after {s.timeout_minutes} minutes")
                return 1
            try:
                code = subprocess.run(cmd, cwd=ROOT, env=env, timeout=remaining).returncode
            except subprocess.TimeoutExpired:
                if IN_GITHUB:
                    print("::endgroup::", flush=True)
                failures.append(line)
                annotate("error", f"{name}: `{line}` timed out after {s.timeout_minutes} minutes")
                return 1
            if IN_GITHUB:
                print("::endgroup::", flush=True)
            if code == 0:
                continue
            if mode == "enforce":
                failures.append(line)
                annotate("error", f"{name}: `{line}` failed")
            else:
                why = f" — {s.reason}" if s.reason else ""
                annotate("warning", f"{name}: `{line}` failed (report-only{why})")

    return 1 if failures else 0


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
    summary = f"{len(include)} package(s) selected ({why})"
    print(summary, file=sys.stderr)

    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a") as fh:
            fh.write(f"matrix={matrix}\ncount={len(include)}\n")
    if step_summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(step_summary, "a") as fh:
            fh.write(f"### CI plan\n\n{summary}\n\n")
            for item in include:
                fh.write(f"- `{item['package']}`{' (needs llama.cpp)' if item['llama'] else ''}\n")
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

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
