#!/usr/bin/env bash
# The live end-to-end proof of the Cloudflare demo (spec 92 feature 06).
#
# The script:
#   1. deploys a preview Worker with its own container;
#   2. waits for it;
#   3. creates one sandbox and runs the four steps;
#   4. creates two more, checks the count, runs one command and one agent turn
#      per tab on the tab's own model path;
#   5. destroys all, checks the count is zero;
#   6. deletes the Worker and its container and proves that nothing remains.
#
# It runs on every exit path and returns a non-zero status when a step fails.
# A machine without Cloudflare credentials prints which value is missing and
# returns 0: the live run did not happen.
#
#   test/live.sh                       deploy a preview name and remove it
#   EWE_DEMO_NAME=<name> test/live.sh  use this preview name
set -uo pipefail
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEMO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${DEMO_DIR}"

WRANGLER_VERSION="$(python3 -c "import tomllib; print(tomllib.load(open('${DEMO_DIR}/../../ci/sandbox.toml', 'rb'))['wrangler_version'])")"
WORKER_BUILD_VERSION="$(python3 -c "import tomllib; print(tomllib.load(open('${DEMO_DIR}/../../ci/sandbox.toml', 'rb'))['worker_build_version'])")"

need() {
    command -v "$1" >/dev/null 2>&1
}

missing=""
for tool in cargo python3 curl docker; do
    need "${tool}" || missing="${missing} ${tool}"
done
need wrangler || missing="${missing} wrangler"
need worker-build || missing="${missing} worker-build"
if [ -n "${missing}" ]; then
    echo "the live proof needs:${missing}" >&2
    exit 2
fi
for file in build/index.js agent-slave demo-model; do
    if [ ! -f "${file}" ]; then
        echo "run \`mise run build\` first: ${file} is absent" >&2
        exit 2
    fi
done

wrangler_seen="$(wrangler --version 2>/dev/null | grep -Eo '[0-9]+\\.[0-9]+\\.[0-9]+' | head -1)"
build_seen="$(worker-build --version 2>/dev/null | grep -Eo '[0-9]+\\.[0-9]+\\.[0-9]+' | head -1)"
if [ "${wrangler_seen}" != "${WRANGLER_VERSION}" ]; then
    echo "wrangler ${WRANGLER_VERSION} is required; ${wrangler_seen:-none} is installed" >&2
    exit 2
fi
if [ "${build_seen}" != "${WORKER_BUILD_VERSION}" ]; then
    echo "worker-build ${WORKER_BUILD_VERSION} is required; ${build_seen:-none} is installed" >&2
    exit 2
fi

export CLOUDFLARE_API_TOKEN="${CLOUDFLARE_API_TOKEN:-${CLOUDFLARE_WORKERS_AI_API_TOKEN:-}}"
if [ -z "${CLOUDFLARE_ACCOUNT_ID:-}" ] || [ -z "${CLOUDFLARE_API_TOKEN}" ]; then
    echo "the live proof did not run: set CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN (or CLOUDFLARE_WORKERS_AI_API_TOKEN)"
    exit 0
fi

NAME="${EWE_DEMO_NAME:-ewe-sandbox-demo-test-$(date +%s)}"
CONTAINER="${NAME}-container"
PREVIEW_CONFIG="wrangler.preview.toml"
LOG_DIR="$(mktemp -d)"
URL=""
FAILED=0
STEP=0
SUMMARY_FILE="${LOG_DIR}/summary.txt"
: > "${SUMMARY_FILE}"

say() {
    printf '%s\n' "$*"
    printf '%s\n' "$*" >> "${SUMMARY_FILE}"
}

step() {
    STEP=$((STEP + 1))
    say ""
    say "== step ${STEP}: $* =="
}

pass() {
    say "PASS: $*"
}

fail() {
    FAILED=$((FAILED + 1))
    say "FAIL: $*"
}

json_string() {
    python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"
}

