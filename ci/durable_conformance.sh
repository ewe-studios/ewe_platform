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
TMP="$(mktemp -d)"
SERVER_PID=""
SERVER_LOG="$TMP/server-conformance.log"
CLIENT_LOG="$TMP/client-conformance.log"

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
  npx vitest run server-conformance.test.ts
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

echo "== done: server 338 passed (338), client 267 passed, 0 failed, 2 suite-forced skips =="
