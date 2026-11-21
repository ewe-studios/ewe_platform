#!/usr/bin/env bash
# Test both todo apps on one iPhone simulator.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
device=${1:?Pass the identifier of an iPhone simulator}
target_dir=${CARGO_TARGET_DIR:-$root/target}
products="$root/target/native-ui/ios-todo-tests"
mkdir -p "$products"
server=
cleanup() {
    if [[ -n "$server" ]]; then kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

scripts/ui_native/prepare-ios-todo.sh
xcrun simctl bootstatus "$device" -b

test_app() {
    local app=$1 scheme=$2
    xcodebuild -project "$root/examples/ui_todo/ios/$app/$scheme.xcodeproj" \
        -scheme "$scheme" -destination "platform=iOS Simulator,id=$device" \
        -derivedDataPath "$products/$scheme" -clonedSourcePackagesDirPath "$products/packages" \
        -parallel-testing-enabled NO CODE_SIGNING_ALLOWED=NO test
}

test_app todo_on_device TodoOnDevice
cargo build --locked --profile uat -p ui_todo --bin ui_todo
examples/ui_todo/build-module.sh
"$target_dir/uat/ui_todo" 127.0.0.1:0 > "$products/server.log" 2>&1 &
server=$!
base=
for ((i=0; i<100; i++)); do
    kill -0 "$server"
    base=$(sed -n 's|^Open \(http://127\.0\.0\.1:[0-9]*\)/$|\1|p' "$products/server.log" | head -n1)
    if [[ -n "$base" ]] && curl -fsS --max-time 2 "$base/native-initial" -o "$products/first.bin"; then break; fi
    sleep 0.2
done
[[ -n "$base" && -s "$products/first.bin" ]] || { cat "$products/server.log" >&2; exit 1; }
TEST_RUNNER_EWE_TODO_URL="$base" test_app todo_server TodoServer
curl -fsS --max-time 5 "$base/native-initial" -o "$products/second.bin"
LC_ALL=C grep -aFq 'Server integration task' "$products/second.bin"
if LC_ALL=C grep -aFq 'Device-only task' "$products/second.bin"; then
    echo 'The server includes a task from the on-device store.' >&2
    exit 1
fi
