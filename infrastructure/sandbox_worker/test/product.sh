#!/usr/bin/env bash
# Runs the Cloudflare Sandbox product end-to-end tests (spec 77 feature 17).
#
# The script:
#   1. builds the donor image and writes the product Dockerfile;
#   2. checks that the donor image layer holds one file;
#   3. reads the SHA-256 of `sandbox-shim` from Cloudflare's donor image;
#   4. starts `wrangler dev` with `wrangler.product.toml` and saves its process
#      identifier (PID);
#   5. runs the product tests, then the fault test with an image that has no
#      slave.
#
# Every `wrangler` process is stopped by the PID that this script saved.
#
#   test/product.sh                 run every step
#   EWE_SANDBOX_E2E_REQUIRED=1      turn a skipped test into a failure
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${WORKER_DIR}/../.." && pwd)"
REQUIRED="${EWE_SANDBOX_E2E_REQUIRED:-0}"
TOKEN="${EWE_SANDBOX_TOKEN:-test}"
PORT="${EWE_SANDBOX_PRODUCT_PORT:-8787}"
NOSLAVE_PORT="${EWE_SANDBOX_PRODUCT_NOSLAVE_PORT:-8788}"
STATE_DIR="${WORKER_DIR}/.wrangler/product-state"
NOSLAVE_STATE_DIR="${WORKER_DIR}/.wrangler/product-noslave-state"

need() {
    command -v "$1" >/dev/null 2>&1
}

if ! need docker; then
    echo "Docker is necessary for wrangler dev containers" >&2
    exit 2
fi

missing=""
for tool in cargo python3 npx curl tar mise; do
    need "$tool" || missing="${missing} ${tool}"
done
for tool in wrangler worker-build; do
    need "$tool" || missing="${missing} ${tool}"
done
if [ -n "${missing}" ]; then
    if [ "${REQUIRED}" = "1" ]; then
        echo "the product end-to-end tests need:${missing}" >&2
        exit 1
    fi
    echo "skipping the product end-to-end tests: missing tools:${missing}"
    exit 0
fi

PRODUCT_PID=""
NOSLAVE_PID=""
IMAGE_ARCHIVE=""
SHIM_FILE=""

cleanup() {
    for pid in "${PRODUCT_PID}" "${NOSLAVE_PID}"; do
        if [ -n "${pid}" ]; then
            kill "${pid}" 2>/dev/null || true
            wait "${pid}" 2>/dev/null || true
        fi
    done
    [ -n "${IMAGE_ARCHIVE}" ] && rm -f "${IMAGE_ARCHIVE}"
    [ -n "${SHIM_FILE}" ] && rm -f "${SHIM_FILE}"
    rm -f "${WORKER_DIR}/Dockerfile.product.noslave"
    rm -f "${WORKER_DIR}/wrangler.noslave.toml"
    rm -f "${WORKER_DIR}/.dev.vars"
}
trap cleanup EXIT

wait_for_worker() {
    local pid="$1"
    local url="$2"
    local log="$3"
    for _ in $(seq 1 300); do
        if ! kill -0 "${pid}" 2>/dev/null; then
            echo "wrangler dev stopped; see ${log}" >&2
            tail -50 "${log}" >&2 || true
            exit 1
        fi
        if curl -s -o /dev/null "${url}"; then
            return 0
        fi
        sleep 1
    done
    echo "the Worker did not become ready at ${url}" >&2
    exit 1
}

version=$(python3 -c "import tomllib; print(tomllib.load(open('${REPO_ROOT}/backends/foundation_sandbox/Cargo.toml', 'rb'))['package']['version'])")
slave_image="ewe/agent-slave:${version}"
tools_image=$(python3 -c "import tomllib; print(tomllib.load(open('${REPO_ROOT}/ci/sandbox.toml', 'rb'))['cloudflare_sandbox_tools_image'])")

# Invariants I4 and I5: the crate does not name the shim and does not depend on
# the product package. The control plane uses `DriverId::CloudflareContainer`.
if grep -RIn --include='*.rs' -e 'sandbox-shim' -e '@cloudflare/sandbox' \
    "${REPO_ROOT}/backends/foundation_sandbox/src"; then
    echo "the foundation_sandbox sources name the product shim or package" >&2
    exit 1
fi

cd "${REPO_ROOT}"

