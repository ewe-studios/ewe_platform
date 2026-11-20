#!/usr/bin/env bash
# Install the package-only starter on a private Android virtual device and check a server action.
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
: "${EWE_UI_KOTLIN:?Set EWE_UI_KOTLIN to the Kotlin package checkout}"
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
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
image="system-images;android-36;aosp_atd;$abi"
if [[ ! -x $emulator || ! -f "$ANDROID_HOME/system-images/android-36/aosp_atd/$abi/source.properties" ]]; then
    "$sdkmanager" --sdk_root="$ANDROID_HOME" --install emulator "$image"
fi
stage=$(mktemp -d)
emulator_pid=
host_pid=
cleanup() {
    result=$?
    trap - EXIT
    if [[ $result != 0 ]]; then
        for log in "$stage/host.log" "$stage/emulator.log"; do
            if [[ -f $log ]]; then tail -40 "$log" >&2; fi
        done
    fi
    if [[ -n $host_pid ]]; then kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; fi
    if [[ -n $emulator_pid ]]; then kill "$emulator_pid" 2>/dev/null || true; wait "$emulator_pid" 2>/dev/null || true; fi
    rm -r "$stage"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
git -C "$EWE_UI_KOTLIN" archive HEAD examples/starter_android | tar -x -C "$stage"
project="$stage/examples/starter_android"
version=$(python3 -c 'import tomllib; print(tomllib.load(open("release/native-ui.toml","rb"))["version"])')
args=("-PeweVersion=$version")
if [[ -n $repository ]]; then
    args+=("-PeweRepository=$repository")
else
    export GRADLE_USER_HOME="$stage/gradle"
fi
"$EWE_UI_KOTLIN/gradlew" -p "$project" "${args[@]}" :app:assembleDebug
if [[ -z ${EWE_TODO_BIN:-} ]]; then
    cargo build --profile uat -p ui_todo
    EWE_TODO_BIN="${CARGO_TARGET_DIR:-$root/target}/uat/ui_todo"
fi
"$EWE_TODO_BIN" 127.0.0.1:0 >"$stage/host.log" 2>&1 &
host_pid=$!
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
url=$(awk '/^Open http:/ { print $2;exit }' "$stage/host.log")
host_port=$(python3 -c 'import sys, urllib.parse; print(urllib.parse.urlsplit(sys.argv[1]).port or sys.exit("No host port"))' "$url")
"$adb" -s "$serial" reverse "tcp:$host_port" "tcp:$host_port"
"$adb" -s "$serial" install -r "$project/app/build/outputs/apk/debug/app-debug.apk"
"$adb" -s "$serial" shell am start -n ewe.starter/.MainActivity --es server "$url"
python3 "$root/scripts/ui_native/release/check-android-starter.py" "$adb" "$serial" "$url"
