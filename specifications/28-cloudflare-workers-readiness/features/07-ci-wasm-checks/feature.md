---
feature: "CI Wasm Checks"
description: "Add CI pipeline step for wasm32-unknown-unknown compilation checks on PRs"
status: "pending"
priority: "medium"
depends_on: ["06-example-app"]
estimated_effort: "small"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0%
---

# Feature: CI Wasm Checks

## Overview

Add a CI job that verifies all foundation crates compile cleanly on `wasm32-unknown-unknown` target. This catches wasm compatibility regressions before they land.

## Implementation

### GitHub Actions Job

```yaml
wasm-check:
  name: Wasm32 Compilation Check
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4

    - name: Install Rust wasm target
      run: rustup target add wasm32-unknown-unknown

    - name: Check foundation_core
      run: >
        cargo check -p foundation_core --target wasm32-unknown-unknown
        --no-default-features
        --features ssl-rustls-awsrc,std

    - name: Check foundation_auth
      run: >
        cargo check -p foundation_auth --target wasm32-unknown-unknown
        --no-default-features
        --features foundation_core/ssl-rustls-awsrc,foundation_core/std

    - name: Check foundation_http
      run: >
        cargo check -p foundation_http --target wasm32-unknown-unknown
        --no-default-features
        --features foundation_core/ssl-rustls-awsrc,foundation_core/std

    - name: Check foundation_db
      run: >
        cargo check -p foundation_db --target wasm32-unknown-unknown
        --no-default-features
        --features d1,r2,foundation_core/ssl-rustls-awsrc,foundation_core/std
```

### Integration

- Add to `.github/workflows/ci.yml` as a parallel job alongside existing checks
- Set as a required check for PR merges (optional, at user discretion)

## Tasks

1. [ ] Add `wasm-check` job to `.github/workflows/ci.yml`
2. [ ] Test CI workflow on a PR branch
3. [ ] Add to required status checks in branch protection (optional)

## Verification

```bash
# Manual equivalent
rustup target add wasm32-unknown-unknown
cargo check --target wasm32-unknown-unknown --workspace \
  --no-default-features --features ssl-rustls-awsrc,std 2>&1
```

---

_Created: 2026-05-15_
