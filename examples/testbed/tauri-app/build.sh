#!/usr/bin/env bash
# Build and run the Tauri E2E test app inside a Linux/macOS VM.
#
# Usage (inside guest):
#   mise run build    — build the Tauri app in release mode
#   mise run verify   — check the build artifact exists and is valid
#   mise run launch   — launch the built binary headlessly
#
# The mise toolchain (rust, tauri-cli) is expected to be pre-installed
# by the testbed bootstrap (BOOTSTRAP_MISE_TOML).

set -euo pipefail

BIN="target/$(rustc -vV | sed -n 's|host: ||p')/release/tauri-e2e-test"

case "${1:-build}" in
  build)
    echo "[tauri-app] building in release mode..."
    cargo tauri build
    echo "[tauri-app] built: $BIN"
    file "$BIN" || true
    ;;
  verify)
    if [ -f "$BIN" ]; then
      echo "[tauri-app] artifact found: $BIN"
      file "$BIN"
      exit 0
    else
      echo "[tauri-app] artifact missing: $BIN"
      exit 1
    fi
    ;;
  launch)
    if [ ! -f "$BIN" ]; then
      echo "[tauri-app] binary not found, building first..."
      cargo tauri build
    fi
    echo "[tauri-app] launching headlessly on display :99..."
    DISPLAY=:99 "$BIN" &
    APP_PID=$!
    sleep 5
    if kill -0 "$APP_PID" 2>/dev/null; then
      echo "[tauri-app] alive (pid $APP_PID)"
      kill "$APP_PID" 2>/dev/null || true
      exit 0
    else
      echo "[tauri-app] dead after 5s"
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 {build|verify|launch}"
    exit 1
    ;;
esac
