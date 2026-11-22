#!/usr/bin/env bash
# Install the package-only starter on a private Android virtual device and check a server action.
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
mode=starter
if [[ ${1:-} == --todo ]]; then
    mode=todo
    shift
fi
case $# in
    0) repository= ;;
    2) [[ $1 == --maven ]] || { echo "Expected --maven DIRECTORY" >&2; exit 2; }
       repository=$(cd "$2" && pwd) ;;
    *) echo "Usage: $0 [--maven DIRECTORY]" >&2; exit 2 ;;
esac
cd "$root"
case $(uname -m) in
    arm64|aarch64) abi=arm64-v8a ;;
    x86_64) abi=x86_64 ;;
    *) echo "Unsupported emulator architecture: $(uname -m)" >&2; exit 1 ;;
esac
if [[ $(uname -s) == Linux && ! -w /dev/kvm ]]; then
    echo "The Android emulator needs read and write access to /dev/kvm." >&2
    exit 1
fi
adb="$ANDROID_HOME/platform-tools/adb"
emulator="$ANDROID_HOME/emulator/emulator"
sdkmanager=$(command -v sdkmanager)
avdmanager=$(command -v avdmanager)
image_kind=aosp_atd
if [[ -n ${EWE_THEME_CAPTURES:-} ]]; then image_kind=google_apis; fi
image="system-images;android-36;$image_kind;$abi"
if [[ ! -x $emulator || ! -f "$ANDROID_HOME/system-images/android-36/$image_kind/$abi/source.properties" ]]; then
    "$sdkmanager" --sdk_root="$ANDROID_HOME" --install emulator "$image"
