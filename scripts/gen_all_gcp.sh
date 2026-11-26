#!/usr/bin/env bash
# Generate all Google Cloud providers after the shared generator validates the input.
set -euo pipefail

generator=(cargo run --locked --profile uat --bin ewe_platform -- task run codegen:generate)
"${generator[@]}" gcp --features --dry-run
"${generator[@]}" gcp --features
cargo check --package foundation_deployment --features gcp
