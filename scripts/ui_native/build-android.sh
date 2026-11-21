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
        env -i HOME="$HOME" PATH="$PATH" TMPDIR="${TMPDIR:-/tmp}" ANDROID_HOME="$ANDROID_HOME" ANDROID_NDK_HOME="$ANDROID_NDK_HOME" cargo ndk --platform 26 -t arm64-v8a -t x86_64 -o "$2" build --locked -p "$1" --release "${@:3}"
    }
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    install_library() {
        local name=$1 built=$2 output=$3 abi other
        for abi in arm64-v8a x86_64; do
            mkdir -p "$output/$abi"
            cp "$built/$abi/lib$name.so" "$output/$abi/"
            # Remove obsolete copies from older versions of this script.
            for other in foundation_ui_native foundation_nativeapis foundation_ui_connectrpc; do
                if [[ "$other" != "$name" ]]; then rm -f "$output/$abi/lib$other.so"; fi
            done
        done
    }
    build_native foundation_ui_native "$stage/binding"
    install_library foundation_ui_native "$stage/binding" "$1/binding/src/main/jniLibs"
    if [[ "$2" == "--with-todo" ]]; then
        build_native ui_todo "$stage/todo" --lib --no-default-features --features mobile
        for example in todo_server todo_on_device; do
            install_library ui_todo "$stage/todo" "$PWD/examples/ui_todo/android/$example/app/src/main/jniLibs"
        done
    fi
BASH