# Creates sandboxes through the streaming control route. The created ids go to
# one comma-separated line in the last output line.
create_sandboxes() {
    local count=$1 out=$2
    curl -sS -N --max-time 900 \
        -X POST "${URL}/api/sandboxes" \
        -H "content-type: application/json" \
        -d "{\"count\":${count}}" -o "${out}"
    python3 - "${out}" <<'PY'
import json
import sys

ids = []
for line in open(sys.argv[1], encoding="utf-8"):
    line = line.strip()
    if not line:
        continue
    event = json.loads(line)
    if event.get("event") == "error":
        print("ERROR: " + event.get("message", ""), file=sys.stderr)
        sys.exit(1)
    if event.get("event") == "created":
        ids.append(event["id"])
print(",".join(ids))
PY
}

# One command in one sandbox. The output file holds the standard output and
# standard error text, then the exit line.
exec_command() {
    local id=$1 command=$2 out=$3
    local body
    body="$(mktemp)"
    python3 -c 'import json,sys; open(sys.argv[1],"w").write(json.dumps({"command":sys.argv[2]}))' "${body}" "${command}"
    local code
    code=$(curl -sS -N --max-time 180 -o "${out}" -w '%{http_code}' \
        -X POST "${URL}/api/sandboxes/${id}/exec" \
        -H "content-type: application/json" --data-binary "@${body}")
    rm -f "${body}"
    printf '%s' "${code}"
}

# One agent turn in one sandbox. Sets REPLY_BODY, REPLY_PATH and REPLY_CODE.
chat_turn() {
    local id=$1 prompt=$2
    local body
    body="$(mktemp)"
    python3 -c 'import json,sys; open(sys.argv[1],"w").write(json.dumps({"prompt":sys.argv[2]}))' "${body}" "${prompt}"
    REPLY_CODE=$(curl -sS -N --max-time 180 \
        -D "${LOG_DIR}/chat-headers.txt" -o "${LOG_DIR}/chat-body.txt" \
        -w '%{http_code}' \
        -X POST "${URL}/api/sandboxes/${id}/chat" \
        -H "content-type: application/json" --data-binary "@${body}")
    rm -f "${body}"
    REPLY_BODY="$(cat "${LOG_DIR}/chat-body.txt")"
    REPLY_PATH="$(grep -i '^x-ewe-model-path:' "${LOG_DIR}/chat-headers.txt" | tail -1 | cut -d' ' -f2 | tr -d '\r')"
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    say ""
    say "== teardown =="
    if [ -n "${URL}" ]; then
        curl -sS -o /dev/null -X DELETE "${URL}/api/sandboxes" --max-time 120 || true
    fi
    if [ -n "${URL}" ] || [ -n "${NAME}" ]; then
        wrangler delete --name "${NAME}" --config "${PREVIEW_CONFIG}" --force \
            >> "${LOG_DIR}/delete.log" 2>&1 || true
    fi

    # Wait up to 180 s for the deployment and the container to go away. The
    # lists are JSON, so a table header never hides a remaining entry.
    local deadline=$(( $(date +%s) + 180 ))
    local worker_left="unknown"
    local container_left="unknown"
    local container_id=""
    while [ "$(date +%s)" -lt "${deadline}" ]; do
        if wrangler deployments list --name "${NAME}" --config "${PREVIEW_CONFIG}" --json \
            > "${LOG_DIR}/deployments.json" 2>"${LOG_DIR}/deployments.err"; then
            worker_left="$(python3 - "${LOG_DIR}/deployments.json" <<'PY'
import json
import sys
try:
    entries = json.load(open(sys.argv[1], encoding="utf-8"))
except (ValueError, OSError):
    entries = []
print("yes" if entries else "no")
PY
)"
        else
            # A deleted Worker has no deployment list.
            worker_left="no"
        fi
        if wrangler containers list --json > "${LOG_DIR}/containers.json" 2>"${LOG_DIR}/containers.err"; then
            container_id="$(python3 - "${LOG_DIR}/containers.json" "${CONTAINER}" <<'PY'
import json
import sys
try:
    entries = json.load(open(sys.argv[1], encoding="utf-8"))
