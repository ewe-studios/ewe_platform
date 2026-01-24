---
module: foundation_core
language: rust
status: active
last_updated: 2026-01-14
maintainer: ewe-platform team
related_specs: []
---

# foundation_core - Documentation

## Overview
`foundation_core` is the central foundational crate for the ewe-platform project, providing core utilities, abstractions, and cross-platform compatibility layers. It serves as the primary dependency for all other foundation crates and provides essential functionality for I/O, networking, async execution, tracing, and platform-specific abstractions.

## Purpose and Responsibility
This crate acts as the central hub for all foundation-level functionality in the ewe-platform ecosystem. Its primary responsibilities include:
- Providing platform-agnostic abstractions (WASM, nostd, std)
- Offering core utilities for I/O operations, memory management, and data serialization
- Implementing networking capabilities (HTTP, SSL/TLS, TCP connections)
- Providing async execution primitives and runtime abstractions
- Offering retry mechanisms and error handling utilities
- Implementing conditional tracing/logging based on feature flags

## Module Location
- **Path**: `backends/foundation_core/`
- **Entry Point**: `src/lib.rs`
- **Language**: Rust 2021 edition
- **Package Manager**: Cargo
- **Version**: 0.0.3

## What It Implements

### Core Modules

#### 1. **compati** (Lines 6)
- **What**: Platform compatibility layer for synchronization primitives
- **Why**: Enables code to work across WASM and native platforms with appropriate sync types
- **How**: Conditional compilation (`cfg`) to export either `foundation_nostd::spin` or `std::sync` types
- **Key Types**: `Mutex`, `RwLock`

#### 2. **extensions** (Lines 7)
- **What**: Extension traits for standard library types
- **Why**: Adds convenient functionality to existing types without modifying them
- **How**: Trait implementations for Result, Serde, String, Tokio, Vec types
- **Sub-modules**:
  - `result_ext`: Extensions for Result types
  - `serde_ext`: Serialization/deserialization helpers
  - `strings_ext`: String manipulation utilities
  - `tokio_ext`: Tokio-specific async utilities
  - `vec_ext`: Vector operation extensions

#### 3. **io** (Lines 8)
- **What**: I/O utilities and memory management
- **Why**: Provides efficient I/O operations and memory handling primitives
- **How**: Custom memory management, byte buffer operations, I/O utilities
- **Sub-modules**:
  - `ioutils`: Generic I/O helper functions
  - `mem`: Memory management, encoding, and pointer operations
  - `ubytes`: Unsigned byte array utilities

#### 4. **macros** (Lines 9)
- **What**: Utility macros for common operations
- **Why**: Reduces boilerplate and provides compile-time code generation
- **How**: Declarative macro definitions
- **Sub-modules**:
  - `collections`: Collection creation macros
  - `expects`: Expectation and assertion macros
  - `ioerrs`: I/O error handling macros
  - `rezzo`: Resource management macros

#### 5. **netcap** (Lines 10)
- **What**: Network capabilities including TCP, SSL/TLS, and HTTP
- **Why**: Provides unified networking interface across platforms
- **How**: Conditional compilation for WASM vs native, multiple SSL backend support
- **Sub-modules**:
  - `connection`: TCP connection management (non-WASM only)
  - `errors`: Network error types
  - `ssl`: SSL/TLS abstractions (OpenSSL, Rustls, Native-TLS)
  - `server`: Server-side networking (non-WASM only)
- **Key Features**: Multi-SSL backend support, WASM compatibility

#### 6. **retries** (Lines 11)
- **What**: Retry mechanism implementations
- **Why**: Handles transient failures in network and I/O operations
- **How**: Configurable retry strategies with backoff algorithms
- **Sub-modules**:
  - `core`: Base retry logic
  - `exponential`: Exponential backoff strategy
  - `same`: Fixed delay retry strategy

