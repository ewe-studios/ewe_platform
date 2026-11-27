#!/usr/bin/env bash
# The end-to-end test of the control plane in a container (spec 77 feature 10,
# section 7, `e2e_control_image`).
#
# It builds the programs, starts the stack with Docker Compose, runs one
# command through the published TCP port, and stops the stack. It needs
# `docker` (with the Compose plugin) and `mise`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="$(cd "${here}/../.." && pwd)"

echo "== build the programs =="
(
  cd "${root}"
  mise run sandbox:bin
)

echo "== copy the programs into the build context =="
cp "${root}/target/agent/agent-sandbox" "${here}/agent-sandbox"
cp "${root}/target/agent/agent-slave-x86_64" "${here}/agent-slave-x86_64"
cp "${root}/target/agent/agent-slave-aarch64" "${here}/agent-slave-aarch64"

echo "== prepare the host run directory and the token =="
if [ ! -f "${here}/sandbox_token" ]; then
  openssl rand -hex 32 > "${here}/sandbox_token"
  chmod 600 "${here}/sandbox_token"
fi
# The daemon and the control plane share this path.
mkdir -p /var/run/ewe-sbx
chmod 700 /var/run/ewe-sbx

cleanup() {
  echo "== stop the stack =="
  (cd "${here}" && docker compose down --remove-orphans)
}
trap cleanup EXIT

echo "== start the stack =="
(
  cd "${here}"
  docker compose up --build --detach --wait
)

echo "== run a command through the published port =="
output="$("${root}/target/agent/agent-sandbox" run \
  --connect tcp:127.0.0.1:7070 \
  --token-file "${here}/sandbox_token" \
  --driver docker \
  --image alpine:3.22 \
  -- echo ok)"
printf '%s' "${output}" | grep -qx "ok"
echo "the control plane in a container answered: ${output}"
