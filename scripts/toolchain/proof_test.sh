#!/usr/bin/env bash
set -euo pipefail

source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/repo"
tools="$work/bin"
export TMPDIR="$work/tmp"
export PROOF_LOG="$work/tools.log"
mkdir -p "$tools" "$TMPDIR" "$root/scripts/toolchain" "$root/specifications/69-foundation-ui-v2" \
    "$work/gradle/gradle-9.9.1/lib/plugins"
cp "$source_dir"/proof.sh "$root/scripts/toolchain/"
cp -R "$source_dir"/proof "$root/scripts/toolchain/"
printf '# Learnings\n' >"$root/specifications/69-foundation-ui-v2/learnings.md"
touch "$work/gradle/gradle-9.9.1/lib/"{kotlinx-coroutines-core-jvm-1.10.2.jar,plugins/jna-5.17.0.jar}
export GRADLE_FIXTURE="$work/gradle"

cat >"$tools/dispatch" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
printf '%s %s\n' "${0##*/}" "$*" >>"$PROOF_LOG"
case "${0##*/} $*" in
    "uname -s") echo Darwin ;;
    "uname -m") echo arm64 ;;
    "xcode-select -p") echo /fake/Xcode.app/Contents/Developer ;;
    "rustup show active-toolchain") echo 'nightly-pinned (override)' ;;
    "rustc --version") echo 'rustc proof-test' ;;
    "xcodebuild -version") printf 'Xcode 27.2\nBuild version fake\n' ;;
    "xcrun --sdk iphonesimulator --show-sdk-version") echo 27.2 ;;
    "xcrun --sdk iphonesimulator --show-sdk-path") echo /fake/simulator.sdk ;;
    "xcrun swiftc --version") printf 'Apple Swift version fake\nTarget: fake\n' ;;
    "xcrun --sdk iphonesimulator swiftc "*)
        [[ "$*" == *'-target arm64-apple-ios27.2-simulator'*  ]]
        case "$*" in
            *'-typecheck'*) ;;
            *'-emit-object'*'-o proof.o') echo fixture >proof.o ;;
            *) exit 1 ;;
        esac
        ;;
    "mise where gradle") echo "$GRADLE_FIXTURE" ;;
    "kotlinc -version") echo 'info: kotlinc-jvm proof-test' >&2 ;;
    "kotlinc -classpath "*)
        [[ "$*" == *'jna-5.17.0.jar'* && "$*" == *'kotlinx-coroutines-core-jvm-1.10.2.jar'* ]]
        [[ "${FAIL_KOTLIN:-}" != 1 ]] || exit 1
        echo fixture >proof.jar
        ;;
    "cargo build "*)
        mkdir -p target/debug
        touch target/debug/libuniffi_proof.dylib
        cat >target/debug/uniffi-bindgen <<'BINDGEN'
#!/usr/bin/env bash
set -euo pipefail
printf 'bindgen %s\n' "$*" >>"$PROOF_LOG"
[[ "$1" == generate && "$2" == --library && -f "$3" && "$4" == --language && "$6" == --out-dir ]]
mkdir -p "$7"
case "$5" in
    swift)
        echo 'public func pushOps() async throws {}' >"$7/uniffi_proof.swift"
        touch "$7/uniffi_proofFFI.modulemap"
        ;;
    kotlin)
        mkdir -p "$7/uniffi/uniffi_proof"
        echo 'suspend fun `pushOps`() {}' >"$7/uniffi/uniffi_proof/uniffi_proof.kt"
        ;;
    *) exit 1 ;;
esac
BINDGEN
        chmod +x target/debug/uniffi-bindgen
        ;;
    *) printf 'Unexpected tool invocation. %s %s\n' "$0" "$*" >&2; exit 1 ;;
esac
FAKE
chmod +x "$tools/dispatch"
for tool in uname xcode-select rustup rustc xcodebuild xcrun mise kotlinc cargo; do
    ln -s dispatch "$tools/$tool"
done

export PATH="$tools:$PATH"
proof="$root/scripts/toolchain/proof.sh"
learnings="$root/specifications/69-foundation-ui-v2/learnings.md"
output="$(bash "$proof")"
for line in 'Rust: rustc proof-test' 'UniFFI: 0.32.2' 'Swift: Apple Swift version fake' \
    'Xcode: Xcode 27.2' 'iOS simulator SDK: 27.2' 'Kotlin: info: kotlinc-jvm proof-test'; do
    grep -Fx "$line" <<<"$output" >/dev/null
    grep -Fx -- "- $line" "$learnings" >/dev/null
done
grep -q -- '--language swift' "$PROOF_LOG"
grep -q -- '--language kotlin' "$PROOF_LOG"
[[ -z "$(find "$TMPDIR" -mindepth 1 -print)" ]]

cp "$learnings" "$work/expected"
bash "$proof" >/dev/null
cmp "$work/expected" "$learnings"
if FAIL_KOTLIN=1 bash "$proof" >/dev/null 2>&1; then
    printf 'FAIL proof succeeds when Kotlin compilation fails\n' >&2
    exit 1
fi
cmp "$work/expected" "$learnings"
[[ -z "$(find "$TMPDIR" -mindepth 1 -print)" ]]
apple_log="$work/apple.log"
apple_output="$(PROOF_LOG="$apple_log" GRADLE_FIXTURE="$work/missing-gradle" FAIL_KOTLIN=1 bash "$proof" --scope apple)"
grep -Fx 'Swift: Apple Swift version fake' <<<"$apple_output" >/dev/null
grep -Fx 'iOS simulator SDK: 27.2' <<<"$apple_output" >/dev/null
grep -q -- '--language swift' "$apple_log"
if grep -Eq '^mise |^kotlinc |^sdkmanager |--language kotlin' "$apple_log" || grep -q '^Kotlin:' <<<"$apple_output"; then
    printf 'FAIL Apple proof invokes a Kotlin or Android tool\n' >&2
    exit 1
fi
cmp "$work/expected" "$learnings"
[[ -z "$(find "$TMPDIR" -mindepth 1 -print)" ]]
printf 'PASS UniFFI proof\n'
