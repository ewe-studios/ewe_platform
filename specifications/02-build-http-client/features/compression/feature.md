---
# Identification
spec_name: "02-build-http-client"
spec_number: 02
feature_name: "compression"
feature_number: 3
description: Automatic compression/decompression of HTTP bodies with gzip, deflate, and brotli support

# Location Context
# How to find: Run `bash pwd` to get CWD, this file is at CWD/specifications/02-build-http-client/features/compression/feature.md
# Workspace root is CWD and contains: .agents/, specifications/, documentation/, backends/
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/compression"
this_file: "specifications/02-build-http-client/features/compression/feature.md"

# Status
status: pending
priority: medium
depends_on:
  - foundation
estimated_effort: small
created: 2026-01-19
last_updated: 2026-02-02
author: Main Agent

# Context Optimization
machine_optimized: true
machine_prompt_file: ./machine_prompt.md
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: true

# Tasks
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0

# Files Required by Agents
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
---

# Compression Feature

## 📍 Location Reference

**How to find your location**:
1. Run `bash pwd` to get current working directory (CWD)
2. This file is at: `CWD/specifications/02-build-http-client/features/compression/feature.md`
3. Workspace root is CWD (contains `.agents/`, `specifications/`, `documentation/`, `backends/`)

**Quick paths** (all relative to workspace root = CWD):
- Parent spec: `specifications/02-build-http-client/requirements.md`
- This feature: `specifications/02-build-http-client/features/compression/`
- This file: `specifications/02-build-http-client/features/compression/feature.md`
- Machine prompt: `specifications/02-build-http-client/features/compression/machine_prompt.md`
- Parent progress: `specifications/02-build-http-client/PROGRESS.md`
- Parent learnings: `specifications/02-build-http-client/LEARNINGS.md`
- Agent rules: `.agents/rules/`
- Stack files: `.agents/stacks/rust.md`
- HTTP client code: `backends/foundation_core/src/wire/simple_http/client/`

**Verification**: If you can read `.agents/AGENTS.md` from CWD, you're in the right place!

**Quick Navigation Commands**:
```bash
# Verify you're in workspace root
test -f .agents/AGENTS.md && echo "✓ In workspace root" || echo "✗ Wrong location"

# View parent spec
cat specifications/02-build-http-client/requirements.md

# List all features in parent spec
ls -d specifications/02-build-http-client/features/*/

# Check foundation feature (dependency)
cat specifications/02-build-http-client/features/foundation/feature.md

# View this feature's structure
tree specifications/02-build-http-client/features/compression/

# Find HTTP client code
find backends/foundation_core/src/wire/simple_http/client/ -type f -name "*.rs"
```

---

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this feature MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** in related modules to understand patterns
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific conventions
4. ✅ **Read parent specification** (`../requirements.md`) for high-level context
5. ✅ **Read module documentation** for modules this feature touches
6. ✅ **Check dependencies** by reading other feature files referenced in `depends_on`
7. ✅ **Follow discovered patterns** consistently with existing codebase

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume patterns based on typical practices without checking this codebase
- ❌ Implement without searching for similar features first
- ❌ Apply generic solutions without verifying project conventions
- ❌ Guess at naming conventions, file structures, or patterns
- ❌ Use pretraining knowledge without validating against actual project code

### Retrieval Checklist

Before implementing, answer these questions by reading code:
- [ ] What similar features exist in this project? (use Grep to find)
- [ ] What patterns do they follow? (read their implementations)
- [ ] What naming conventions are used? (observed from existing code)
- [ ] How are errors handled in similar code? (check error patterns)
- [ ] What testing patterns exist? (read existing test files)
- [ ] Are there existing helper functions I can reuse? (search thoroughly)

### Enforcement

- Show your retrieval steps in your work report
- Reference specific files/patterns you discovered
- Explain how your implementation matches existing patterns
- "I assumed..." responses will be rejected - only "I found in [file]..." accepted

---

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification/feature MUST follow token and context optimization protocols.**

### Machine-Optimized Prompts (Rule 14)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification/feature finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

### Context Compaction (Rule 15)

**Sub-Agents MUST** (before starting work):
1. Read machine_prompt.md and PROGRESS.md
2. Generate `COMPACT_CONTEXT.md`:
   - Embed machine_prompt.md content for current task
   - Extract current status from PROGRESS.md
   - List files for current task only (500-800 tokens)
3. CLEAR entire context
4. RELOAD from COMPACT_CONTEXT.md only
5. Proceed with 97% context reduction (180K→5K tokens)

**After PROGRESS.md Updates**:
- Regenerate COMPACT_CONTEXT.md (re-embed machine_prompt content)
- Clear and reload
- Maintain minimal context

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task
- Contains ONLY current task (no history)
- Deleted when task completes
- Rewritten from scratch for next task

**See**:
- Rule 14: .agents/rules/14-machine-optimized-prompts.md
- Rule 15: .agents/rules/15-instruction-compaction.md

---

## Overview

Add automatic compression and decompression support for HTTP request/response bodies. This feature enables transparent handling of gzip, deflate, and brotli compressed content, following HTTP content negotiation standards.

## Dependencies

This feature depends on:
- `foundation` - Uses HttpClientError for error handling

