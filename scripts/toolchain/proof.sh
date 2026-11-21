#!/usr/bin/env bash
set -euo pipefail

scope=full
if [[ $# == 2 && "$1" == --scope && "$2" == apple ]]; then
    scope=apple
elif [[ $# != 0 ]]; then
    printf 'Usage: %s [--scope apple]\n' "${0##*/}" >&2
    exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$script_dir/../.."
root="$PWD"
if [[ "$(uname -s)" != Darwin ]]; then
    printf 'The UniFFI proof requires macOS and the Xcode iOS simulator SDK.\n' >&2
    exit 1
fi

# Use full Xcode when the active tools contain no simulator SDK.
if [[ -z "${DEVELOPER_DIR:-}" ]] &&
    [[ "$(xcode-select -p 2>/dev/null || :)" == /Library/Developer/CommandLineTools ]] &&
    [[ -d /Applications/Xcode.app/Contents/Developer ]]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

rust_toolchain="$(rustup show active-toolchain | cut -d ' ' -f 1)"
export RUSTUP_TOOLCHAIN="$rust_toolchain"
sdk_version="$(xcrun --sdk iphonesimulator --show-sdk-version)"
sdk_path="$(xcrun --sdk iphonesimulator --show-sdk-path)"
target="$(uname -m)-apple-ios${sdk_version}-simulator"

work="$(mktemp -d)"
trap 'find "$work" -depth -delete' EXIT
cp -R "$script_dir/proof/." "$work/"
cd "$work"

# Keep the host library and the generator together in the temporary directory.
env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND cargo build --locked --target-dir target
library=target/debug/libuniffi_proof.dylib
bindgen=target/debug/uniffi-bindgen
"$bindgen" generate --library "$library" --language swift --out-dir swift --no-format

swift_binding=swift/uniffi_proof.swift
if ! grep -Eq 'func pushOps.*async' "$swift_binding"; then
    printf 'The Swift binding has no async pushOps function.\n' >&2
    exit 1
fi
xcrun --sdk iphonesimulator swiftc -sdk "$sdk_path" -target "$target" -typecheck \
    -Xcc "-fmodule-map-file=$work/swift/uniffi_proofFFI.modulemap" "$swift_binding"
xcrun --sdk iphonesimulator swiftc -sdk "$sdk_path" -target "$target" -emit-object \
    -Xcc "-fmodule-map-file=$work/swift/uniffi_proofFFI.modulemap" "$swift_binding" -o proof.o
[[ -s proof.o ]]

rust_version="$(rustc --version)"
uniffi_version="$(sed -nE 's/^uniffi = .*version = "=([^"]+)".*/\1/p' Cargo.toml)"
swift_version="$(xcrun swiftc --version | sed -n '1p')"
xcode_version="$(xcodebuild -version | sed -n '1p')"
report="$(printf '%s\n' "Rust: $rust_version" "UniFFI: $uniffi_version" "Swift: $swift_version" \
    "Xcode: $xcode_version" "iOS simulator SDK: $sdk_version")"
if [[ "$scope" == apple ]]; then
    printf '%s\n' "$report"
    exit 0
fi

# The Gradle distribution supplies the Kotlin binding dependencies.
gradle_home="$(cd "$root" && mise where gradle)"
jna=("$gradle_home"/gradle-*/lib/plugins/jna-[0-9]*.jar)
coroutines=("$gradle_home"/gradle-*/lib/kotlinx-coroutines-core-jvm-[0-9]*.jar)
if [[ ${#jna[@]} != 1 || ! -f ${jna[0]} || ${#coroutines[@]} != 1 || ! -f ${coroutines[0]} ]]; then
    printf 'Expected one JNA jar and one Kotlin coroutines jar in %s.\n' "$gradle_home" >&2
    exit 1
fi

"$bindgen" generate --library "$library" --language kotlin --out-dir kotlin --no-format
kotlin_binding=kotlin/uniffi/uniffi_proof/uniffi_proof.kt
if ! grep -Eq 'suspend fun (pushOps|`pushOps`)\(' "$kotlin_binding"; then
    printf 'The Kotlin binding has no suspend pushOps function.\n' >&2
    exit 1
fi
kotlinc -classpath "${jna[0]}:${coroutines[0]}" "$kotlin_binding" -d proof.jar
[[ -s proof.jar ]]

kotlin_version="$(kotlinc -version 2>&1 | sed -n '/kotlinc-jvm/{p;}')"

report="$(printf '%s\n' "$report" "Kotlin: $kotlin_version")"
python3 - "$root/specifications/69-foundation-ui-v2/learnings.md" "$report" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
start = "<!-- uniffi-proof:start -->"
end = "<!-- uniffi-proof:end -->"
versions = "\n".join(f"- {line}" for line in sys.argv[2].splitlines())
section = f"{start}\n## UniFFI toolchain proof\n\n{versions}\n{end}"
original = path.read_text()
prefix, marker, remainder = original.partition(start)
if marker:
    _, closing_marker, suffix = remainder.partition(end)
    if not closing_marker:
        sys.exit("The UniFFI proof section has no end marker.")
    updated = prefix + section + suffix
else:
    updated = original.rstrip() + "\n\n" + section + "\n"
if updated != original:
    path.write_text(updated)
PY
printf '%s\n' "$report"
