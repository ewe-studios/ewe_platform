#!/usr/bin/env bash
# Build the two native libraries for the Android binding.
set -euo pipefail
worktree="$(cd "$(dirname "$0")/../.." && pwd)"
package="$worktree/mobile/android"
demo="${1:-}"
cd "$worktree"
case "$demo" in
    ""|--with-todo) ;;
    *) echo "Usage: $0 [--with-todo]" >&2; exit 2 ;;
esac
mise exec -- bash -seu -- "$package" "$demo" <<'BASH'
    export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$ANDROID_NDK"
    rustup target add aarch64-linux-android x86_64-linux-android
    build_native() {
        env -i HOME="$HOME" PATH="$PATH" TMPDIR="${TMPDIR:-/tmp}" ANDROID_HOME="$ANDROID_HOME" ANDROID_NDK_HOME="$ANDROID_NDK_HOME" cargo ndk --platform 26 -t arm64-v8a -t x86_64 -o "$2" build --locked -p "$1" --release
    }
    build_native foundation_ui_native "$1/binding/src/main/jniLibs"
    if [[ "$2" == "--with-todo" ]]; then
        stage="$(mktemp -d)"
        trap 'rm -rf "$stage"' EXIT
        build_native ui_todo_native "$stage"
        for example in todo_server todo_on_device; do
            dest="$PWD/examples/ui_todo/android/$example/app/src/main/jniLibs"
            mkdir -p "$dest"
            rsync -a --delete "$stage/" "$dest/"
        done
    fi
BASH
