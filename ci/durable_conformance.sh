#!/usr/bin/env bash
# Run both reference Durable Streams conformance suites and check their counts.
#
# The server suite runs against the example server in this repository. The
# client suite runs against the example client adapter.
#
# The gate passes only when:
#   - the server suite reports 338 passed (338) with no skip;
#   - the client suite reports 0 failed and 267 passed; and
#   - the only client skips are the two tests that the reference suite forces
#     to skip with the reason "Swift implementation broken".
#
# Usage: ci/durable_conformance.sh
#
# Environment:
#   DURABLE_CONFORMANCE_PORT  server port (default 4437)
#
# The script needs Node.js 20 or later and network access for the suites.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${DURABLE_CONFORMANCE_PORT:-4437}"
SERVER_URL="http://127.0.0.1:${PORT}"
# The pinned wrangler version for the Durable Object driver. `mise.toml` sets
# the same value; keep the fallback in step with it.
WRANGLER_VERSION="${WRANGLER_VERSION:-4.156.2}"
export WRANGLER_VERSION
TMP="$(mktemp -d)"
SERVER_PID=""
SERVER_LOG="$TMP/server-conformance.log"
CLIENT_LOG="$TMP/client-conformance.log"

# The live S3 run is mandatory in continuous integration (CI). When the require
# flag is set, every variable must be present, so a misconfigured gate fails
# instead of skipping the live run.
if [ "${EWE_REQUIRE_LIVE_S3:-}" = "1" ]; then
  missing=""
  for variable in EWE_S3_ENDPOINT EWE_S3_ACCESS_KEY EWE_S3_SECRET_KEY EWE_S3_BUCKET; do
    if [ -z "${!variable:-}" ]; then
      missing="${missing} ${variable}"
    fi
  done
  if [ -n "$missing" ]; then
    echo "FAIL: EWE_REQUIRE_LIVE_S3=1 but these variables are missing:${missing}" >&2
    exit 1
  fi
fi

