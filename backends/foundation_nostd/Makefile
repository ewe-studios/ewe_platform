# Foundation CondVar Testing Makefile
#
# Provides convenient commands for running tests, benchmarks, and builds
# across different configurations.

.PHONY: all test test-nostd test-std bench stress clean help test-wasm-build

# Default target
all: test

# Run all tests
test: test-std test-docs
	@echo "✓ All tests passed"

# Run tests with std feature (default)
test-std:
	@echo "Running tests with std feature..."
	cargo test --package foundation_nostd --features std
	cargo test --package foundation_testing

# Run tests without std (no_std mode)
test-nostd:
	@echo "Running tests in no_std mode..."
	cargo test --package foundation_nostd --no-default-features

# Run documentation tests
test-docs:
	@echo "Running documentation tests..."
	cargo test --package foundation_nostd --doc
	cargo test --package foundation_testing --doc

# Run benchmarks
bench:
	@echo "Running benchmarks..."
	cargo bench --package foundation_testing

# Run stress tests specifically
stress:
	@echo "Running stress tests..."
	cargo test --package foundation_testing --release -- stress

# Build everything
build:
	@echo "Building all packages..."
	cargo build --package foundation_nostd
	cargo build --package foundation_testing

# Build for WASM target (requires wasm32-unknown-unknown target)
build-wasm:
	@echo "Building for WASM target..."
	cargo build --package foundation_nostd --target wasm32-unknown-unknown --no-default-features

# Test WASM build (compile only, as WASM tests require wasm-bindgen-test)
test-wasm-build:
	@echo "Building WASM targets..."
	cargo build --package foundation_nostd --target wasm32-unknown-unknown --no-default-features
	cargo build --package foundation_nostd --target wasm32-unknown-unknown --no-default-features --tests
	@echo "✓ WASM builds successful"
	@echo "Note: WASM runtime tests require wasm-bindgen-test-runner"
	@echo "      foundation_testing cannot be built for WASM (depends on criterion with rayon)"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Check code with clippy
clippy:
	@echo "Running clippy..."
	cargo clippy --package foundation_nostd --features std -- -D warnings
	cargo clippy --package foundation_testing -- -D warnings

# Format code
fmt:
	@echo "Formatting code..."
	cargo fmt --package foundation_nostd
	cargo fmt --package foundation_testing

# Check formatting
fmt-check:
	@echo "Checking code formatting..."
	cargo fmt --package foundation_nostd -- --check
	cargo fmt --package foundation_testing -- --check

# Run all quality checks
quality: fmt-check clippy test
	@echo "✓ All quality checks passed"

# Display help
help:
	@echo "Foundation CondVar Testing Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  make test          - Run all tests (default)"
	@echo "  make test-std      - Run tests with std feature"
	@echo "  make test-nostd    - Run tests in no_std mode"
	@echo "  make test-docs     - Run documentation tests"
	@echo "  make test-wasm-build - Build WASM tests (compile-time verification)"
	@echo "  make bench         - Run benchmarks"
	@echo "  make stress        - Run stress tests"
	@echo "  make build         - Build all packages"
	@echo "  make build-wasm    - Build for WASM target"
	@echo "  make clippy        - Run clippy linter"
	@echo "  make fmt           - Format code"
	@echo "  make fmt-check     - Check code formatting"
	@echo "  make quality       - Run all quality checks"
	@echo "  make clean         - Clean build artifacts"
	@echo "  make help          - Display this help message"
