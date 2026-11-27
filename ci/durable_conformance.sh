#!/usr/bin/env bash
# Run both reference Durable Streams conformance suites.
#
# The server suite runs against the example server in this repository. The
# client suite runs against the example client adapter.
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

(
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
)

echo "== client conformance =="
(
  cd "$TMP"
  npm install --silent @durable-streams/client-conformance-tests@0.2.13
  npx @durable-streams/client-conformance-tests \
    --run "$ROOT/target/debug/examples/durable_client_adapter"
)

echo "== done: both suites passed =="
