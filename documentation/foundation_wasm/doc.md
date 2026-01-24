---
module: foundation_wasm
language: rust
status: active
last_updated: 2026-01-14
maintainer: ewe-platform team
related_specs: []
---

# foundation_wasm - Documentation

## Overview
`foundation_wasm` is a no-std compatible crate that implements the runtime interface for WebAssembly (WASM) and JavaScript interoperability in the ewe-platform. It provides a sophisticated binary protocol for cross-boundary communication between WASM and JS environments, enabling function calls, memory management, and data exchange with type-safe encoding and quantization optimizations.

## Purpose and Responsibility
This crate serves as the foundational layer for WASM/JS interop, providing:
- Binary protocol for encoding/decoding operations across WASM boundaries
- Type-safe parameter and return value marshalling
- Memory management with generational allocation tracking
- Function registration and invocation mechanisms (sync and async)
- Scheduling primitives (frames, intervals, callbacks)
- Value quantization for efficient cross-boundary data transfer
- No-std compatibility for WASM environments

## Module Location
- **Path**: `backends/foundation_wasm/`
- **Entry Point**: `src/lib.rs`
- **Language**: Rust 2021 edition (no_std)
- **Package Manager**: Cargo
- **Version**: 0.0.2

## What It Implements

### Core Modules

#### 1. **base** (Line 5)
- **What**: Core type definitions for the binary protocol
- **Why**: Provides type-safe representation of all values that can cross WASM boundaries
- **How**: Extensive enums and structs with u8-based discrimination for efficient encoding
- **Key Types**:
  - `TypedSlice`: Enum for typed array slices (Int8, Int16, Int32, Int64, Uint*, Float*)
  - `ReturnTypeId`: 32 variants representing all possible return types (Bool, Text8, Int*, Uint*, Float*, Arrays, Objects, References, etc.)
  - `ThreeState`: Expresses 1, 2, or 3 possible states (like Option/Result)
  - `ReturnTypeHints`: Describes function return signatures (None, One, List, Multi)
  - `Returns` & `ReturnValues`: Actual return values from function calls
  - `ParamTypeId` & `Params`: Parameter types and values for function calls
  - `Operations`: Operation types (Begin, MakeFunction, Invoke, InvokeAsync, End, Stop)
  - `ArgumentOperations`: Argument encoding markers (Start, Begin, End, Stop)
  - `InternalPointer` & `ExternalPointer`: Reference handles across boundaries
  - `MemoryId`: Generational index for memory allocations (u32 index + u32 generation)
  - `MemoryLocation`: Pointer + length pair
  - `CallParams`: Call parameter location descriptor
  - `TypeOptimization`: 27 variants for quantized type representations
- **Special Features**:
  - Value quantization module for compressing numeric types
  - Comprehensive From/Into implementations for type conversions
  - Binary encoding with little-endian format
  - Extensive test coverage for quantization

#### 2. **error** (Line 6)
- **What**: Error types for binary operations
- **Why**: Type-safe error handling for encoding/decoding failures
- **How**: Custom error enums with `core::error::Error` implementations
- **Key Types**:
  - `BinaryReadError`: Errors during binary deserialization
  - `BinaryReaderResult<T>`: Result type for read operations
  - `ReturnValueError`: Errors in return value handling
  - `MemoryReaderError`: Memory access errors
  - `MemoryAllocationError`: Allocation failures
  - `TaskResult`: Task execution results
  - `TaskErrorCode`: Error codes for task failures

#### 3. **frames** (Line 7)
- **What**: Animation frame callback management
- **Why**: Implements requestAnimationFrame-like functionality for WASM
- **How**: Registry of callbacks with tick state tracking
- **Key Types**:
  - `FrameCallback`: Callback that returns tick state (Continue/Stop/Restart)
  - `FnFrameCallback`: Function-based frame callback wrapper
  - `FrameCallbackList`: BTreeMap-based registry
  - `TickState`: Enum for callback lifecycle control
- **Platform Handling**: Different trait bounds for WASM (no Send/Sync) vs native

#### 4. **intervals** (Line 8)
- **What**: Interval-based recurring callback system
- **Why**: Implements setInterval-like functionality for WASM
- **How**: Registry with execution tracking and removal support
- **Key Types**:
  - `IntervalCallback`: Recurring callback interface
  - `FnIntervalCallback`: Function wrapper for intervals
  - `IntervalRegistry`: BTreeMap registry with call tracking
- **Platform Handling**: Conditional trait bounds based on target architecture