This feature is required by:
- `public-api` - Exposes compression configuration to users

## Requirements

### Auto Accept-Encoding Header

Automatically add `Accept-Encoding` header to requests:

```rust
// When compression is enabled (default)
Accept-Encoding: gzip, deflate, br

// Header is added automatically unless:
// 1. Compression is disabled
// 2. User has already set Accept-Encoding header
```

### Automatic Decompression

Decompress response bodies based on `Content-Encoding` header:

```rust
pub enum ContentEncoding {
    Identity,    // No compression
    Gzip,        // gzip compression
    Deflate,     // deflate compression
    Brotli,      // br (brotli) compression
    Unknown(String),
}

impl ContentEncoding {
    pub fn from_header(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "gzip" => Self::Gzip,
            "deflate" => Self::Deflate,
            "br" => Self::Brotli,
            "identity" => Self::Identity,
            other => Self::Unknown(other.to_string()),
        }
    }
}
```

### Streaming Decompression

Decompression MUST be iterator-based, not buffered:

```rust
pub struct DecompressingReader<R: Read> {
    inner: DecompressorKind<R>,
}

enum DecompressorKind<R: Read> {
    Identity(R),
    Gzip(GzDecoder<R>),
    Deflate(DeflateDecoder<R>),
    Brotli(BrotliDecoder<R>),
}

impl<R: Read> Read for DecompressingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            DecompressorKind::Identity(r) => r.read(buf),
            DecompressorKind::Gzip(d) => d.read(buf),
            DecompressorKind::Deflate(d) => d.read(buf),
            DecompressorKind::Brotli(d) => d.read(buf),
        }
    }
}
```

### Feature-Gated Dependencies

Compression libraries are optional:

```toml
[dependencies]
flate2 = { version = "1.0", optional = true }
brotli = { version = "6.0", optional = true }

[features]
default = ["compression"]
compression = ["flate2", "brotli"]
gzip = ["flate2"]
brotli = ["brotli"]
```

### Configuration API

```rust
// Client-level configuration
let client = SimpleHttpClient::new()
    .compression(true)  // Enable compression (default)
    .compression(false); // Disable compression

// Per-request override
let response = client.get(url)
    .no_compression()  // Disable for this request
    .send()?;

// Access both raw and decompressed body
let response = client.get(url).send()?;
response.body();      // Auto-decompressed
response.raw_body();  // Original compressed bytes
```

### Error Handling

Add compression-related error variants:

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    #[from(ignore)]
    DecompressionFailed(String),

    #[from(ignore)]
    UnsupportedEncoding(String),

    #[from]
    IoError(std::io::Error),
}
```

## Implementation Details

### File Structure

```
client/
├── compression.rs    (NEW - Compression/decompression logic)
└── ...
```

### CompressionConfig

```rust
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    /// Enable automatic Accept-Encoding header
    pub add_accept_encoding: bool,

    /// Enable automatic decompression
    pub auto_decompress: bool,

    /// Supported encodings (in preference order)
    pub supported_encodings: Vec<ContentEncoding>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            add_accept_encoding: true,
            auto_decompress: true,
            supported_encodings: vec![
                ContentEncoding::Brotli,
                ContentEncoding::Gzip,
                ContentEncoding::Deflate,
            ],
        }
    }
}
```

### Accept-Encoding Header Generation

```rust
impl CompressionConfig {
    pub fn accept_encoding_value(&self) -> String {
        self.supported_encodings
            .iter()
            .filter_map(|e| match e {
                ContentEncoding::Gzip => Some("gzip"),
                ContentEncoding::Deflate => Some("deflate"),
                ContentEncoding::Brotli => Some("br"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}
```

### Integration Points

1. **Request Building**: Add Accept-Encoding header if compression enabled
2. **Response Reading**: Wrap response body reader with DecompressingReader
3. **ClientResponse**: Expose both `body()` and `raw_body()` methods

## Success Criteria

- [ ] `compression.rs` exists and compiles
- [ ] `ContentEncoding` enum correctly parses header values
- [ ] `DecompressingReader` implements streaming decompression
- [ ] Gzip decompression works correctly
- [ ] Deflate decompression works correctly
- [ ] Brotli decompression works correctly (feature-gated)
- [ ] Accept-Encoding header is auto-added when compression enabled
- [ ] Per-request compression override works
- [ ] `response.body()` returns decompressed content
- [ ] `response.raw_body()` returns original bytes
- [ ] Compression can be disabled client-wide
- [ ] Feature gates work correctly
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- compression
cargo build --package foundation_core
cargo build --package foundation_core --features compression
cargo build --package foundation_core --no-default-features
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** foundation feature is complete
- **MUST READ** flate2 documentation for GzDecoder, DeflateDecoder
- **MUST READ** brotli crate documentation for BrotliDecoder
- **MUST CHECK** existing simple_http structures for integration points

### Implementation Guidelines
- Use streaming (Read trait) for decompression, NOT buffering
- Feature gate brotli separately (larger dependency)
- Handle unknown encodings gracefully (return raw body)
- Preserve original body access via `raw_body()`
- Follow existing error patterns with derive_more::From

### Testing Considerations
- Test with real compressed content
- Test streaming behavior (not just final result)
- Test fallback when compression dependencies not available
- Test per-request override behavior

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