#### 7. **synca** (Lines 12)
- **What**: Synchronization and async primitives
- **Why**: Provides cross-platform async/await and synchronization constructs
- **How**: Custom implementations for signal handling, event notification, and async sleep
- **Sub-modules**:
  - `drops`: Drop guards and cleanup handlers
  - `entrylist`: Entry list data structures
  - `event`: Event notification primitives
  - `idleman`: Idle management
  - `mpp`: Multi-producer primitives
  - `signals`: Signal handling
  - `sleepers`: Async sleep utilities

#### 8. **trace** (Lines 13)
- **What**: Conditional tracing/logging macros
- **Why**: Zero-cost logging abstraction that can be completely compiled out
- **How**: Feature-flag based macro definitions that expand to tracing calls or no-ops
- **Key Macros**: `info!`, `warn!`, `error!`, `debug!`
- **Features**: Controlled by `log_info`, `log_warnings`, `log_errors`, `log_debug` features

#### 9. **valtron** (Lines 14)
- **What**: Value transformation and async execution framework
- **Why**: Provides powerful async execution and task management primitives
- **How**: Executors, iterators, and functional programming constructs
- **Sub-modules**:
  - `executors`: Single and multi-threaded executors
  - `drain`: Draining iterators
  - `funcs`: Functional combinators
  - `iterators`: Custom iterator types
  - `notifiers`: Notification mechanisms
  - `types`: Core valtron types
  - `delayed_iterators`: Lazy evaluation iterators
  - `multi_iterator`: Multi-source iterator composition

#### 10. **wire** (Lines 15)
- **What**: Wire protocol and HTTP utilities
- **Why**: Provides HTTP client/server abstractions
- **How**: Event-driven HTTP, streaming, and simple HTTP implementations
- **Sub-modules**:
  - `event_source`: Server-Sent Events (SSE) support
  - `http_stream`: Streaming HTTP operations
  - `simple_http`: Simple HTTP client/server

## What It Imports

### Workspace Dependencies
- **foundation_wasm** (workspace): WASM-specific utilities
- **foundation_nostd** (workspace): No-std compatible primitives

### External Dependencies
- **derive_more** (v2.0): Derive macro utilities (From, Debug, Error)
- **serde** (v1): Serialization framework
- **serde_json** (v1): JSON serialization
- **serde_yml** (v0.0.12): YAML serialization
- **toml** (v0.9): TOML serialization
- **toml_datetime** (v0.7): TOML datetime support
- **concurrent-queue** (v2.5.0): Lock-free concurrent queue
- **fastrand** (v2.3.0): Fast random number generation
- **rand** (v0.9): Random number generation
- **rand_chacha** (v0.9): ChaCha RNG implementation
- **async-trait** (v0.1.88): Async trait support
- **memchr** (v2.7.4): Fast byte string searching
- **tracing** (v0.1.41): Application-level tracing
- **thiserror** (v2.0): Error derive macros
- **spin** (v0.10): Spinlock implementations
- **wasm_sync** (v0.1.2): WASM synchronization primitives
- **url** (v2.5): URL parsing and manipulation
- **regex** (v1.12.2): Regular expressions
- **ctrlc** (v3.4): Ctrl+C signal handling
- **rust-embed** (v8.7.0): Embed files in binary

### Optional Dependencies
- **openssl** (v0.10): OpenSSL bindings (feature: `ssl-openssl`)
- **rustls** (v0.23): Pure Rust TLS implementation (feature: `ssl-rustls`)
- **rustls-pemfile** (v2.2): PEM file parsing for Rustls
- **zeroize** (v1): Secure memory zeroing
- **native-tls** (v0.2): Platform-native TLS (feature: `ssl-native-tls`)
- **tokio** (v1.44): Async runtime (feature: `tokio_runtime`)

## Public API

### Re-exported Types
All sub-modules re-export their public items at the crate root level, making them accessible via `foundation_core::*`.

### Key Public Items

#### From `compati`
- `Mutex<T>`: Platform-appropriate mutex
- `RwLock<T>`: Platform-appropriate read-write lock

#### From `netcap`
- Connection types for TCP networking
- SSL/TLS configuration and contexts
- HTTP client/server primitives
- Network error types

#### From `retries`
- `RetryStrategy`: Retry configuration
- Exponential backoff implementations
- Fixed delay retry strategies

