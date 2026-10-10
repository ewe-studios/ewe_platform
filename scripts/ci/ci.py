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
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG_PATH = ROOT / "ci" / "packages.toml"
IN_GITHUB = bool(os.environ.get("GITHUB_ACTIONS"))

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
)

STEPS = ("fmt", "build", "clippy", "test")
MODES = ("enforce", "report", "off")

# The `dev` profile uses the Cranelift backend, which CI doesn't install. CI
# builds with `--profile uat`, but cargo invocations nested inside tests (e.g.
# trybuild compile tests) build their own project with `dev`; point that
# profile at LLVM too. A value already set in the environment wins.
NESTED_BUILD_ENV = {"CARGO_PROFILE_DEV_CODEGEN_BACKEND": "llvm"}


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
    test_args: list[str]
    timeout_minutes: int
    reason: str | None


def cargo_metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
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
        merged = {**defaults, **overrides.get(name, {})}
        s = Settings(
            skip=merged.get("skip"),
            fmt=merged.get("fmt", "report"),
            build=merged.get("build", "enforce"),
            clippy=merged.get("clippy", "enforce"),
            test=merged.get("test", "enforce"),
            runs=list(merged.get("runs", [""])) or [""],
            test_args=list(merged.get("test-args", [])),
            timeout_minutes=int(merged.get("timeout-minutes", 60)),
            reason=merged.get("reason"),
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


def commands(name: str, s: Settings, profile: str) -> dict[str, list[list[str]]]:
    plan: dict[str, list[list[str]]] = {step: [] for step in STEPS}
    if s.fmt != "off":
        plan["fmt"].append(["cargo", "fmt", "-p", name, "--check"])
    if s.build != "off":
        # Library, binaries, tests, examples and benches: compile errors show
        # up here, separately from test failures.
        plan["build"].append(
            ["cargo", "build", "--profile", profile, "-p", name, "--all-targets", *feature_flags(s.runs[0])]
        )
    if s.clippy != "off":
        plan["clippy"].append(
            ["cargo", "clippy", "--profile", profile, "-p", name, "--all-targets", *feature_flags(s.runs[0])]
        )
    if s.test != "off":
        for run in s.runs:
            cmd = ["cargo", "test", "--profile", profile, "-p", name, "--no-fail-fast", *feature_flags(run)]
            if s.test_args:
                cmd += ["--", *s.test_args]
            plan["test"].append(cmd)
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
    env = {**NESTED_BUILD_ENV, **os.environ}
    failures: list[str] = []
    for step in steps:
        mode = getattr(s, step)
        for cmd in plan[step]:
            line = shlex.join(cmd)
            print(f"::group::{step}: {line}" if IN_GITHUB else f"$ {line}", flush=True)
            code = subprocess.run(cmd, cwd=ROOT, env=env).returncode
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
        status = f"SKIP: {s.skip}" if s.skip else f"fmt={s.fmt} build={s.build} clippy={s.clippy} test={s.test}"
        runs = ", ".join(repr(r) for r in s.runs)
        print(f"{name:36} {status:42} runs=[{runs}]{'  +llama.cpp' if p.needs_llama else ''}")
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