#### 5. **jsapi** (Line 9)
- **What**: High-level JavaScript API implementation
- **Why**: Provides the main WASM-to-JS communication interface
- **How**: Static registries with mutex-protected shared state
- **Key Functionality**:
  - Global registries (ALLOCATIONS, ANIMATION_FRAME_CALLBACKS, INTERNAL_CALLBACKS, etc.)
  - Memory allocation/deallocation with generational tracking
  - Function registration and invocation
  - Async callback handling
  - Frame and interval management
  - Binary instruction encoding/decoding
- **Key Functions**:
  - `tick_animations()`: Process animation frame callbacks
  - `tick_intervals()`: Process interval callbacks
  - `tick_schedules()`: Process scheduled callbacks
  - `register_callback()`: Register async callback handlers
  - `allocate_memory()`: Allocate tracked memory blocks
  - `deallocate_memory()`: Free memory with generation check
  - `encode_instructions()`: Create binary operation batches
  - `decode_return_values()`: Parse binary return values

#### 6. **mem** (Line 10)
- **What**: Memory management with generational allocation
- **Why**: Safe memory tracking across WASM boundaries
- **How**: Generational arena allocator with Vec-based storage
- **Key Types**:
  - `MemoryAllocation`: Single allocation (data + metadata)
  - `MemoryAllocations`: Arena allocator with free list and generations
  - `MemoryEntry`: Allocation entry with generation counter
- **Key Features**:
  - O(1) allocation and deallocation
  - Generational indices prevent use-after-free
  - Automatic free list management
  - Graceful handling of invalid IDs

#### 7. **ops** (Line 11)
- **What**: Binary operations encoding/decoding
- **Why**: Serialize/deserialize cross-boundary function calls
- **How**: Custom binary protocol with type markers and quantization
- **Key Traits**:
  - `ToBinary`: Serialization to byte vector
  - `FromBinary`: Deserialization from byte slice
- **Key Types**:
  - `Instructions`: Collection of encoded operations
  - Binary readers/writers with cursor tracking
- **Protocol Details**:
  - Little-endian byte order
  - Type-prefixed values
  - Quantization optimization markers
  - Begin/End markers for structured data

#### 8. **registry** (Line 12)
- **What**: Internal callback reference registry
- **Why**: Track async callbacks for host->WASM communication
- **How**: BTreeMap-based registry with unique IDs
- **Key Types**:
  - `InternalCallback`: Async callback with return type hints
  - `InternalReferenceRegistry`: Thread-safe callback storage
  - `FnInternalCallback`: Function wrapper for callbacks
- **Platform Handling**: Conditional Send/Sync traits

#### 9. **schedule** (Line 13)
- **What**: Scheduled task execution registry
- **Why**: Implements setTimeout-like functionality for WASM
- **How**: BTreeMap registry with DoTask trait
- **Key Types**:
  - `DoTask`: Task execution trait (platform-conditional)
  - `FnDoTask`: Function wrapper for tasks
  - `ScheduleRegistry`: Task storage and execution
- **Platform Handling**: Different trait bounds for WASM vs native

#### 10. **wrapped** (Line 14)
- **What**: Wrapper type for registered items
- **Why**: Provides consistent wrapping for registry storage
- **How**: Newtype pattern with Mutex for thread safety
- **Key Type**:
  - `WrappedItem<T>`: Generic wrapper with platform-appropriate locking

## What It Imports

### Workspace Dependencies
- **foundation_nostd** (workspace): No-std compatible primitives (spin locks, raw parts)
- **foundation_macros** (workspace): Procedural macros for asset embedding

### External Dependencies
None beyond the workspace dependencies. This is a minimal no-std crate.

## Public API

### Re-exported Types
All sub-modules re-export their public items at the crate root level via:
```rust
pub use base::*;
pub use error::*;
pub use frames::*;
pub use intervals::*;
pub use jsapi::*;
pub use mem::*;
pub use ops::*;
pub use registry::*;
pub use schedule::*;
pub use wrapped::*;
```

### Key Public Items

#### From `base`
- **Type System**: `TypedSlice`, `ReturnTypeId`, `ParamTypeId`, `ThreeState`, `ReturnTypeHints`
- **Value Types**: `Params<'a>`, `Returns`, `ReturnValues`
- **Pointers**: `InternalPointer`, `ExternalPointer`
- **Memory**: `MemoryId`, `MemoryLocation`, `CallParams`
- **Operations**: `Operations`, `ArgumentOperations`, `TypeOptimization`
- **Quantization**: `value_quantitization` module with qi16, qi32, qi64, qi128, qu16, qu32, qu64, qu128, qf64, qpointer

