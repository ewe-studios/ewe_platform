#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

mkdir "$scratch/bin"
export ANDROID_HOME="$scratch/sdk dir"
export ANDROID_COMPILE_SDK=fixture-platform
export ANDROID_UI_COMPILE_SDK=fixture-ui-platform
export ANDROID_BUILD_TOOLS=fixture-build-tools
export ANDROID_NDK=fixture-ndk
export SDK_LOG="$scratch/sdk.log"
export PATH="$scratch/bin:$PATH"

cat > "$scratch/bin/sdkmanager" <<'SDK'
#!/usr/bin/env bash
set -euo pipefail
[[ $1 = "--sdk_root=$ANDROID_HOME" ]]
shift
case "$1" in
  --licenses)
    read -r answer
    [[ $answer = y ]]
    [[ ${FAIL_LICENSES:-0} = 0 ]] || exit 42
    printf '%s\n' "$1" >> "$SDK_LOG"
    ;;
  --install)
    printf '%s\n' "$1" >> "$SDK_LOG"
    shift
    for package in "$@"; do
      printf '%s\n' "$package" >> "$SDK_LOG"
      directory="$ANDROID_HOME/$(tr ';' '/' <<< "$package")"
      mkdir -p "$directory"
      touch "$directory/source.properties"
    done
    ;;
  *)
    exit 2
    ;;
esac
SDK
chmod +x "$scratch/bin/sdkmanager"

run_android() {
  local expected=$1 status=0
  output=$(bash "$script_dir/android.sh" 2>&1) || status=$?
  if [[ $status != "$expected" ]]; then
    printf 'Expected exit %s, got %s\n%s\n' "$expected" "$status" "$output" >&2
    exit 1
  fi
}

export FAIL_LICENSES=1
run_android 42
[[ ! -e $SDK_LOG ]]
unset FAIL_LICENSES

run_android 0
expected=$(printf '%s\n' --licenses --install platform-tools "platforms;android-$ANDROID_COMPILE_SDK" "build-tools;$ANDROID_BUILD_TOOLS" "ndk;$ANDROID_NDK" "platforms;android-$ANDROID_UI_COMPILE_SDK")
[[ $(cat "$SDK_LOG") = "$expected" ]]

run_android 0
[[ $output = *'All Android SDK packages are present.'* ]]
[[ $(cat "$SDK_LOG") = "$expected" ]]
printf 'Android package tests passed.\n'
