#!/bin/bash
# platform-images: provision a macOS VM for CI (layer 1), then run every
# repository layer (layers/<name>/install.sh next to this script, in name
# order). Run over SSH by `vm.nu provision` after the one-time manual
# install, as the admin user, which needs passwordless sudo (README).
# bash 3.2 compatible (what macOS ships).
#
# Installs (pins in components.env, tools in mise/):
#   Xcode Command Line Tools (clang, git, make, the macOS SDK)
#   mise + the shared tools (nushell, node, python, cmake, ninja, protoc, ...)
#   rustup without a toolchain (RUSTUP_HOME/CARGO_HOME under /usr/local/rust)
#   Vulkan SDK (MoltenVK, glslc; VULKAN_SDK)
#   the GitHub Actions runner (/usr/local/actions-runner)
# and writes /usr/local/platform-images/env.sh, sourced from /etc/zshenv and
# /etc/profile, so every shell (SSH commands included) sees the tools.
#
# Outcome: /usr/local/platform-images/status/provisioned.ok, or
# provision.failed with the log in provision.log.

set -euo pipefail

GUEST=$(cd "$(dirname "$0")" && pwd)
ROOT=/usr/local/platform-images
STATUS=$ROOT/status
DOWNLOADS=$ROOT/downloads

log() { printf '[platform-images %s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "ERROR: $*"; sudo mkdir -p "$STATUS"; echo "$*" | sudo tee "$STATUS/provision.failed" >/dev/null; exit 1; }
trap 'fail "line $LINENO: $BASH_COMMAND exited with $?"' ERR

[ "$(uname -s)" = Darwin ] || fail "provision.sh is for macOS"
[ "$(uname -m)" = x86_64 ] || fail "components.env pins x86_64 downloads, this is $(uname -m)"
sudo -n true 2>/dev/null || fail "the provisioning user needs passwordless sudo (see the README)"

# shellcheck source=components.env source-path=SCRIPTDIR
. "$GUEST/components.env"

sudo mkdir -p "$ROOT" "$STATUS" "$DOWNLOADS"
sudo chown -R "$(id -un)" "$ROOT"
rm -f "$STATUS/provisioned.ok" "$STATUS/provision.failed"

download() { # download <name> <url> <file>
  local attempt=1
  log "downloading $1 $2"
  until curl -fsSL -o "$3" "$2"; do
    [ "$attempt" -ge 3 ] && fail "download of $1 failed"
    attempt=$((attempt + 1)); sleep $((10 * attempt))
  done
}
check_sha256() { # check_sha256 <name> <file> <sha256>
  local actual
  actual=$(shasum -a 256 "$2" | cut -d' ' -f1)
  [ "$actual" = "$3" ] || fail "$1: sha256 $actual does not match the pinned $3"
  log "verified $1"
}

# --- Xcode Command Line Tools -------------------------------------------------
if ! xcode-select -p >/dev/null 2>&1; then
  log "Xcode Command Line Tools"
  # softwareupdate only lists the CLT while this marker exists.
  touch /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress
  # softwareupdate lists the newest Command Line Tools last.
  label=$(softwareupdate -l 2>/dev/null | sed -n 's/^\* Label: \(Command Line Tools for Xcode.*\)$/\1/p' | tail -1)
  [ -n "$label" ] || fail "softwareupdate offers no Command Line Tools"
  sudo softwareupdate -i "$label" --verbose
  rm -f /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress
  xcode-select -p >/dev/null 2>&1 || fail "the Command Line Tools did not install"
fi

# --- environment shared by every shell ----------------------------------------
cat > "$ROOT/env.sh" <<EOF
# platform-images environment (written by provision.sh)
export MISE_DATA_DIR=$ROOT/mise-data
export MISE_CACHE_DIR=$ROOT/mise-cache
export MISE_VERSION_CHECK=0
export RUSTUP_HOME=/usr/local/rust/rustup
export CARGO_HOME=/usr/local/rust/cargo
export VULKAN_SDK=$ROOT/vulkan/$VULKAN_SDK_VERSION/macOS
export PLATFORM_IMAGES_HOME=$GUEST
export PATH="\$CARGO_HOME/bin:$ROOT/tools/bin:$ROOT/bin:\$VULKAN_SDK/bin:\$PATH"
if [ -f $ROOT/env.d.sh ]; then . $ROOT/env.d.sh; fi
EOF
for rc in /etc/zshenv /etc/profile; do
  if ! grep -q 'platform-images/env.sh' "$rc" 2>/dev/null; then
    printf '\nif [ -f %s/env.sh ]; then . %s/env.sh; fi\n' "$ROOT" "$ROOT" | sudo tee -a "$rc" >/dev/null
  fi
done
# shellcheck disable=SC1091
. "$ROOT/env.sh"
mkdir -p "$ROOT/bin" "$ROOT/tools/bin"

# --- mise and the shared tools --------------------------------------------------
download mise "$MISE_URL" "$DOWNLOADS/mise.tar.gz"
check_sha256 mise "$DOWNLOADS/mise.tar.gz" "$MISE_SHA256"
tar -xzf "$DOWNLOADS/mise.tar.gz" -C "$DOWNLOADS"
install -m 0755 "$DOWNLOADS/mise/bin/mise" "$ROOT/bin/mise"

# install_mise_tools <config-dir>: locked install, wrappers into tools/bin
# (the same as the Linux image's pi-mise-install).
install_mise_tools() {
  local dir=$1 count=0 path f
  [ -f "$dir/mise.toml" ] || fail "no mise.toml in $dir"
  ( cd "$dir"
    export MISE_YES=1
    if [ -f mise.lock ]; then export MISE_LOCKED=1 MISE_PYTHON_GITHUB_ATTESTATIONS=false; fi
    mise trust --yes "$dir/mise.toml"
    mise install --yes
    mise bin-paths > "$STATUS/bin-paths.txt" )
  while IFS= read -r path; do
    [ -d "$path" ] || continue
    for f in "$path"/*; do
      if [ -f "$f" ] && [ -x "$f" ]; then
        printf '#!/bin/sh\nexec "%s" "$@"\n' "$f" > "$ROOT/tools/bin/$(basename "$f")"
        chmod 0755 "$ROOT/tools/bin/$(basename "$f")"
        count=$((count + 1))
      fi
    done
  done < "$STATUS/bin-paths.txt"
  [ "$count" -gt 0 ] || fail "no executables for $dir/mise.toml"
  log "mise tools from $dir: $count executables"
}
export -f install_mise_tools log fail
install_mise_tools "$GUEST/mise"

# --- rustup (no toolchain: repository layers install theirs) -------------------
sudo mkdir -p /usr/local/rust && sudo chown "$(id -un)" /usr/local/rust
download rustup "$RUSTUP_URL" "$DOWNLOADS/rustup-init"
check_sha256 rustup "$DOWNLOADS/rustup-init" "$RUSTUP_SHA256"
chmod +x "$DOWNLOADS/rustup-init"
"$DOWNLOADS/rustup-init" -y --no-modify-path --profile minimal --default-toolchain none

# --- Vulkan SDK (MoltenVK, glslc) ------------------------------------------------
download "Vulkan SDK" "$VULKAN_SDK_URL" "$DOWNLOADS/vulkansdk.zip"
rm -rf "$DOWNLOADS/vulkansdk" && mkdir -p "$DOWNLOADS/vulkansdk"
unzip -q "$DOWNLOADS/vulkansdk.zip" -d "$DOWNLOADS/vulkansdk"
app=$(find "$DOWNLOADS/vulkansdk" -maxdepth 2 -name 'InstallVulkan*.app' | head -1)
[ -n "$app" ] || fail "no InstallVulkan*.app in the Vulkan SDK archive"
codesign --verify --deep --strict "$app" || fail "Vulkan SDK installer: invalid code signature"
codesign -dv --verbose=2 "$app" 2>&1 | grep -q "Authority=.*$VULKAN_SDK_SIGNER" \
  || fail "Vulkan SDK installer is not signed by $VULKAN_SDK_SIGNER"
log "verified Vulkan SDK"
installer=$(find "$app/Contents/MacOS" -type f -perm -u+x | head -1)
"$installer" --root "$ROOT/vulkan/$VULKAN_SDK_VERSION" --accept-licenses --default-answer --confirm-command install
[ -x "$VULKAN_SDK/bin/glslc" ] || fail "the Vulkan SDK has no glslc under $VULKAN_SDK"

# --- GitHub Actions runner (registered per job; see vm.nu runner) --------------
download "actions runner" "$RUNNER_URL" "$DOWNLOADS/actions-runner.tar.gz"
check_sha256 "actions runner" "$DOWNLOADS/actions-runner.tar.gz" "$RUNNER_SHA256"
sudo mkdir -p /usr/local/actions-runner && sudo chown "$(id -un)" /usr/local/actions-runner
tar -xzf "$DOWNLOADS/actions-runner.tar.gz" -C /usr/local/actions-runner
[ -x /usr/local/actions-runner/run.sh ] || fail "the actions runner has no run.sh"

# --- repository layers ------------------------------------------------------------
if [ -d "$GUEST/layers" ]; then
  for layer in "$GUEST"/layers/*/; do
    [ -f "$layer/install.sh" ] || fail "layer $layer has no install.sh"
    log "layer $(basename "$layer")"
    bash "$layer/install.sh"
    # A layer may add environment for later shells in env.d.sh.
    # shellcheck disable=SC1091
    . "$ROOT/env.sh"
  done
fi

# --- record what is installed ---------------------------------------------------
{
  for c in "clang --version" "git --version" "nu --version" "node --version" "python3 --version" "cmake --version" "rustup --version" "glslc --version"; do
    printf '%s: %s\n' "${c%% *}" "$($c 2>&1 | head -1)"
  done
} > "$STATUS/versions.txt"
cat "$STATUS/versions.txt"
echo ok > "$STATUS/provisioned.ok"
log "provisioning done"
