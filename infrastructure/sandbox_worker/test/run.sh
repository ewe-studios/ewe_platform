#!/usr/bin/env bash
# Starts `wrangler dev` and the local fake model server, runs the Cloudflare
# end-to-end tests, then stops both processes by their saved process identifiers
# (PIDs).
#
#   test/run.sh                 start the local Worker and run the tests
#   test/run.sh --keep-running  leave the Worker up and print its PID
#   test/run.sh --url <url>     run the tests against a deployed Worker; set
#                               EWE_SANDBOX_WORKER_TOKEN
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${WORKER_DIR}/../.." && pwd)"
REQUIRED="${EWE_SANDBOX_E2E_REQUIRED:-0}"

KEEP_RUNNING=0
DEPLOYED_URL=""
while [ $# -gt 0 ]; do
    case "$1" in
        --keep-running) KEEP_RUNNING=1 ;;
        --url)
            shift
            DEPLOYED_URL="${1:-}"
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

need() {
    command -v "$1" >/dev/null 2>&1
}

missing=""
for tool in cargo python3 npx curl; do
    need "$tool" || missing="${missing} ${tool}"
done
if [ -z "${DEPLOYED_URL}" ]; then
    for tool in wrangler worker-build docker; do
        need "$tool" || missing="${missing} ${tool}"
    done
fi
if [ -n "${missing}" ]; then
    if [ "${REQUIRED}" = "1" ]; then
        echo "the Cloudflare end-to-end tests need:${missing}" >&2
        exit 1
    fi
    echo "skipping the Cloudflare end-to-end tests: missing tools:${missing}"
    exit 0
fi

if [ -n "${DEPLOYED_URL}" ] && [ -z "${EWE_SANDBOX_WORKER_TOKEN:-}" ]; then
    echo "set EWE_SANDBOX_WORKER_TOKEN for --url ${DEPLOYED_URL}" >&2
    exit 2
fi

cd "${WORKER_DIR}"

WRANGLER_PID=""
MODEL_PID=""

cleanup() {
    if [ -n "${WRANGLER_PID}" ]; then
        kill "${WRANGLER_PID}" 2>/dev/null || true
        wait "${WRANGLER_PID}" 2>/dev/null || true
    fi
    if [ -n "${MODEL_PID}" ]; then
        kill "${MODEL_PID}" 2>/dev/null || true
        wait "${MODEL_PID}" 2>/dev/null || true
    fi
}

if [ -z "${DEPLOYED_URL}" ]; then
    WORKER_PORT="${EWE_SANDBOX_WORKER_PORT:-8787}"
    MODEL_PORT="${EWE_FAKE_MODEL_PORT:-8799}"
    TOKEN="${EWE_SANDBOX_WORKER_TOKEN:-test}"
    DEPLOYED_URL="http://127.0.0.1:${WORKER_PORT}"

    cat > .dev.vars <<EOF
SANDBOX_TOKEN=${TOKEN}
MODEL_BACKEND=openrouter
MODEL_BASE_URL=http://127.0.0.1:${MODEL_PORT}
MODEL_NAME=fake
MODEL_API_KEY=test
EOF

    python3 "${SCRIPT_DIR}/fake_model.py" --port "${MODEL_PORT}" &
    MODEL_PID=$!
    npx wrangler dev --local --port "${WORKER_PORT}" >"${WORKER_DIR}/wrangler-dev.log" 2>&1 &
    WRANGLER_PID=$!

    if [ "${KEEP_RUNNING}" = "1" ]; then
        echo "wrangler dev PID ${WRANGLER_PID}; fake model PID ${MODEL_PID}"
    else
        trap cleanup EXIT
    fi

    ready=0
    for _ in $(seq 1 180); do
        if ! kill -0 "${WRANGLER_PID}" 2>/dev/null; then
            echo "wrangler dev stopped; see ${WORKER_DIR}/wrangler-dev.log" >&2
            tail -50 "${WORKER_DIR}/wrangler-dev.log" >&2 || true
            exit 1
        fi
        if curl -s -o /dev/null "${DEPLOYED_URL}"; then
            ready=1
            break
        fi
        sleep 1
    done
    if [ "${ready}" != "1" ]; then
        echo "the Worker did not become ready at ${DEPLOYED_URL}" >&2
        exit 1
    fi
    export EWE_SANDBOX_WORKER_TOKEN="${TOKEN}"
fi

export EWE_SANDBOX_WORKER_URL="${DEPLOYED_URL}"
cd "${REPO_ROOT}"
cargo test -p foundation_sandbox --features "testing cloudflare" --profile uat \
    e2e_cloudflare -- --nocapture
