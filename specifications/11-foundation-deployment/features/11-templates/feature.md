---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/11-templates"
this_file: "specifications/11-foundation-deployment/features/11-templates/feature.md"

status: rejected
priority: high
created: 2026-03-26
updated: 2026-04-11

depends_on: []

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---


# Composable Templates

## Status: REJECTED

**This feature is rejected as a configuration-generation approach.**

**Reason:** Templates that generate YAML deployment plans contradict the core principle of Feature 35 (Trait-Based Deployments). Users should write **Rust code**, not generated YAML configs.

### What remains valid

- **Project scaffolding** for creating initial Rust project structure (`Cargo.toml`, basic `src/main.rs`) is still useful
- **Example code snippets** in documentation showing `Deployable` implementations
- **NOT** YAML deployment plan generation