#### From `synca`
- `Event`: Async event primitives
- Signal handlers
- Sleep utilities
- Drop guards

#### From `trace`
- `info!()`: Info-level logging macro
- `warn!()`: Warning-level logging macro
- `error!()`: Error-level logging macro
- `debug!()`: Debug-level logging macro

#### From `valtron`
- Executor types (single/multi-threaded)
- Iterator extensions
- Async function combinators
- Notifier patterns

#### From `wire`
- `SimpleHttp`: Simple HTTP client
- Event source (SSE) support
- HTTP streaming utilities

## Feature Flags

### Logging Features
- **`log_info`**: Enables info-level logging
- **`log_warnings`**: Enables warning-level logging
- **`log_errors`**: Enables error-level logging
- **`log_debug`**: Enables debug-level logging
- **`debug_trace`**: Enables all logging (implies `standard` + `log_debug`)
- **`standard`**: Enables info, errors, and warnings logging

### SSL/TLS Features
- **`ssl-openssl`**: Use OpenSSL for TLS
- **`ssl-rustls`**: Use Rustls for TLS (pure Rust)
- **`ssl-native-tls`**: Use platform-native TLS
- **`ssl`**: Alias for `ssl-rustls` (default TLS implementation)

### Runtime Features
- **`nothread_runtime`**: Single-threaded runtime for WASM/no-std environments
- **`tokio_runtime`**: Enable Tokio async runtime support

### Default Features
- **`default`**: `["standard", "ssl"]` - Standard logging + Rustls TLS

## Architecture

### Design Patterns Used
- **Conditional Compilation**: Extensive use of `cfg` attributes for platform-specific code
- **Zero-Cost Abstractions**: Macros that compile to nothing when features are disabled
- **Builder Pattern**: Used in configuration types for SSL and network settings
- **Trait-based Abstractions**: Extension traits for adding functionality to existing types
- **Feature Flag Architecture**: Modular feature system for opt-in/opt-out functionality

### Module Structure
```
foundation_core/
├── src/
│   ├── lib.rs                    # Crate root, module declarations
│   ├── compati/                  # Platform compatibility
│   │   └── mod.rs
│   ├── extensions/               # Extension traits
│   │   ├── mod.rs
│   │   ├── result_ext/
│   │   ├── serde_ext/
│   │   ├── strings_ext/
│   │   ├── tokio_ext/
│   │   └── vec_ext/
│   ├── io/                       # I/O utilities
│   │   ├── mod.rs
│   │   ├── ioutils/
│   │   ├── mem/
│   │   └── ubytes/
│   ├── macros/                   # Utility macros
│   │   ├── mod.rs
│   │   ├── collections/
│   │   ├── expects/
│   │   ├── ioerrs/
│   │   └── rezzo/
│   ├── netcap/                   # Network capabilities
│   │   ├── mod.rs
│   │   ├── connection/
│   │   ├── errors/
│   │   ├── ssl/
│   │   └── server/
│   ├── retries/                  # Retry mechanisms
│   │   ├── mod.rs
│   │   ├── core.rs
│   │   ├── exponential.rs
│   │   └── same.rs
│   ├── synca/                    # Sync and async primitives
│   │   ├── mod.rs
│   │   ├── drops.rs
│   │   ├── entrylist.rs
│   │   ├── event.rs
│   │   ├── idleman.rs
│   │   ├── mpp/
│   │   ├── signals.rs
│   │   └── sleepers.rs
│   ├── trace/                    # Conditional tracing
│   │   └── mod.rs
│   ├── valtron/                  # Async execution framework
│   │   ├── mod.rs
│   │   ├── executors/
│   │   ├── drain.rs
│   │   ├── funcs.rs
│   │   ├── iterators.rs
│   │   ├── notifiers.rs
│   │   └── types.rs
│   └── wire/                     # HTTP and wire protocols
│       ├── mod.rs
│       ├── event_source/
│       ├── http_stream/
│       └── simple_http/
└── Cargo.toml
```

## Key Implementation Details

