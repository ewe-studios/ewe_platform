# scripts/setup/common.sh — shared, platform-independent setup steps.
#
# Sourced (not executed) by scripts/setup/{macos,archlinux}.sh. The caller
# must already have `cd`'d into the repository root. bash 3.2 safe (what macOS
# ships): no associative arrays, no `mapfile`, no `${var,,}`.

install_rust_components() {
    echo "Installing Rust components (rustfmt, clippy, rust-analyzer, cranelift)..."
    rustup component add rustfmt clippy rust-analyzer rustc-codegen-cranelift 2>/dev/null || true
    echo "✓ Rust components installed"
}

install_wasm_targets() {
    echo "Installing WASM targets..."
    rustup target add wasm32-unknown-unknown
    rustup target add wasm32-unknown-emscripten
    rustup target add wasm32-wasip1
    rustup target add wasm32-wasip2
    echo "✓ WASM targets installed (unknown-unknown, emscripten, wasip1, wasip2)"
}

# Path-scoped on purpose: `git submodule update --init` without a path aborts on
# the stray `tools/depot_tools` gitlink that has no .gitmodules entry.
init_submodules() {
    echo "Initializing llama.cpp submodule..."
    git submodule update --init --recursive tools/llama.cpp
    echo "Initializing emsdk submodule..."
    git submodule update --init --recursive tools/emsdk

    if [ ! -f tools/llama.cpp/include/llama.h ]; then
        echo "ERROR: tools/llama.cpp/include/llama.h is missing after submodule init" >&2
        echo "The llama.cpp submodule did not populate correctly." >&2
        exit 1
    fi
    echo "✓ Submodules ready"
}

install_pnpm_deps() {
    echo "Installing pnpm dependencies..."
    pnpm install
    echo "✓ pnpm dependencies installed"
}