#### From `jsapi`
- **Memory**: `allocate_memory()`, `deallocate_memory()`, `get_memory_location()`
- **Animation**: `tick_animations()`, `register_frame_callback()`, `unregister_frame_callback()`
- **Intervals**: `tick_intervals()`, `register_interval()`, `unregister_interval()`
- **Scheduling**: `tick_schedules()`, `register_schedule()`, `unregister_schedule()`
- **Callbacks**: `register_callback()`, `callback_complete()`
- **Operations**: `encode_instructions()`, `decode_return_values()`

#### From `mem`
- **Allocator**: `MemoryAllocations` with `allocate()`, `deallocate()`, `get()` methods

#### From `registry`
- **Callbacks**: `InternalReferenceRegistry` for async callback management

## Feature Flags

### Optional Features
- **`f128`**: Enable experimental f128 type support (currently empty, for future use)

### Default Features
None - minimal by design for maximum WASM compatibility.

## Architecture

### Design Patterns Used
- **Generational Indices**: Prevents use-after-free in memory management
- **Binary Protocol**: Custom encoding for cross-boundary efficiency
- **Type Discrimination**: u8-based type IDs for compact representation
- **Quantization**: Automatic size reduction for numeric types
- **Registry Pattern**: Centralized storage for callbacks and allocations
- **Static Globals**: Mutex-protected registries for cross-call state
- **Trait-Based Abstraction**: DoTask, FrameCallback, IntervalCallback traits
- **Platform Conditionals**: Different implementations for WASM vs native

### Module Structure
```
foundation_wasm/
├── src/
│   ├── lib.rs                    # Crate root (no_std, module exports)
│   ├── base.rs                   # Core type system (2283 lines)
│   │   ├── TypedSlice, ReturnTypeId, ParamTypeId enums
│   │   ├── ThreeState, ReturnTypeHints, Returns types
│   │   ├── Params<'a>, ReturnValues, Operations enums
│   │   ├── Pointer types (Internal/External)
│   │   ├── Memory types (MemoryId, MemoryLocation)
│   │   └── value_quantitization module (qi*, qu* functions)
│   ├── error.rs                  # Error types
│   │   ├── BinaryReadError, ReturnValueError
│   │   ├── MemoryReaderError, MemoryAllocationError
│   │   └── TaskResult, TaskErrorCode
│   ├── frames.rs                 # Animation frame callbacks
│   │   ├── FrameCallback trait (platform-conditional)
│   │   ├── FnFrameCallback wrapper
│   │   ├── FrameCallbackList registry
│   │   └── TickState enum
│   ├── intervals.rs              # Interval callbacks
│   │   ├── IntervalCallback trait
│   │   ├── FnIntervalCallback wrapper
│   │   └── IntervalRegistry
│   ├── jsapi.rs                  # Main JS API (137KB, high complexity)
│   │   ├── Static global registries
│   │   ├── Memory management functions
│   │   ├── Callback registration/execution
│   │   ├── Instruction encoding/decoding
│   │   └── Tick functions for animations/intervals/schedules
│   ├── mem.rs                    # Generational memory allocator
│   │   ├── MemoryAllocation struct
│   │   ├── MemoryAllocations arena
│   │   └── MemoryEntry with generation tracking
│   ├── ops.rs                    # Binary protocol operations
│   │   ├── ToBinary trait
│   │   ├── FromBinary trait
│   │   ├── Instructions type
│   │   └── Binary encoding/decoding logic
│   ├── registry.rs               # Internal callback registry
│   │   ├── InternalCallback struct
│   │   ├── FnInternalCallback wrapper
│   │   └── InternalReferenceRegistry
│   ├── schedule.rs               # Scheduled task registry
│   │   ├── DoTask trait (platform-conditional)
│   │   ├── FnDoTask wrapper
│   │   └── ScheduleRegistry
│   └── wrapped.rs                # Generic wrapper type
│       └── WrappedItem<T>
└── Cargo.toml
```

## Key Implementation Details

### Performance Considerations
- **Quantization**: Automatic numeric type compression (e.g., u64 → u8 if value ≤ 255)
- **Generational Indices**: O(1) allocation/deallocation with memory safety
- **Binary Protocol**: Compact wire format with type prefixes
- **Little-Endian**: Matches WASM native endianness
- **Static Allocations**: Zero-cost registry initialization
- **Arena Allocator**: Minimizes allocation overhead

### Security Considerations
- **Generational Checks**: Prevents use-after-free bugs
- **Type Safety**: Strong typing at protocol level
- **Bounds Checking**: Safe memory access validation
- **No Panics in Hot Paths**: Graceful error handling

### Concurrency/Async Handling
- **Platform-Specific Traits**: WASM targets don't require Send/Sync
- **Mutex Protection**: All registries protected by spin::Mutex
- **Async Callbacks**: Internal callbacks support Promise-like patterns
- **Single-Threaded WASM**: No threading concerns in target environment