# Step 1: the donor image and the product Dockerfile.
mise trust --yes mise.toml
mise run sandbox:slave-image
mise run sandbox:product-dockerfile

# Step 2: the donor image has one file and no other layer.
#
# `docker export` also writes the container runtime files (`/etc/hosts`,
# `/dev`, ...). The image layer is the content of the image, so `docker save`
# and its first layer give the exact entry list.
IMAGE_ARCHIVE=$(mktemp)
layer_dir=$(mktemp -d)
trap 'cleanup; rm -rf "${layer_dir}"' EXIT
docker save "${slave_image}" -o "${IMAGE_ARCHIVE}"
tar -xf "${IMAGE_ARCHIVE}" -C "${layer_dir}"
layer=$(python3 -c "import json; print(json.load(open('${layer_dir}/manifest.json'))[0]['Layers'][0])")
entries=$(tar -tf "${layer_dir}/${layer}" | sort)
expected=$(printf 'usr/\nusr/local/\nusr/local/bin/\nusr/local/bin/agent-slave\n')
if [ "${entries}" != "${expected}" ]; then
    echo "the donor image does not hold one file: ${entries}" >&2
    exit 1
fi

# Step 3: the SHA-256 of `sandbox-shim` in Cloudflare's donor image.
SHIM_FILE=$(mktemp)
containers=$(docker create --entrypoint /bin/true "${tools_image}")
docker cp "${containers}:/usr/local/bin/sandbox-shim" "${SHIM_FILE}"
docker rm "${containers}" >/dev/null
if need sha256sum; then
    EWE_SHIM_SHA256=$(sha256sum "${SHIM_FILE}" | awk '{print $1}')
else
    EWE_SHIM_SHA256=$(shasum -a 256 "${SHIM_FILE}" | awk '{print $1}')
fi
export EWE_SHIM_SHA256
export EWE_SANDBOX_TOKEN="${TOKEN}"

# Step 4: the product Worker.
cat > "${WORKER_DIR}/.dev.vars" <<EOF
SANDBOX_TOKEN=${TOKEN}
EOF
cd "${WORKER_DIR}"
: > product-dev.log
npx wrangler dev --local --port "${PORT}" --persist-to "${STATE_DIR}" \
    -c wrangler.product.toml >>product-dev.log 2>&1 &
PRODUCT_PID=$!
export EWE_SANDBOX_URL="http://127.0.0.1:${PORT}"
wait_for_worker "${PRODUCT_PID}" "${EWE_SANDBOX_URL}" "${WORKER_DIR}/product-dev.log"

# Step 5: the product tests.
cd "${REPO_ROOT}"
cargo test -p foundation_sandbox --features "testing" --profile uat e2e_cf -- \
    --nocapture

# The fault: an image with no slave. The image is the product image without the
# second `COPY` line, and the Worker is a second local Worker.
grep -v '^COPY --from=agent-slave ' \
    "${WORKER_DIR}/Dockerfile.product" > "${WORKER_DIR}/Dockerfile.product.noslave"
sed \
    -e 's/ewe-sandbox/ewe-sandbox-noslave/g' \
    -e 's|^image = "./Dockerfile.product"$|image = "./Dockerfile.product.noslave"|' \
    "${WORKER_DIR}/wrangler.product.toml" > "${WORKER_DIR}/wrangler.noslave.toml"

kill "${PRODUCT_PID}" 2>/dev/null || true
wait "${PRODUCT_PID}" 2>/dev/null || true
PRODUCT_PID=""

cd "${WORKER_DIR}"
: > product-noslave.log
npx wrangler dev --local --port "${NOSLAVE_PORT}" --persist-to "${NOSLAVE_STATE_DIR}" \
    -c wrangler.noslave.toml >>product-noslave.log 2>&1 &
NOSLAVE_PID=$!
export EWE_SANDBOX_NOSLAVE_URL="http://127.0.0.1:${NOSLAVE_PORT}"
wait_for_worker "${NOSLAVE_PID}" "${EWE_SANDBOX_NOSLAVE_URL}" "${WORKER_DIR}/product-noslave.log"

cd "${REPO_ROOT}"
cargo test -p foundation_sandbox --features "testing" --profile uat \
    e2e_cf_product::e2e_cf_product_no_slave -- --nocapture
