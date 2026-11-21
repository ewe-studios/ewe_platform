#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$script_dir/../.." && pwd)
unset JAVA_HOME
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

mkdir "$scratch/bin"
for tool in bash python3 dirname; do
  ln -s "$(command -v "$tool")" "$scratch/bin/$tool"
done

pins=$(python3 -I - "$root/mise.toml" <<'PY'
import sys
import tomllib

with open(sys.argv[1], "rb") as source:
    config = tomllib.load(source)
tools = config["tools"]
for tool in ("github:google/flatbuffers", "gradle", "kotlin", "java", "github:bbqsrc/cargo-ndk", "http:android-sdk"):
    value = tools[tool]
    print(value if isinstance(value, str) else value["version"])
print(config["env"]["XCODE_MIN"])
PY
)
{
  read -r flatc
  read -r gradle
  read -r kotlin
  read -r java
  read -r ndk
  read -r sdk
  read -r xcode
} <<< "$pins"

fake() {
  printf '#!/usr/bin/env bash\n%s\n' "$2" > "$scratch/bin/$1"
  chmod +x "$scratch/bin/$1"
}

fake flatc "echo 'flatc version $flatc'"
fake gradle "echo 'Gradle $gradle'"
fake kotlinc "echo 'info: kotlinc-jvm $kotlin (JRE 25)' >&2"
java=${java#temurin-}
java=${java%.0.LTS}
fake java "echo 'openjdk version \"${java%%+*}\"' >&2
echo 'OpenJDK Runtime Environment Temurin-$java (build $java)' >&2"
fake javac "echo 'javac ${java%%+*}'"
fake cargo "[[ \$* = 'ndk --version' ]]
echo 'cargo-ndk $ndk'"
fake cargo-ndk 'exit 1'
fake xcodebuild "echo 'Xcode $xcode'
echo 'Build version fixture'"
export ANDROID_HOME="$scratch/sdk"
mkdir -p "$ANDROID_HOME/cmdline-tools/bin"
printf 'Pkg.Revision=%s\n' "$sdk" > "$ANDROID_HOME/cmdline-tools/source.properties"
fake sdkmanager "echo 'Android CLI fixture'"
mv "$scratch/bin/sdkmanager" "$ANDROID_HOME/cmdline-tools/bin/sdkmanager"
ln -s "$ANDROID_HOME/cmdline-tools/bin/sdkmanager" "$scratch/bin/sdkmanager"

while IFS= read -r package; do
  mkdir -p "$ANDROID_HOME/$package"
  touch "$ANDROID_HOME/$package/source.properties"
done < <(python3 -I - "$root/mise.toml" <<'PYTHON'
import sys
import tomllib

with open(sys.argv[1], "rb") as source:
    env = tomllib.load(source)["env"]
print("platform-tools")
print("platforms/android-" + env["ANDROID_COMPILE_SDK"])
if env.get("ANDROID_UI_COMPILE_SDK"):
    print("platforms/android-" + env["ANDROID_UI_COMPILE_SDK"])
print("build-tools/" + env["ANDROID_BUILD_TOOLS"])
print("ndk/" + env["ANDROID_NDK"])
PYTHON
)
fake uname 'echo Darwin'

run_check() {
  local expected=$1 status=0
  shift
  output=$(PATH="$scratch/bin" bash "$script_dir/check.sh" "$@" 2>&1) || status=$?
  if [[ $status != "$expected" ]]; then
    printf 'Expected exit %s, got %s\n%s\n' "$expected" "$status" "$output" >&2
    exit 1
  fi
}

expect() {
  if [[ $output != *"$1"* ]]; then
    printf 'Expected output to contain %s\n%s\n' "$1" "$output" >&2
    exit 1
  fi
}

run_check 0
expect "ok flatc: $flatc"
expect "ok xcode: $xcode"
expect "ok android-sdk: $sdk"
expect 'ok android packages:'

rm "$ANDROID_HOME/platform-tools/source.properties"
run_check 1
expect 'FAIL android packages: missing platform-tools'
run_check 0 --scope wire
expect "ok flatc: $flatc"
[[ $output != *'android packages'* ]]
run_check 0 --scope apple
expect "ok xcode: $xcode"
[[ $output != *'flatc'* ]]
run_check 0 --scope android --without-android-packages
expect "ok android-sdk: $sdk"
[[ $output != *'xcode'* ]]
touch "$ANDROID_HOME/platform-tools/source.properties"

rm "$scratch/bin/flatc"
run_check 1
expect 'FAIL flatc: not found'

fake flatc 'echo "flatc version 0.0.1"'
run_check 1
expect "FAIL flatc: expected $flatc, got 0.0.1"

fake flatc "echo 'flatc version $flatc'
exit 1"
run_check 1
expect 'FAIL flatc: version command failed'
fake flatc "echo 'flatc version $flatc'"

export JAVA_HOME="$scratch/jdk"
mkdir -p "$JAVA_HOME/bin"
cp "$scratch/bin/java" "$scratch/bin/javac" "$JAVA_HOME/bin"
fake java 'echo "wrong Java on PATH"; exit 1'
run_check 0 --scope android
expect "ok java: $java"
unset JAVA_HOME
cp "$scratch/jdk/bin/java" "$scratch/bin/java"

fake xcodebuild 'echo "Xcode 1.0"'
run_check 1
expect "FAIL xcode: expected at least $xcode, got 1.0"

rm "$scratch/bin/xcodebuild" "$scratch/bin/sdkmanager"
fake uname 'echo Linux'
run_check 0
expect 'SKIP xcode'
expect 'SKIP android packages'
printf 'Toolchain check tests passed.\n'