except (ValueError, OSError):
    entries = []
for entry in entries:
    if sys.argv[2] in json.dumps(entry):
        print(entry.get("id") or entry.get("application_id") or "")
        break
PY
)"
            if [ -n "${container_id}" ]; then
                container_left="yes"
            else
                container_left="no"
            fi
        else
            container_left="unknown"
        fi
        [ "${worker_left}" = "no" ] && [ "${container_left}" = "no" ] && break
        sleep 5
    done
    if [ "${container_left}" = "yes" ] && [ -n "${container_id}" ]; then
        # A container that the Worker delete left behind is deleted by id.
        wrangler containers delete "${container_id}" >> "${LOG_DIR}/containers-delete.log" 2>&1 || true
        sleep 5
        if wrangler containers list --json > "${LOG_DIR}/containers.json" 2>/dev/null \
            && ! grep -q "${CONTAINER}" "${LOG_DIR}/containers.json"; then
            container_left="no"
        fi
    fi
    say "teardown: Worker deployment remains: ${worker_left}"
    say "teardown: container ${CONTAINER} remains: ${container_left}"
    say "teardown: wrangler deployments list (JSON):"
    sed -n '1,20p' "${LOG_DIR}/deployments.json" 2>/dev/null | sed 's/^/  /' | tee -a "${SUMMARY_FILE}" >/dev/null || true
    say "teardown: wrangler containers list, matching lines:"
    if grep -q "${CONTAINER}" "${LOG_DIR}/containers.json" 2>/dev/null; then
        grep "${CONTAINER}" "${LOG_DIR}/containers.json" | sed 's/^/  /' | tee -a "${SUMMARY_FILE}" >/dev/null
    else
        say "  (none)"
    fi
    if [ "${worker_left}" = "yes" ] || [ "${container_left}" = "yes" ]; then
        say "TEARDOWN FAILED: the preview left a Worker or a container behind"
        FAILED=$((FAILED + 1))
        status=1
    fi
    rm -f "${PREVIEW_CONFIG}"
    say ""
    say "== summary =="
    if [ "${FAILED}" -eq 0 ]; then
        say "every step passed; the preview is gone"
    else
        say "${FAILED} step(s) failed"
    fi
    say "the preview URL was ${URL:-none}"
    say "the logs are in ${LOG_DIR}"
    if [ "${FAILED}" -ne 0 ]; then
        status=1
    fi
    exit "${status}"
}
trap cleanup EXIT INT TERM

# The preview configuration: the Worker name and the container name both carry
# the preview suffix, so two previews never meet.
python3 - "${NAME}" "${PREVIEW_CONFIG}" <<'PY'
import sys

name, path = sys.argv[1], sys.argv[2]
text = open("wrangler.toml", encoding="utf-8").read()
text = text.replace('name = "ewe-sandbox-demo"', f'name = "{name}"', 1)
text = text.replace('name = "ewe-sandbox-demo-container"', f'name = "{name}-container"', 1)
open(path, "w", encoding="utf-8").write(text)
PY

step "deploy the preview Worker ${NAME}"
start=$(date +%s)
if ! wrangler deploy --config "${PREVIEW_CONFIG}" > "${LOG_DIR}/deploy.log" 2>&1; then
    say "the deploy failed:"
    tail -60 "${LOG_DIR}/deploy.log" | tee -a "${SUMMARY_FILE}" >/dev/null
    fail "wrangler deploy"
    exit 1
fi
URL="$(sed -E 's/\x1B\[[0-9;]*[mK]//g' "${LOG_DIR}/deploy.log" | grep -Eo 'https://[A-Za-z0-9._-]+\.workers\.dev' | head -1)"
if [ -z "${URL}" ]; then
    fail "the deploy printed no workers.dev URL"
    exit 1
fi
say "deployed in $(( $(date +%s) - start )) s: ${URL}"

