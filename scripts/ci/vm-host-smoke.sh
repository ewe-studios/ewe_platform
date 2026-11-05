#!/usr/bin/env bash
# Smoke-test a VM host container from `docker-compose.yaml`.
#
# Usage:
#   scripts/ci/vm-host-smoke.sh <service> <container> <web-port> [seconds]
#
# Example:
#   scripts/ci/vm-host-smoke.sh macos ewe_macos 8006 600
#
# The images run the guest operating system in QEMU inside the container.
# `docker exec` reaches the container's Linux side, never the guest, so this
# script checks only what the host side can prove:
#
#   1. the container starts and stays running,
#   2. its web console answers on the published port,
#   3. the repository is visible at `/shared` inside the container.
#
# In-guest builds and tests run in the provisioned VMs of
# `.github/workflows/platform-vms.yaml`, which reach the guest over SSH.
#
# Env overrides:
#   COMPOSE_FILE           compose file to use (default: docker-compose.yaml)
#   EWE_SHARED_DIR         host directory mounted at /shared (default in the
#                          compose file is the developer's own checkout)
#   EWE_<OS>_STORAGE       host directory mounted at /storage

set -euo pipefail

service="${1:?usage: vm-host-smoke.sh <service> <container> <web-port> [seconds]}"
container="${2:?missing container name}"
web_port="${3:?missing web console port}"
boot_timeout="${4:-600}"
compose_file="${COMPOSE_FILE:-docker-compose.yaml}"

compose=(docker compose -f "$compose_file")

dump() {
    echo "::group::docker compose ps"
    "${compose[@]}" ps || true
    echo "::endgroup::"
    echo "::group::docker logs $container"
    docker logs --tail 100 "$container" 2>&1 || true
    echo "::endgroup::"
}

fail() {
    echo "::error::$*"
    dump
    exit 1
}

echo "Starting the $service VM host (container $container)..."
"${compose[@]}" up -d "$service"

# A container that `docker inspect` cannot see yet is still being created: allow
# a minute before calling it gone. A container that exists but is not running
# has already died, so report that at once with its log.
echo "Waiting up to ${boot_timeout}s for $container to run..."
start=$SECONDS
missing=0
state=missing
while (( SECONDS - start < boot_timeout )); do
    state=$(docker inspect -f '{{.State.Running}}' "$container" 2>/dev/null) || state=missing
    case "$state" in
        true) break ;;
        missing)
            missing=$((missing + 1))
            if (( missing >= 6 )); then
                fail "the $container container does not exist"
            fi
            ;;
        *) fail "the $container container is not running" ;;
    esac
    sleep 10
done
[[ "$state" == true ]] || fail "the $container container did not start in ${boot_timeout}s"
echo "$container runs after $((SECONDS - start))s"

# The web console is served by the container from the moment it starts, so it
# answers while the guest installs. It proves the image's entry point is alive.
echo "Waiting up to 180s for the web console on port $web_port..."
start=$SECONDS
until (exec 3<>"/dev/tcp/127.0.0.1/$web_port") 2>/dev/null; do
    if (( SECONDS - start >= 180 )); then
        fail "the $container web console does not answer on port $web_port"
    fi
    sleep 5
done
echo "The web console answers on port $web_port"

# The compose file mounts the repository at /shared. A host path that does not
# exist is not an error for Docker: it creates an empty directory. Check for the
# workspace itself so an empty mount cannot pass as a good one.
docker exec "$container" test -f /shared/Cargo.toml ||
    fail "$container has no workspace at /shared (check EWE_SHARED_DIR and the volumes in docker-compose.yaml)"

"${compose[@]}" ps
echo "The $service VM host passed the smoke test"
