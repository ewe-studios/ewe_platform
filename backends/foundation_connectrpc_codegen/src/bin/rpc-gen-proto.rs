//! Protoc plugin for ConnectRPC code generation (Decision 10 Mode 2).
//!
//! Implements the protoc plugin protocol:
//! 1. Reads `CodeGeneratorRequest` from stdin (protobuf binary).
//! 2. Calls `foundation_connectrpc_codegen::generate_services` on the
//!    file descriptors.
//! 3. Writes `CodeGeneratorResponse` to stdout (protobuf binary).
//!
//! Usage — the binary is `rpc-gen-proto`, which is not a `protoc-gen-*` name, so
//! map it explicitly with `--plugin=protoc-gen-<name>=<path>`:
//! ```sh
//! protoc --connect-ewe_out=. \
//!        --plugin=protoc-gen-connect-ewe="$(command -v rpc-gen-proto)" \
//!        service.proto
//! ```

use std::io::{Read, Write};

use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Read CodeGeneratorRequest from stdin ──────────────────────────────
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf)?;
    let request =
        prost_types::compiler::CodeGeneratorRequest::decode(buf.as_slice())?;

    // ── Generate ConnectRPC service code ──────────────────────────────────
    let code =
        foundation_connectrpc_codegen::generate_services(&request.proto_file);

    // ── Build CodeGeneratorResponse ───────────────────────────────────────
    let response = prost_types::compiler::CodeGeneratorResponse {
        file: vec![prost_types::compiler::code_generator_response::File {
            name: Some("_connectrpc.rs".to_string()),
            content: Some(code),
            ..Default::default()
        }],
        ..Default::default()
    };

    // ── Write CodeGeneratorResponse to stdout ─────────────────────────────
    let mut out = Vec::new();
    response.encode(&mut out)?;
    std::io::stdout().write_all(&out)?;

    Ok(())
}
