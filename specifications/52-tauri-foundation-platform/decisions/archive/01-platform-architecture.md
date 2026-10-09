# Decision 01: Platform Architecture

## Context
We need a cross-platform foundation that supports desktop, mobile, and web. Tauri is our chosen framework.

## Decision
- We will create a new crate `foundation_platform` to own the Tauri integration.
- This crate will act as the bridge between our Rust backend and the frontend (WASM UI).
- We will adopt a server-rendered approach inspired by Basecamp's Hotwire Native where appropriate.

## Rationale
- Tauri provides a robust cross-platform abstraction.
- A dedicated crate ensures separation of concerns and modularity.
- Hotwire Native patterns offer a proven path for hybrid native/web applications.
