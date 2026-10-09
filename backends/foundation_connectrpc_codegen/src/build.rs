//! Build.rs helper for ConnectRPC code generation (Decision 10 Mode 1).
//!
//! WHY: Users need a simple `build.rs` API that compiles `.proto` files and
//! generates both message types (via prost-build) and ConnectRPC service stubs
//! (via [`generate_services`]).
//!
//! WHAT: [`Config`] builder with `files`, `includes`, and `out_dir` settings.
//! Calling `compile()` runs prost-build to generate message types and the
//! connectrpc-codegen to generate service traits, registration functions, and
//! typed clients — all in one pass.
//!
//! HOW: Configures prost-build to compile `.proto` files and emit a file
//! descriptor set. Reads the descriptor set back, passes it to
//! [`super::generate_services`], and writes the result to
//! `_connectrpc.rs` in the output directory.

use std::path::PathBuf;
use std::str::FromStr;

use prost::Message;

/// Builder for protobuf compilation and ConnectRPC code generation.
pub struct Config {
    files: Vec<String>,
    includes: Vec<String>,
    out_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Create a new compilation config with empty defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            includes: Vec::new(),
            out_dir: None,
        }
    }

    /// Set the `.proto` source files to compile.
    #[must_use]
    pub fn files(mut self, files: &[&str]) -> Self {
        self.files = files.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set the include directories for proto imports.
    #[must_use]
    pub fn includes(mut self, dirs: &[&str]) -> Self {
        self.includes = dirs.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Override the output directory (defaults to `OUT_DIR` env var).
    #[must_use]
    pub fn out_dir(mut self, dir: &str) -> Self {
        self.out_dir = Some(PathBuf::from_str(dir).expect("valid path"));
        self
    }

    /// Compile `.proto` files and generate ConnectRPC service code.
    ///
    /// 1. Uses prost-build to compile `.proto` → message types and a file
    ///    descriptor set.
    /// 2. Passes the descriptor set to `codegen::generate_services`.
    /// 3. Writes the generated code to `_connectrpc.rs` in `out_dir`.
    ///
    /// # Errors
    /// Returns a boxed error if prost-build compilation fails, the descriptor
    /// set cannot be read or decoded, or the output file cannot be written.
    pub fn compile(self) -> Result<(), Box<dyn std::error::Error>> {
        let out_dir = self
            .out_dir
            .unwrap_or_else(|| PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string())));

        // ── Configure prost-build ───────────────────────────────────────────
        let mut config = prost_build::Config::new();

        // Set output directory
        config.out_dir(out_dir.clone());

        // Enable file descriptor set emission
        let descriptor_path = out_dir.join("file_descriptor_set.bin");
        config.file_descriptor_set_path(descriptor_path.clone());

        // Compile proto files
        config.compile_protos(&self.files, &self.includes)?;

        // ── Read descriptor set ─────────────────────────────────────────────
        let buf = std::fs::read(&descriptor_path)?;
        let descriptor_set = prost_types::FileDescriptorSet::decode(buf.as_slice())?;

        // ── Generate ConnectRPC service code ────────────────────────────────
        let connectrpc_code =
            super::generate_services(&descriptor_set.file);

        // ── Write output ────────────────────────────────────────────────────
        let out_file = out_dir.join("_connectrpc.rs");
        std::fs::write(&out_file, connectrpc_code)?;

        // Cleanup: remove the intermediate descriptor set file
        let _ = std::fs::remove_file(&descriptor_path);

        Ok(())
    }
}
