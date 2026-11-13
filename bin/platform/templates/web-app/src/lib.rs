// Entrypoint to the http-app crate library.
//
// The modules are public, like `App` in the api-app template, so the
// generated project's library exposes them to its binaries and tests. Keep
// them reachable: a private module with unused items builds with warnings.

pub mod app;
pub mod config;
