#!/usr/bin/env bash
# Build the two native libraries for the Android binding.
set -euo pipefail
worktree="$(cd "$(dirname "$0")/../.." && pwd)"
checkout="$(git -C "$worktree" rev-parse --path-format=absolute --git-common-dir)"
package="${EWE_UI_KOTLIN:-$(dirname "$(dirname "$checkout")")/ewe-ui-kotlin}"
demo="${1:-}"
cd "$worktree"
mise exec -- bash -euc '
    export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$ANDROID_NDK"
    rustup target add aarch64-linux-android x86_64-linux-android
    build_native() {
        env -i HOME="$HOME" PATH="$PATH" TMPDIR="${TMPDIR:-/tmp}" ANDROID_HOME="$ANDROID_HOME" ANDROID_NDK_HOME="$ANDROID_NDK_HOME" cargo ndk --platform 26 -t arm64-v8a -t x86_64 -o "$2" build --locked -p "$1" --release
    }
    build_native foundation_ui_native "$1/binding/src/main/jniLibs"
    if [[ "$2" == "--with-todo" ]]; then
        build_native ui_todo_native "$1/examples/ui_todo_android/app/src/main/jniLibs"
    fi
' _ "$package" "$demo"
