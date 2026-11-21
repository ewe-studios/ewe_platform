#!/usr/bin/env bash
set -euo pipefail

: "${ANDROID_HOME:?mise must resolve the android-sdk tool first}"
: "${ANDROID_COMPILE_SDK:?run this script with mise}"
: "${ANDROID_BUILD_TOOLS:?run this script with mise}"
: "${ANDROID_NDK:?run this script with mise}"

packages=(
  platform-tools
  "platforms;android-$ANDROID_COMPILE_SDK"
  "build-tools;$ANDROID_BUILD_TOOLS"
  "ndk;$ANDROID_NDK"
)
if [[ -n "${ANDROID_UI_COMPILE_SDK:-}" && "$ANDROID_UI_COMPILE_SDK" != "$ANDROID_COMPILE_SDK" ]]; then
  packages+=("platforms;android-$ANDROID_UI_COMPILE_SDK")
fi
separator=/
missing=()
for package in "${packages[@]}"; do
  if [[ ! -f "$ANDROID_HOME/${package/;/$separator}/source.properties" ]]; then
    missing+=("$package")
  fi
done

if (( ${#missing[@]} == 0 )); then
  printf 'All Android SDK packages are present.\n'
  exit 0
fi

# sdkmanager closes its input when it finishes. Use its status.
(
  set +o pipefail
  yes | sdkmanager "--sdk_root=$ANDROID_HOME" --licenses >/dev/null
)

sdkmanager "--sdk_root=$ANDROID_HOME" --install "${missing[@]}"

for package in "${missing[@]}"; do
  if [[ ! -f "$ANDROID_HOME/${package/;/$separator}/source.properties" ]]; then
    printf 'FAIL android packages: sdkmanager did not install %s\n' "$package" >&2
    exit 1
  fi
done
