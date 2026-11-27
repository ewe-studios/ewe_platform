#!/usr/bin/env bash
# Starts the moto AWS KMS emulator for the local AWS KMS test.
#
#   test/run.sh                 start moto, run the test, then stop moto
#   test/run.sh --keep-running  leave moto up and print its process identifier
#   test/run.sh --stop          stop a moto that --keep-running left up
#
# The test is `cargo test -p foundation_auth --features testing --profile uat
# kms::aws_local`. It needs no secret. The container maps the host port
# `KMS_MOTO_PORT` (default 5055) to the moto port 5000. The default avoids port
# 5000, which macOS gives to ControlCenter. `ci/local-services` uses port 5055
# for the same reason.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
PORT="${KMS_MOTO_PORT:-5055}"
# The digest is the one that `ci/local-services/docker-compose.yml` pins, so a
# laptop run and a CI run use the same bytes.
IMAGE="${KMS_MOTO_IMAGE:-motoserver/moto@sha256:344e076f31ad1f5ff1ec4492f741e7d42911d1c7a1ef9b1590f56c340b968513}"
CONTAINER="ewe-kms-moto-${PORT}"
PID_FILE="${REPO_ROOT}/target/kms-moto-${PORT}.pid"

KEEP_RUNNING=0
STOP=0
while [ $# -gt 0 ]; do
    case "$1" in
        --keep-running) KEEP_RUNNING=1 ;;
        --stop) STOP=1 ;;
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

if ! need docker; then
    echo "the local KMS test needs docker" >&2
    exit 1
fi

if [ "${STOP}" = "1" ]; then
    docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
    rm -f "${PID_FILE}"
    echo "stopped ${CONTAINER}"
    exit 0
fi

for tool in curl; do
    if ! need "${tool}"; then
        echo "the local KMS test needs ${tool}" >&2
        exit 1
    fi
done

if docker inspect "${CONTAINER}" >/dev/null 2>&1; then
    echo "the container ${CONTAINER} exists; run this task with --stop first" >&2
    exit 1
fi

echo "starting ${IMAGE} on port ${PORT}"
docker run -d --rm --name "${CONTAINER}" -p "${PORT}:5000" "${IMAGE}" >/dev/null

mkdir -p "${REPO_ROOT}/target"
docker inspect -f '{{.State.Pid}}' "${CONTAINER}" >"${PID_FILE}"

# Wait for the server. The moto API answers 200 when the server is ready.
deadline=$(($(date +%s) + 60))
while :; do
    if ! docker inspect "${CONTAINER}" >/dev/null 2>&1; then
        echo "the moto container stopped; see docker logs ${CONTAINER}" >&2
        docker logs --tail 50 "${CONTAINER}" >&2 || true
        exit 1
    fi
    if curl -s -o /dev/null "http://127.0.0.1:${PORT}/moto-api/"; then
        break
    fi
    if [ "$(date +%s)" -ge "${deadline}" ]; then
        echo "moto did not answer within 60 seconds" >&2
        docker logs --tail 50 "${CONTAINER}" >&2 || true
        exit 1
    fi
    sleep 1
done

echo "export AWS_ENDPOINT_URL_KMS=http://localhost:${PORT}"
echo "export AWS_ACCESS_KEY_ID=test"
echo "export AWS_SECRET_ACCESS_KEY=test"

if [ "${KEEP_RUNNING}" = "1" ]; then
    echo "moto runs as ${CONTAINER}; PID $(cat "${PID_FILE}")"
    echo "stop it with: mise run kms:moto -- --stop"
    exit 0
fi

cleanup() {
    docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
    rm -f "${PID_FILE}"
}
trap cleanup EXIT

export AWS_ENDPOINT_URL_KMS="http://localhost:${PORT}"
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
cd "${REPO_ROOT}"
cargo test -p foundation_auth --features testing --profile uat kms::aws_local