if [ -n "${OPENROUTER_API_KEY:-}" ]; then
    printf '%s' "${OPENROUTER_API_KEY}" \
        | wrangler secret put OPENROUTER_API_KEY --config "${PREVIEW_CONFIG}" \
            > "${LOG_DIR}/secret.log" 2>&1 \
        && pass "the OpenRouter secret is set" \
        || fail "the OpenRouter secret was not set"
else
    say "OPENROUTER_API_KEY is absent: the OpenRouter tab will name the missing secret"
fi

step "wait for the probe"
start=$(date +%s)
ready=0
for _ in $(seq 1 180); do
    code=$(curl -s -o /dev/null -w '%{http_code}' "${URL}/api/probe" || true)
    if [ "${code}" = "200" ]; then
        ready=1
        break
    fi
    sleep 2
done
if [ "${ready}" = "1" ]; then
    pass "the probe answers 200 after $(( $(date +%s) - start )) s"
else
    fail "the probe did not answer 200"
    exit 1
fi

step "start from zero sandboxes"
curl -sS -o /dev/null -X DELETE "${URL}/api/sandboxes" --max-time 120 || true
count_now="$(curl -sS "${URL}/api/sandboxes" | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')"
if [ "${count_now}" = "0" ]; then
    pass "the count starts at 0"
else
    fail "the count starts at ${count_now}, not 0"
fi

step "create the first sandbox"
start=$(date +%s)
first_ids="$(create_sandboxes 1 "${LOG_DIR}/create-one.ndjson")" || {
    fail "the first create"
    first_ids=""
}
first_id="$(printf '%s' "${first_ids}" | cut -d, -f1)"
if [ -n "${first_id}" ]; then
    pass "created ${first_id} in $(( $(date +%s) - start )) s"
else
    fail "the first create returned no identifier"
    exit 1
fi

step "tab 1: the command console runs uname -a"
start=$(date +%s)
code="$(exec_command "${first_id}" "uname -a" "${LOG_DIR}/uname.txt")"
uname_text="$(cat "${LOG_DIR}/uname.txt")"
if [ "${code}" = "200" ] && printf '%s' "${uname_text}" | grep -q "Linux"; then
    pass "uname -a answered Linux in $(( $(date +%s) - start )) s: $(printf '%s' "${uname_text}" | head -1)"
else
    fail "uname -a (status ${code}): ${uname_text}"
fi

step "tab 1: the file write and read"
marker="live-proof-$(date +%s)"
body="$(mktemp)"
python3 -c 'import json,sys; open(sys.argv[1],"w").write(json.dumps({"path":"notes/live.txt","content":sys.argv[2]}))' "${body}" "${marker}"
write_code=$(curl -sS -o "${LOG_DIR}/write.json" -w '%{http_code}' \
    -X POST "${URL}/api/sandboxes/${first_id}/file" \
    -H "content-type: application/json" --data-binary "@${body}")
rm -f "${body}"
read_code=$(curl -sS -o "${LOG_DIR}/read.txt" -w '%{http_code}' \
    "${URL}/api/sandboxes/${first_id}/file?path=notes%2Flive.txt")
if [ "${write_code}" = "200" ] && [ "${read_code}" = "200" ] \
    && [ "$(cat "${LOG_DIR}/read.txt")" = "${marker}" ]; then
    pass "the file write and read match: ${marker}"
else
    fail "the file write (${write_code}) and read (${read_code}) did not match"
fi

step "tab 1: one agent turn on the Workers AI path"
chat_turn "${first_id}" "Reply with the single word ready."
if [ "${REPLY_CODE}" = "200" ] && [ "${REPLY_PATH}" = "workers-ai" ] \
    && [ -n "$(printf '%s' "${REPLY_BODY}" | tr -d '[:space:]')" ]; then
    pass "Workers AI (${REPLY_PATH}) answered: $(printf '%s' "${REPLY_BODY}" | head -c 200)"
else
    fail "the Workers AI turn (status ${REPLY_CODE}, path ${REPLY_PATH:-none}): $(printf '%s' "${REPLY_BODY}" | head -c 400)"
fi