cleanup() {
  if [ -n "$SERVER_PID" ]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

echo "== build =="
cargo build -p foundation_http --features multi --example durable_server
cargo build -p foundation_netio --features multi --example durable_client_adapter

echo "== live S3 =="
if [ -n "${EWE_S3_ENDPOINT:-}" ]; then
  # The VFS conformance kit, the two-writer compare-and-swap race, the restart
  # spool replay and the SQL block tier all run against the live service.
  cargo test --profile uat -p foundation_nativeapis --features vfs-object-netio --test durable_object_vfs
  echo "live S3: passed against ${EWE_S3_ENDPOINT}"
else
  echo "live S3: skipped (EWE_S3_ENDPOINT is not set)"
fi

echo "== server conformance =="
DURABLE_ADDR="127.0.0.1:${PORT}" "$ROOT/target/debug/examples/durable_server" &
SERVER_PID=$!

for _ in $(seq 1 100); do
  if curl -s -o /dev/null "${SERVER_URL}/__conformance_probe"; then
    break
  fi
  sleep 0.1
done

if ! (
  cd "$TMP"
  npm init -y >/dev/null 2>&1
  npm install --silent @durable-streams/server-conformance-tests@0.3.7 vitest@4.1.12
  cat > server-conformance.test.ts <<EOF
import { describe } from "vitest"
import { runConformanceTests } from "@durable-streams/server-conformance-tests"

describe("ewe durable streams server", () => {
  runConformanceTests({ baseUrl: "${SERVER_URL}", subscriptions: true })
})
EOF
  # The vitest default is 5 s per test. A loaded CI host needs room for the
  # property tests; the assertions do not change.
  npx vitest run server-conformance.test.ts --testTimeout=30000
) 2>&1 | tee "$SERVER_LOG"; then
  echo "FAIL: the server conformance suite reported a failure" >&2
  exit 1
fi

# Vitest prints "338 passed (338)" only when the run has no skip.
if ! grep -Eq 'Tests[[:space:]]+338 passed \(338\)' "$SERVER_LOG"; then
  echo "FAIL: the server suite must report 338 passed (338) with no skip" >&2
  grep -E 'Tests[[:space:]]+' "$SERVER_LOG" >&2 || true
  exit 1
fi
echo "server conformance: $(grep -E 'Tests[[:space:]]+338 passed \(338\)' "$SERVER_LOG" | head -n 1 | sed -E 's/^[[:space:]]*//')"

echo "== client conformance =="
if ! (
  cd "$TMP"
  npm install --silent @durable-streams/client-conformance-tests@0.2.13
  npx @durable-streams/client-conformance-tests \
    --run "$ROOT/target/debug/examples/durable_client_adapter"
) 2>&1 | tee "$CLIENT_LOG"; then
  echo "FAIL: the client conformance suite reported a failure" >&2
  exit 1
fi

summary() {
  sed -nE "s/^$1: ([0-9]+).*/\1/p" "$CLIENT_LOG" | tail -n 1
}
total="$(summary Total)"
passed="$(summary Passed)"
failed="$(summary Failed)"
skipped="$(summary Skipped)"
if [ -z "$total" ] || [ -z "$passed" ] || [ -z "$failed" ] || [ -z "$skipped" ]; then
  echo "FAIL: the client suite did not print its summary counts" >&2
  exit 1
fi
echo "client conformance: ${passed} passed, ${failed} failed, ${skipped} skipped"

if [ "$failed" -ne 0 ]; then
  echo "FAIL: the client suite has ${failed} failed test(s)" >&2
  exit 1
fi
if [ "$passed" -ne 267 ] || [ "$skipped" -ne 2 ] || [ "$total" -ne 269 ]; then
  echo "FAIL: the client suite must report 267 passed and 2 skipped of 269 tests" >&2
  exit 1
fi

# Each skip prints as "  o <name> (skipped: <reason>)". Only the two
# suite-forced Swift skips are allowed.
skip_lines="$(grep -E '\(skipped' "$CLIENT_LOG" || true)"
skip_count="$(printf '%s\n' "$skip_lines" | grep -c . || true)"
if [ "$skip_count" -ne 2 ]; then
  echo "FAIL: the client suite must skip exactly 2 tests, not ${skip_count}" >&2
  printf '%s\n' "$skip_lines" >&2
  exit 1
fi
bad_skips="$(printf '%s\n' "$skip_lines" | grep -v 'Swift implementation broken' || true)"
if [ -n "$bad_skips" ]; then
  echo "FAIL: a client skip has a reason other than the suite-forced Swift skip" >&2
  printf '%s\n' "$bad_skips" >&2
  exit 1
fi

echo "== Durable Object transport matrix =="
# The DO driver needs Node, wrangler and worker-build. Use the pinned wrangler
# through npx; the fixture builds with worker-build. The fixture has its own
# Cargo.lock. A binary install of worker-build is faster than a source build.
if ! command -v worker-build >/dev/null 2>&1; then
  if command -v cargo-binstall >/dev/null 2>&1; then
    cargo binstall --no-confirm worker-build@0.8.7
  else
    cargo install worker-build --version 0.8.7 --locked
  fi
fi
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo test --profile uat -p durable_transport_matrix --features do-driver --test do_driver
echo "DO transport matrix: all eight scenarios match the model and the embedded driver"

echo "== local Cloudflare R2 block store =="
# `wrangler dev --local` gives a real local R2 bucket through Miniflare, so the
# wasm block store runs with no Cloudflare account. The test also checks the
# bare-etag `onlyIf` semantics, because older Miniflare versions reversed them.
cargo test --profile uat -p foundation_nativeapis --features r2-wrangler --test r2_wrangler
echo "local R2: the block store round trip passed under wrangler"

echo "== done: server 338 passed (338), client 267 passed, 0 failed, 2 suite-forced skips =="