### Platform Compatibility
- **No-std**: Works in bare WASM environments
- **Conditional Compilation**: `#[cfg(target_arch = "wasm32")]` extensively used
- **Extern Alloc**: Uses `extern crate alloc` for Vec/String
- **No System Dependencies**: Pure Rust implementation

### Binary Protocol Details
The binary protocol is sophisticated and well-documented in code:
- **Memory Layout**: Documented in base.rs with byte-level specifications
- **Operations Format**: Begin/Operation/End/Stop markers
- **Arguments Format**: Start/Begin/[Type+Content]/End/Stop
- **Return Values**: Type ID + optimized content + markers
- **Quantization**: TypeOptimization enum (27 variants) for size reduction

## Tests

### Test Coverage
- **Quantization Tests**: Comprehensive tests in `base.rs` (lines 2054-2282)
  - `can_quantize_ptr`: Tests pointer quantization
  - `can_quantize_i128`: Tests signed 128-bit integer quantization
  - `can_quantize_u128`: Tests unsigned 128-bit integer quantization
  - `can_quantize_u64`: Tests unsigned 64-bit integer quantization
  - `can_quantize_i64`: Tests signed 64-bit integer quantization
- **Schedule Registry Tests**: Tests in `schedule.rs` (lines 163-193)
  - `test_add`: Tests callback addition and execution
- **Module Tests**: Inline tests in frames, intervals, registry modules

### Testing Strategy
- Unit tests for quantization ensure correct byte output and type optimization
- Tests verify all quantization ranges (u8, u16, u32, u64, u128, i8, i16, i32, i64, i128)
- Registry tests verify callback lifecycle (add, call, remove)
- Test cases use Arc<Mutex<T>> for shared state verification

## Dependencies and Relationships

### Depends On
- **foundation_nostd** (workspace): Provides spin locks, RawParts utility, embeddable traits
- **foundation_macros** (workspace): Asset embedding macros (not directly used in this crate)

### Used By
- **foundation_core**: Uses foundation_wasm for WASM-specific implementations
- **All WASM-compiled crates**: Any crate targeting WebAssembly uses this for JS interop

### Sibling Modules
- **foundation_nostd**: Provides synchronization primitives
- **foundation_core**: Higher-level abstractions built on this foundation
- **foundation_macros**: Compile-time code generation

## Configuration

### Feature Flags (Cargo.toml)
- **`f128`**: Reserved for future f128 type support (currently no implementation)

### Environment Variables
None - all configuration is compile-time.

### Build Configuration
- **no_std**: Crate attribute `#![no_std]`
- **Edition**: Rust 2021
- **Target**: Primarily wasm32/wasm64, but also supports native for testing

## Known Issues and Limitations

### Current Limitations
1. **No-std Only**: Cannot use std library features (by design)
2. **WASM-Specific**: API designed specifically for WASM/JS boundary
3. **Manual Memory Management**: Users must track allocations and deallocations
4. **f128 Support**: Feature flag exists but not implemented (waiting for stable f128)
5. **Single-Threaded**: WASM environments are single-threaded, no multi-threading support
6. **Static Registries**: Global state limits architectural flexibility

### Technical Debt
- **Complex Binary Protocol**: jsapi.rs is 137KB with high complexity
- **Extensive Unsafe**: Memory operations require unsafe code
- **Limited Documentation**: Some internal functions lack doc comments
- **Test Coverage**: Could benefit from more integration tests

## Future Improvements

### Planned Enhancements
- **f128 Support**: Implement when Rust stabilizes f128 types
- **Streaming Operations**: Support for large data transfers
- **Protocol Versioning**: Backward compatibility mechanism
- **Better Error Context**: More detailed error messages

### Refactoring Opportunities
- **Split jsapi.rs**: Break down large file into smaller modules
- **Macro-based Protocol**: Generate encode/decode code from schema
- **Zero-Copy Operations**: Reduce allocation in hot paths
- **Documentation**: Add more examples and usage guides

## Related Documentation

### Specifications
- No specific specifications currently documented
- Binary protocol documented inline in base.rs

### External Resources
- [WebAssembly Specification](https://webassembly.github.io/spec/)
- [Rust no_std Book](https://docs.rust-embedded.org/book/intro/no-std.html)
- [WASM Bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)

### Related Modules
- `documentation/foundation_nostd/doc.md`
- `documentation/foundation_core/doc.md`
- `documentation/foundation_macros/doc.md`

## Version History

### [0.0.2] - Current
- No-std WASM/JS interop layer
- Binary protocol for cross-boundary communication
- Generational memory management
- Animation frame and interval support
- Async callback system
- Value quantization for efficient encoding

---
*Last Updated: 2026-01-14*
*Documentation Version: 1.0*