### Performance Considerations
- **Zero-Cost Logging**: Trace macros compile to no-ops when features disabled (Lines 4-50 in trace/mod.rs)
- **Lock-Free Queues**: Uses `concurrent-queue` for high-performance concurrent operations
- **Fast RNG**: `fastrand` for performance-critical random number generation
- **Efficient String Operations**: `memchr` for fast byte searching

### Security Considerations
- **Multiple TLS Backends**: Support for OpenSSL, Rustls, and Native-TLS
- **Secure Memory Zeroing**: `zeroize` for sensitive data cleanup
- **Vendored Native-TLS**: Ensures consistent TLS behavior across platforms

### Concurrency/Async Handling
- **Platform-Specific Locks**: WASM uses spinlocks, native uses std::sync
- **Custom Executors**: Single and multi-threaded executors in valtron
- **Optional Tokio Integration**: Can integrate with Tokio runtime via feature flag
- **WASM-Compatible**: Special handling for single-threaded WASM environments

### Platform Compatibility
- **WASM Support**: Conditional compilation for WebAssembly targets
- **No-std Compatible**: Can work in no-std environments via foundation_nostd
- **Cross-Platform Networking**: Abstracts platform differences in networking

## Tests

### Test Coverage
- **Unit Tests**: Inline tests in most modules
- **Integration Tests**: Located in `tests/` directory
- **Dev Dependencies**:
  - `tracing-test` (v0.2.5): Testing tracing output
  - `reqwest` (v0.12.15): HTTP client testing

### Testing Strategy
- Feature flag testing ensures different configurations work correctly
- Platform-specific code tested via conditional compilation
- SSL backend testing for each TLS implementation
- Network mock testing for wire protocols

## Dependencies and Relationships

### Depends On
- **foundation_wasm** (workspace): WASM utilities and abstractions
- **foundation_nostd** (workspace): No-std compatible primitives

### Used By
- **foundation_ai**: AI-specific functionality
- **foundation_macros**: Procedural macros for the platform
- **runtimes**: Runtime implementations
- All other crates in the ewe-platform ecosystem

### Sibling Modules
- **foundation_wasm**: WASM-specific implementations
- **foundation_nostd**: No-std implementations
- **foundation_macros**: Compile-time code generation

## Configuration

### Feature Flags (Cargo.toml)
All configuration is done via Cargo feature flags (see Feature Flags section above).

### Environment Variables
No direct environment variable usage; configuration is compile-time via features.

## Known Issues and Limitations

### Current Limitations
1. **Platform-Specific Code**: Some features only available on non-WASM platforms (networking, SSL)
2. **Single-Threaded WASM**: WASM support limited to single-threaded execution
3. **Optional Tokio**: Tokio integration requires feature flag, not enabled by default

### Technical Debt
- Some modules allow `unused_imports` and `dead_code` (netcap/mod.rs Lines 1-2)
- Feature flag complexity could be simplified
- Documentation could be more comprehensive in some sub-modules

## Future Improvements

### Planned Enhancements
- **Async WASM**: Better async support for WASM environments as standards evolve
- **Additional TLS Backends**: Potential for more TLS implementation options
- **Performance Optimizations**: Further zero-cost abstraction improvements

### Refactoring Opportunities
- **Feature Flag Consolidation**: Simplify feature flag dependencies
- **Module Organization**: Some large modules could be split further
- **Error Handling**: Consolidate error types across modules

## Related Documentation

### Specifications
- No specific specifications currently documented

### External Resources
- [Rust Documentation](https://doc.rust-lang.org/)
- [Tracing Docs](https://docs.rs/tracing/)
- [Tokio Docs](https://tokio.rs/)
- [Rustls](https://docs.rs/rustls/)

### Related Modules
- `documentation/foundation_wasm/doc.md`
- `documentation/foundation_nostd/doc.md`
- `documentation/foundation_macros/doc.md`

## Version History

### [0.0.3] - Current
- Central foundation crate with comprehensive utilities
- Multi-SSL backend support
- WASM and no-std compatibility
- Conditional tracing system
- Async execution framework (valtron)

---
*Last Updated: 2026-01-14*
*Documentation Version: 1.0*