fi
stage=$(mktemp -d)
emulator_pid=
host_pid=
cleanup() {
    result=$?
    if [[ -n ${data_pid:-} ]]; then kill "$data_pid" 2>/dev/null || true; wait "$data_pid" 2>/dev/null || true; fi
    trap - EXIT
    if [[ $result != 0 ]]; then
        for log in "$stage/host.log" "$stage/emulator.log"; do
            if [[ -f $log ]]; then tail -40 "$log" >&2; fi
        done
    fi
    if [[ -n $host_pid ]]; then kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; fi
    if [[ -n ${host_port:-} && -n ${serial:-} ]]; then
        "$adb" -s "$serial" reverse --remove "tcp:$host_port" 2>/dev/null || true
    fi
    if [[ -n $emulator_pid ]]; then kill "$emulator_pid" 2>/dev/null || true; wait "$emulator_pid" 2>/dev/null || true; fi
    rm -r "$stage"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
version=$(python3 -c 'import tomllib; print(tomllib.load(open("release/native-ui.toml","rb"))["version"])')
args=("-PeweVersion=$version")
if [[ -n $repository ]]; then
    args+=("-PeweRepository=$repository")
else
    export GRADLE_USER_HOME="$stage/gradle"
fi
if [[ $mode == todo ]]; then
    for app in todo_on_device todo_server; do
        "$root/mobile/android/gradlew" -p "$root/examples/ui_todo/android/$app" "${args[@]}" :app:assembleDebug
    done
    server="$root/examples/ui_todo/android/todo_server"
    "$root/mobile/android/gradlew" -p "$server" "${args[@]}" :app:assembleDebugAndroidTest
else
    mkdir -p "$stage/starter"
    rsync -a --exclude build --exclude .gradle --exclude .kotlin --exclude local.properties         "$root/examples/mobile_starter/android/" "$stage/starter/"
    project="$stage/starter"
    "$root/mobile/android/gradlew" -p "$project" "${args[@]}" :app:assembleDebug
fi
if [[ -z ${EWE_TODO_BIN:-} ]]; then
    cargo build --profile uat -p ui_todo
    EWE_TODO_BIN="${CARGO_TARGET_DIR:-$root/target}/uat/ui_todo"
fi
if [[ -n ${EWE_ANDROID_SERIAL:-} ]]; then
    serial="$EWE_ANDROID_SERIAL"
else
export ANDROID_AVD_HOME="$stage/avd"
mkdir -p "$ANDROID_AVD_HOME"
echo no | "$avdmanager" create avd --name ewe-release --package "$image" --device pixel
config="$ANDROID_AVD_HOME/ewe-release.avd/config.ini"
python3 - "$config" "$abi" <<'PY'
import sys
from pathlib import Path
config = Path(sys.argv[1])
settings = {}
for line in config.read_text().splitlines():
    if "=" in line:
        key, value = line.split("=", 1)
        settings[key.strip()] = value.strip()
settings["image.sysdir.1"] = f"system-images/android-36/aosp_atd/{sys.argv[2]}/"
settings["disk.dataPartition.size"] = "2G"
config.write_text("".join(f"{key}={value}\n" for key, value in settings.items()))
PY
# The emulator needs two consecutive ports. Leave existing devices running.
port=$(python3 - <<'PY'
import socket
for port in range(5554, 5682, 2):
    with socket.socket() as console, socket.socket() as transport:
        try:
            console.bind(("127.0.0.1", port))
            transport.bind(("127.0.0.1", port + 1))
        except OSError:
            continue
        print(port)
        break
else:
    raise SystemExit("No available emulator port pair")
PY
)
serial="emulator-$port"
"$adb" start-server
"$emulator" -avd ewe-release -port "$port" -no-window -no-audio -no-snapshot -no-boot-anim \
    -partition-size 2048 -gpu swiftshader_indirect >"$stage/emulator.log" 2>&1 &
emulator_pid=$!
deadline=$((SECONDS + ${EWE_ANDROID_BOOT_TIMEOUT:-240}))
until [[ $("$adb" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r') == 1 ]]; do
    if ((SECONDS >= deadline)) || ! kill -0 "$emulator_pid" 2>/dev/null; then
        echo "The emulator did not start." >&2
        exit 1
    fi
    sleep 2
done
fi

if [[ $mode == todo ]]; then
    local_app="$root/examples/ui_todo/android/todo_on_device"
    "$adb" -s "$serial" install -r "$local_app/app/build/outputs/apk/debug/app-debug.apk"
    "$adb" -s "$serial" install -r "$server/app/build/outputs/apk/debug/app-debug.apk"
    "$adb" -s "$serial" install -r "$server/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk"
    "$adb" -s "$serial" shell am force-stop ewe.todo.local
    "$adb" -s "$serial" shell am start -n ewe.todo.local/.MainActivity
    python3 "$root/scripts/ui_native/check-android-todo.py" "$adb" "$serial" offline
fi

"$EWE_TODO_BIN" 127.0.0.1:0 >"$stage/host.log" 2>&1 &
host_pid=$!
deadline=$((SECONDS + 30))
until url=$(awk '/^Open http:/ { print $2;exit }' "$stage/host.log") && [[ -n $url ]]; do
    if ((SECONDS >= deadline)) || ! kill -0 "$host_pid" 2>/dev/null; then
        echo "The todo server did not start." >&2
        exit 1
    fi
    sleep 1
done
host_port=$(python3 -c 'import sys, urllib.parse; print(urllib.parse.urlsplit(sys.argv[1]).port or sys.exit("No host port"))' "$url")
"$adb" -s "$serial" reverse "tcp:$host_port" "tcp:$host_port"
if [[ $mode == todo ]]; then
    "$adb" -s "$serial" shell am force-stop ewe.todo
    "$adb" -s "$serial" shell am start -n ewe.todo/.MainActivity --es server "$url" --ez rpc "${EWE_TODO_RPC:-false}" --ez socket "${EWE_TODO_SOCKET:-false}"
    python3 "$root/scripts/ui_native/check-android-todo.py" "$adb" "$serial" server
    "$EWE_TODO_BIN" 127.0.0.1:0 --data-only >"$stage/data-host.log" 2>&1 &
    data_pid=$!
    for ((i=0; i<100; i++)); do
        kill -0 "$data_pid"
        data_url=$(awk '/^Open http:/ {print $2;exit}' "$stage/data-host.log")
        [[ -z "$data_url" ]] || break
        sleep 0.1
    done
    [[ -n "$data_url" ]]
    data_port=$(python3 -c 'import sys,urllib.parse;print(urllib.parse.urlsplit(sys.argv[1]).port)' "$data_url")
    "$adb" -s "$serial" reverse "tcp:$data_port" "tcp:$data_port"
    result=$("$adb" -s "$serial" shell am instrument -w -e data true -e server "$data_url" ewe.todo.test/ewe.todo.ReplayCheck)
    printf '%s\n' "$result"
    [[ $result == *"GetTodo returned the requested UUID, Buy milk, and NotFound."* ]]
    "$adb" -s "$serial" reverse --remove "tcp:$data_port"
    kill "$data_pid"
    wait "$data_pid" || true
    data_pid=
    subscriptions=$("$adb" -s "$serial" shell am instrument -w -e subscriptions true -e server "$url" ewe.todo.test/ewe.todo.ReplayCheck)
    printf '%s\n' "$subscriptions"
    [[ $subscriptions == *"delivered targeted expiry."* ]] || exit 1
    replay=$("$adb" -s "$serial" shell am instrument -w -e server "$url" ewe.todo.test/ewe.todo.ReplayCheck)
    printf '%s\n' "$replay"
    [[ $replay == *"Second delete refused:"* ]] || exit 1
    "$adb" -s "$serial" shell am start -n ewe.todo/.MainActivity --es server "$url" --ez rpc "${EWE_TODO_RPC:-false}" --ez socket "${EWE_TODO_SOCKET:-false}"
    python3 "$root/scripts/ui_native/check-android-todo.py" "$adb" "$serial" server-delete
    "$adb" -s "$serial" shell am start -n ewe.todo.local/.MainActivity
    python3 "$root/scripts/ui_native/check-android-todo.py" "$adb" "$serial" isolation
    if [[ -n ${EWE_THEME_CAPTURES:-} ]]; then
        python3 "$root/scripts/ui_native/check-android-todo.py" "$adb" "$serial" theme-review
    fi
else
    "$adb" -s "$serial" install -r "$project/app/build/outputs/apk/debug/app-debug.apk"
    "$adb" -s "$serial" shell am start -n ewe.starter/.MainActivity --es server "$url"
    python3 "$root/scripts/ui_native/release/check-android-starter.py" "$adb" "$serial" "$url"
fi
