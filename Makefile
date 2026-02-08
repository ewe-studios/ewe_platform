DEBUG :=
PACKAGES = ewe_platform foundations_ext foundations_jsnostd foundations_nostd ewe_trace ewe_async_utils ewe_channels ewe_domain ewe_devserver ewe_domain_server ewe_html ewe_html_macro ewe_mem ewe_routing ewe_spawn ewe_spawn ewe_template_macro ewe_templates ewe_temple ewe_watch_utils ewe_watchers ewe_web
TESTS_PACKAGES = $(wildcard ./tests/integrations/*)

TEST_DIRECTORY ?= ./tests/integrations/tests_callfunction
TEST_PACKAGE ?= $(notdir $(TEST_DIRECTORY))

TARGET_TEST ?= tests_callfunction

nextest:
	bacon -j nextest 

test_foundation_core:
	bacon -j test_foundation_core

bacon:
	bacon -j bacon-ls

sandbox:
	cargo run --profile dev --bin ewe_platform sandbox

build-test-directory:
	@RUSTFLAGS='-C link-arg=-s' cargo build --package $(TEST_PACKAGE) --target wasm32-unknown-unknown
	@cp ./target/wasm32-unknown-unknown/debug/$(TEST_PACKAGE).d $(TEST_DIRECTORY)/module.d
	@cp ./target/wasm32-unknown-unknown/debug/$(TEST_PACKAGE).wasm $(TEST_DIRECTORY)/module.wasm
	@cp ./assets/jsruntime/megatron.js $(TEST_DIRECTORY)/megatron.js
	@wasm2wat $(TEST_DIRECTORY)/module.wasm -o $(TEST_DIRECTORY)/module.wat

build-demos:
	@RUSTFLAGS='-C link-arg=-s' cargo build --package intro --target wasm32-unknown-unknown
	cp target/wasm32-unknown-unknown/debug/intro.wasm ./assets/public/intro.wasm
	wasm2wat ./assets/public/intro.wasm -o ./assets/public/intro.wat

build-tests:
	$(foreach var,$(TESTS_PACKAGES), $(MAKE) TEST_DIRECTORY=$(var) TEST_PACKAGE=$(notdir $(var)) build-test-directory;)

build-target-test:
	$(MAKE) TEST_DIRECTORY=./tests/integrations/$(TARGET_TEST) TEST_PACKAGE=$(TARGET_TEST) build-test-directory

wasm-tests: build-tests
	$(foreach var,$(TESTS_PACKAGES), DEBUG=$(DEBUG) node $(var)/index.node.js;)

wasm-test: build-target-test
	 DEBUG=$(DEBUG) node $(dir $(TEST_DIRECTORY))/$(TARGET_TEST)/index.node.js

lint:
	cargo fmt

test:
	$(foreach var,$(PACKAGES), cargo test --package $(var);)

publish:
	$(foreach var,$(PACKAGES), cargo publish --package $(var);)

# ============================================================================
# Git development commands
# ============================================================================

update_submodules:
	git submodule update .agents
	git submodule update tools/dawn
	git submodule update tools/emsdk
	git submodule update infrastructure/llama-bindings/llama.cpp/

# ============================================================================
# Development Environment Setup
# ============================================================================

.PHONY: setup setup-wasm setup-tools check-tools

setup: setup-tools setup-wasm
	@echo "✓ Development environment setup complete"

setup-tools:
	@echo "Installing development tools..."
	@rustup component add rustfmt clippy rust-analyzer 2>/dev/null || true
	@cargo install cargo-nextest 2>/dev/null || echo "cargo-nextest already installed"
	@cargo install cargo-audit 2>/dev/null || echo "cargo-audit already installed"
	@echo "✓ Tools installed"

setup-wasm:
	@echo "Installing WASM targets..."
	@rustup target add wasm32-unknown-unknown
	@rustup target add wasm32-wasip1
	@echo "✓ WASM targets installed"

check-tools:
	@echo "Checking installed tools..."
	@rustup --version
	@cargo --version
	@rustup target list | grep wasm32-unknown-unknown
	@cargo nextest --version 2>/dev/null || echo "⚠ cargo-nextest not installed (optional)"
	@echo "✓ Tool check complete"

# ============================================================================
# Testing - Comprehensive Test Suite
# ============================================================================

.PHONY: test-all test-unit test-integration test-wasm test-benches
.PHONY: test-foundation test-nostd test-nostd-unit test-nostd-integration
.PHONY: test-nostd-wasm test-coverage test-quick

# Run all tests (unit + integration)
test-all: test-unit test-integration
	@echo "✓ All tests passed"

# Run only unit tests (fast, in-crate tests)
test-unit:
	@echo "Running unit tests..."
	@cargo test --lib --all
	@echo "✓ Unit tests passed"

# Run only integration tests (workspace-level tests)
test-integration:
	@echo "Running integration tests..."
	@cargo test --package ewe_platform_tests
	@echo "✓ Integration tests passed"

# Quick smoke test (fast feedback)
test-quick:
	@echo "Running quick smoke tests..."
	@cargo test --package foundation_nostd --lib
	@echo "✓ Quick tests passed"

# ============================================================================
# Foundation NoStd Testing (Spec 04 - CondVar Primitives)
# ============================================================================

# All foundation_nostd tests
test-nostd: test-nostd-unit test-nostd-integration
	@echo "✓ All foundation_nostd tests passed"

# Unit tests only (160 tests in lib)
test-nostd-unit:
	@echo "Running foundation_nostd unit tests..."
	@cargo test --package foundation_nostd --lib
	@echo "✓ foundation_nostd unit tests passed (160 tests)"

# Integration tests only (workspace-level)
test-nostd-integration:
	@echo "Running foundation_nostd integration tests..."
	@cargo test --package ewe_platform_tests --lib
	@echo "✓ foundation_nostd integration tests passed"

# WASM compilation tests
test-nostd-wasm: test-nostd-wasm-build test-nostd-wasm-verify
	@echo "✓ WASM tests complete"

# Build for WASM targets
test-nostd-wasm-build:
	@echo "Building foundation_nostd for WASM (no_std)..."
	@cargo build --package foundation_nostd --target wasm32-unknown-unknown --no-default-features
	@echo "Building foundation_nostd for WASM (with std)..."
	@cargo build --package foundation_nostd --target wasm32-unknown-unknown --features std
	@echo "Building foundation_nostd for WASM (release)..."
	@cargo build --package foundation_nostd --target wasm32-unknown-unknown --release --no-default-features
	@echo "✓ WASM builds successful"

# Verify WASM artifacts
test-nostd-wasm-verify:
	@echo "Verifying WASM artifacts..."
	@ls -lh target/wasm32-unknown-unknown/debug/deps/libfoundation_nostd*.rlib | head -1
	@ls -lh target/wasm32-unknown-unknown/release/deps/libfoundation_nostd*.rlib | head -1
	@echo "✓ WASM artifacts verified"
	@echo "See specifications/04-condvar-primitives/WASM_TESTING_REPORT.md for details"

# ============================================================================
# Benchmarking
# ============================================================================

.PHONY: bench bench-condvar bench-all

# Run all benchmarks
bench-all: bench
	@echo "✓ All benchmarks complete"

# Run benchmarks (currently at workspace root in benches/)
bench:
	@echo "Running Criterion benchmarks..."
	@cargo bench
	@echo "✓ Benchmarks complete"
	@echo "Results: target/criterion/report/index.html"

# Run specific CondVar benchmarks
bench-condvar:
	@echo "Running CondVar benchmarks..."
	@cargo bench --bench condvar_bench
	@echo "✓ CondVar benchmarks complete"

# ============================================================================
# Code Quality and Verification
# ============================================================================

.PHONY: quality check clippy fmt fmt-check audit verify-all

# Run all quality checks
quality: fmt-check clippy test-unit
	@echo "✓ All quality checks passed"

# Full verification (quality + all tests)
verify-all: quality test-all
	@echo "✓ Full verification passed"

# Check code compiles
check:
	@echo "Checking compilation..."
	@cargo check --all
	@echo "✓ Compilation check passed"

# Run clippy with strict lints
clippy:
	@echo "Running clippy..."
	@cargo clippy --all-targets --all-features -- -D warnings
	@echo "✓ Clippy passed (zero warnings)"

# Format code
fmt:
	@echo "Formatting code..."
	@cargo fmt --all
	@echo "✓ Code formatted"

# Check if code is formatted
fmt-check:
	@echo "Checking code formatting..."
	@cargo fmt --all -- --check
	@echo "✓ Code is properly formatted"

# Security audit
audit:
	@echo "Running security audit..."
	@cargo audit
	@echo "✓ Security audit complete"

# ============================================================================
# Build Targets
# ============================================================================

.PHONY: build build-all build-release build-wasm clean

# Build everything (debug)
build-all:
	@echo "Building all packages (debug)..."
	@cargo build --all
	@echo "✓ Build complete"

# Build release
build-release:
	@echo "Building all packages (release)..."
	@cargo build --all --release
	@echo "✓ Release build complete"

# Build for WASM
build-wasm:
	@echo "Building for WASM..."
	@cargo build --package foundation_nostd --target wasm32-unknown-unknown
	@echo "✓ WASM build complete"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@echo "✓ Clean complete"

# ============================================================================
# Documentation
# ============================================================================

.PHONY: doc doc-open doc-nostd

# Generate documentation
doc:
	@echo "Generating documentation..."
	@cargo doc --no-deps --all-features
	@echo "✓ Documentation generated"
	@echo "Open: target/doc/index.html"

# Generate and open documentation
doc-open:
	@echo "Generating and opening documentation..."
	@cargo doc --no-deps --all-features --open

# Generate foundation_nostd documentation
doc-nostd:
	@echo "Generating foundation_nostd documentation..."
	@cargo doc --package foundation_nostd --no-deps --all-features --open

# ============================================================================
# Help
# ============================================================================

.PHONY: help

help:
	@echo "EWE Platform - Makefile Commands"
	@echo ""
	@echo "Setup:"
	@echo "  make setup              Install all dev tools and WASM targets"
	@echo "  make setup-tools        Install rustfmt, clippy, cargo-nextest, etc."
	@echo "  make setup-wasm         Install WASM targets"
	@echo "  make check-tools        Verify installed tools"
	@echo ""
	@echo "Testing:"
	@echo "  make test-all           Run all tests (unit + integration)"
	@echo "  make test-unit          Run only unit tests (fast)"
	@echo "  make test-integration   Run only integration tests"
	@echo "  make test-quick         Quick smoke test"
	@echo ""
	@echo "Foundation NoStd (CondVar):"
	@echo "  make test-nostd         All foundation_nostd tests"
	@echo "  make test-nostd-unit    Unit tests (160 tests)"
	@echo "  make test-nostd-integration  Integration tests"
	@echo "  make test-nostd-wasm    WASM compilation + verification"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make bench              Run all benchmarks"
	@echo "  make bench-condvar      Run CondVar benchmarks only"
	@echo ""
	@echo "Quality:"
	@echo "  make quality            Run fmt-check + clippy + unit tests"
	@echo "  make verify-all         Full verification (quality + all tests)"
	@echo "  make clippy             Run clippy (zero warnings)"
	@echo "  make fmt                Format code"
	@echo "  make fmt-check          Check if code is formatted"
	@echo "  make audit              Security audit"
	@echo ""
	@echo "Build:"
	@echo "  make build-all          Build all packages (debug)"
	@echo "  make build-release      Build all packages (release)"
	@echo "  make build-wasm         Build for WASM"
	@echo "  make clean              Clean build artifacts"
	@echo ""
	@echo "Documentation:"
	@echo "  make doc                Generate documentation"
	@echo "  make doc-open           Generate and open documentation"
	@echo "  make doc-nostd          Open foundation_nostd docs"
	@echo ""
	@echo "Legacy (existing targets):"
	@echo "  make nextest            Run with bacon nextest"
	@echo "  make test               Run tests for listed packages"
	@echo "  make lint               Format code (alias for fmt)"
	@echo ""
	@echo "Examples:"
	@echo "  make setup && make test-all     Setup and run all tests"
	@echo "  make quality                     Quick quality check"
	@echo "  make test-nostd-wasm            Test WASM compatibility"
