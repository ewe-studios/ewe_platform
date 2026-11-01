#!/usr/bin/env bash
# scripts/setup/fff.sh — install the `fff` CLI (fast file finder + grep) into cargo bin.
# Run via `mise run tools:fff` (or directly). bash 3.2 safe. Idempotent.
#
# `fff` replaces ripgrep for code search in this repo: `fff grep <pattern>`.
# It is a git-only cargo install (no crates.io binary, no GitHub release), so it
# is not declared in [tools]; this script pins the commit instead.
set -euo pipefail

FFF_GIT_URL="https://github.com/magnusmalm/fff-cli"
# Pinned commit for reproducible builds. Keep in sync with `tools:fff` in mise.toml.
FFF_REV="8b9930ee3ca0e76df5542a089f3ab7314009a54d"

log() { printf '[setup:fff] %s\n' "$*"; }
fail() { log "ERROR: $*"; exit 1; }

command -v cargo >/dev/null 2>&1 || fail "cargo is not on PATH (install Rust via rust-toolchain.toml first)"

if command -v fff >/dev/null 2>&1; then
    log "fff already present at $(command -v fff)"
    log "to reinstall pinned rev: cargo install --git $FFF_GIT_URL --rev $FFF_REV --locked --force"
    exit 0
fi

log "installing fff at pinned rev $FFF_REV ..."
cargo install --git "$FFF_GIT_URL" --rev "$FFF_REV" --locked

command -v fff >/dev/null 2>&1 || fail "install finished but fff is not on PATH"
log "fff installed: $(fff --version 2>/dev/null || echo 'version unavailable')"
log "fff setup complete"
