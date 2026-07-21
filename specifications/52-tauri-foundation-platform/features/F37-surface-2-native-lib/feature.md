# F37 — Surface 2: native static library

**Resolution: No work required.** Surface 2 is already available.

Users who need native performance add their crate as a dependency to
`src-tauri/Cargo.toml` and call it directly. The compiler handles
`.a`/`.so` linking automatically — this is standard Rust crate linking,
not a platform feature. No annotation, no codegen, no build.rs changes.

**Status: Deferred (already possible, zero platform work needed).**