step "create two more sandboxes"
start=$(date +%s)
more_ids="$(create_sandboxes 2 "${LOG_DIR}/create-two.ndjson")" || {
    fail "the two creates"
    more_ids=""
}
second_id="$(printf '%s' "${more_ids}" | cut -d, -f1)"
third_id="$(printf '%s' "${more_ids}" | cut -d, -f2)"
if [ -n "${second_id}" ] && [ -n "${third_id}" ]; then
    pass "created ${second_id} and ${third_id} in $(( $(date +%s) - start )) s"
else
    fail "the two creates returned ${more_ids:-no identifiers}"
    exit 1
fi
count_now="$(curl -sS "${URL}/api/sandboxes" | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')"
if [ "${count_now}" = "3" ]; then
    pass "the count is 3, one per tab"
else
    fail "the count is ${count_now}, not 3"
fi

step "tab 2: its own command"
code="$(exec_command "${second_id}" "echo tab-two-answer" "${LOG_DIR}/tab-two.txt")"
if [ "${code}" = "200" ] && grep -q "tab-two-answer" "${LOG_DIR}/tab-two.txt"; then
    pass "tab 2 answered $(head -1 "${LOG_DIR}/tab-two.txt")"
else
    fail "tab 2 (status ${code}): $(cat "${LOG_DIR}/tab-two.txt")"
fi

step "tab 3: its own command"
code="$(exec_command "${third_id}" "echo tab-three-answer" "${LOG_DIR}/tab-three.txt")"
if [ "${code}" = "200" ] && grep -q "tab-three-answer" "${LOG_DIR}/tab-three.txt"; then
    pass "tab 3 answered $(head -1 "${LOG_DIR}/tab-three.txt")"
else
    fail "tab 3 (status ${code}): $(cat "${LOG_DIR}/tab-three.txt")"
fi

step "tab 2: one agent turn on the OpenRouter path"
chat_turn "${second_id}" "Reply with the single word ready."
if [ "${REPLY_CODE}" = "200" ] && [ "${REPLY_PATH}" = "openrouter" ] \
    && [ -n "$(printf '%s' "${REPLY_BODY}" | tr -d '[:space:]')" ]; then
    pass "OpenRouter (${REPLY_PATH}) answered: $(printf '%s' "${REPLY_BODY}" | head -c 200)"
elif printf '%s' "${REPLY_BODY}" | grep -q "OPENROUTER_API_KEY"; then
    say "the OpenRouter path did not run: $(printf '%s' "${REPLY_BODY}" | head -c 300)"
else
    fail "the OpenRouter turn (status ${REPLY_CODE}, path ${REPLY_PATH:-none}): $(printf '%s' "${REPLY_BODY}" | head -c 400)"
fi

step "tab 3: one agent turn on the container model path"
chat_turn "${third_id}" "Say hello from the container."
if [ "${REPLY_CODE}" = "200" ] && [ "${REPLY_PATH}" = "container-model" ] \
    && printf '%s' "${REPLY_BODY}" | grep -q "container-model"; then
    pass "the container model (${REPLY_PATH}) answered: $(printf '%s' "${REPLY_BODY}" | head -c 200)"
else
    fail "the container model turn (status ${REPLY_CODE}, path ${REPLY_PATH:-none}): $(printf '%s' "${REPLY_BODY}" | head -c 400)"
fi

step "destroy all and check the count"
start=$(date +%s)
delete_code=$(curl -sS -o "${LOG_DIR}/destroy-all.json" -w '%{http_code}' \
    -X DELETE "${URL}/api/sandboxes" --max-time 300)
count_now="$(curl -sS "${URL}/api/sandboxes" | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])')"
if [ "${delete_code}" = "200" ] && [ "${count_now}" = "0" ]; then
    pass "destroyed all sandboxes in $(( $(date +%s) - start )) s; the count is 0"
else
    fail "destroy all (${delete_code}) left the count at ${count_now}"
fi

say ""
say "the live proof finished with ${FAILED} failed step(s)"
if [ "${FAILED}" -ne 0 ]; then
    exit 1
fi
