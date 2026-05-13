---
feature: body-reader-streaming
description: Replace eager body reading with Data-exposing streaming readers, add DataBytesIterator for compatibility
status: design
priority: high
depends_on: []
estimated_effort: large
created: 2026-05-13
last_updated: 2026-05-13
author: Claude Code
---

# Feature: Body Reader Streaming with Data Exposure

## Overview

Replace eager body reading with streaming readers that expose `Data` enum (including `Data::Retry`) instead of hiding it behind loops. This allows callers to handle retry signals themselves (e.g., sleep, yield, do other work).

**Breaking Change:** `SendSafeBody::Stream` type changes from `Iterator<Item=Result<Vec<u8>, BoxedError>>` to `Iterator<Item=Result<Data, BoxedError>>`.

**Compatibility:** New `DataBytesIterator` maintains old semantics for callers who want automatic retry absorption.

## Motivation

### Current Problem

Readers like `BatchStreamReader` loop internally, hiding `Data::Retry`:

```rust
// Current BatchStreamReader::next()
fn next(&mut self) -> Option<Self::Item> {
    loop {
        match self.inner.next() {
            Some(Ok(Data::Retry)) => {}  // Hidden! Loops automatically
            Some(Ok(Data::Bytes(bytes))) => return Some(Ok(bytes)),
            ...
        }
    }
}
```

Problems:
1. **No control over retry behavior** - Callers can't sleep, yield, or do other work between retries
2. **Blocking behavior** - In async contexts, spinning on `Data::Retry` wastes CPU
3. **Inconsistent with valtron** - Valtron tasks should yield on retry, not spin

### Proposed Solution

Expose `Data` directly, let callers decide:

```rust
// New design - caller sees Data::Retry
fn next(&mut self) -> Option<Self::Item> {
    match self.inner.next() {
        Some(Ok(data)) => Some(Ok(data)),  // Returns Data::Bytes OR Data::Retry
        ...
    }
}

// Caller chooses strategy:
match reader.next() {
    Some(Ok(Data::Retry)) => {
        // Can sleep, yield, or do other work
        return TaskStatus::Delayed(Duration::from_millis(10));
    }
    Some(Ok(Data::Bytes(bytes))) => process(bytes),
    ...
}
```

## Design

### Core Changes

#### 1. New Data-Exposing Readers

All readers changed to expose `Data` directly (no internal loops):

| Reader | Before | After |
|--------|--------|-------|
| `BatchReader` | Already yields `Result<Data, io::Error>` | **Unchanged** |
| `BatchStreamReader` | Loops, yields `Result<Vec<u8>, BoxedError>` | Yields `Result<Data, BoxedError>` (no loop) |
| `FullBodyReader` | `read_full()` returns `Result<Vec<u8>, io::Error>` | Iterator yielding `Result<Data, io::Error>` |
| `EofReader` | `read_to_end()` returns `Result<Vec<u8>, io::Error>` | Iterator yielding `Result<Data, io::Error>` |
| `LimitedBatchStreamReader` | N/A (new) | Iterator yielding `Result<Data, BoxedError>` |
| `EOFStreamReader` | N/A (new) | Iterator yielding `Result<Data, BoxedError>` |
| `LimitedEOFStreamReader` | N/A (new) | Iterator yielding `Result<Data, BoxedError>` |

#### 2. DataBytesIterator

New wrapper that converts `Data`-exposing iterators to simple `Vec<u8>` iterators:

```rust
/// WHY: Backward compatibility - converts Data-exposing iterator to simple bytes iterator.
/// WHAT: Iterator<Item=Result<Data, E>> → Iterator<Item=Result<Vec<u8>, E>>
/// HOW: Absorbs Data::Retry, yields Data::Bytes as Vec<u8>.
pub struct DataBytesIterator<I, E> {
    inner: I,
    _marker: PhantomData<E>,
}

impl<I, E> DataBytesIterator<I, E> {
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<I, E> Iterator for DataBytesIterator<I, E>
where
    I: Iterator<Item = Result<Data, E>>,
{
    type Item = Result<Vec<u8>, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Ok(Data::Retry)) => continue,  // Absorb and retry
                Some(Ok(Data::Bytes(bytes))) => return Some(Ok(bytes)),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }
    }
}

// Convenience extension trait
pub trait IntoBytes<E> {
    fn into_bytes(self) -> DataBytesIterator<Self, E>
    where
        Self: Iterator<Item = Result<Data, E>> + Sized,
    {
        DataBytesIterator::new(self)
    }
}

impl<I, E> IntoBytes<E> for I where I: Iterator<Item = Result<Data, E>> {}
```

#### 3. SendSafeBody::Stream Type Change

**Before:**
```rust
pub enum SendSafeBody {
    Stream(Option<BoxedSendableIterator<Vec<u8>, BoxedError>>),
    ...
}
```

**After:**
```rust
pub enum SendSafeBody {
    // Now exposes Data, not just Vec<u8>
    Stream(Option<BoxedSendableIterator<Data, BoxedError>>),
    ...
}

// New type alias for backward compatibility
pub type DataBytesStream = BoxedSendableIterator<Vec<u8>, BoxedError>;
```

## Affected Areas in Codebase

### Foundation Core - readers/mod.rs

**File:** `backends/foundation_core/src/io/readers/mod.rs`

**Changes:**
1. Modify `BatchStreamReader` to expose `Data` (remove loop)
2. Convert `FullBodyReader` from function-based to iterator
3. Convert `EofReader` from function-based to iterator
4. Add `LimitedBatchStreamReader` (new)
5. Add `EOFStreamReader` (new)
6. Add `LimitedEOFStreamReader` (new)
7. Add `DataBytesIterator` (new)

### Foundation Core - impls.rs

**File:** `backends/foundation_core/src/wire/simple_http/impls.rs`

**Line 307 - SendSafeBody::Stream type:**
```rust
// Line 307: Change this
Stream(Option<BoxedSendableIterator<Vec<u8>, BoxedError>>),
// To this
Stream(Option<BoxedSendableIterator<Data, BoxedError>>),
```

**Lines affected (need Data handling):**
- Line 189, 202: PartialEq implementations
- Line 397: SimpleBody::write_to
- Line 1647, 1834: Stream body assignments
- Line 2148, 2501: Stream processing in iterators
- Line 3546-3547: Stream body wrapping
- Line 4032-4033: Stream body wrapping
- Line 5162-5332: SimpleHttpBody::extract implementations

### Foundation Core - body_reader.rs

**File:** `backends/foundation_core/src/wire/simple_http/client/body_reader.rs`

**Functions needing updates (23 usages):**
1. `collect_string_strict` (lines 175-234) - handle Data::Retry
2. `collect_bytes_strict` (lines 336-417) - handle Data::Retry
3. `collect_json` (lines 496-538) - handle Data::Retry
4. `write_body_to` (lines 761-800) - handle Data::Retry
5. `write_body_to_with_callback` (lines 858-909) - handle Data::Retry
6. `process_stream_with_callback` (lines 1168-1235) - handle Data::Retry
7. Test functions (lines 1595-1703) - create test data

**Pattern change required:**
```rust
// Before
for chunk_result in iter {
    match chunk_result {
        Ok(data) => bytes.extend_from_slice(&data),  // data is Vec<u8>
        Err(e) => return Err(...),
    }
}

// After - Option 1: Handle Data explicitly
for chunk_result in iter {
    match chunk_result {
        Ok(Data::Bytes(bytes)) => process(bytes),
        Ok(Data::Retry) => continue,  // or sleep, yield, etc.
        Err(e) => return Err(...),
    }
}

// After - Option 2: Use DataBytesIterator (backward compatible)
let wrapped = DataBytesIterator::new(iter);
for chunk_result in wrapped {
    match chunk_result {
        Ok(bytes) => process(bytes),  // Already Vec<u8>
        Err(e) => return Err(...),
    }
}
```

## Complete Reader Specifications

### 1. BatchStreamReader (Modified)

```rust
/// WHY: Adapter from BatchReader that can be boxed for SendSafeBody.
/// WHAT: Iterator yielding Result<Data, BoxedError> - exposes Retry to caller.
/// HOW: No internal loop - caller decides how to handle Data::Retry.
pub struct BatchStreamReader<R: Read> {
    inner: BatchReader<R>,
}

impl<R: Read + Send> Iterator for BatchStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + Send + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Ok(data)) => Some(Ok(data)),  // Exposes Data::Bytes AND Data::Retry
            Some(Err(e)) => Some(Err(Box::new(e))),
            None => None,
        }
    }
}
```

### 2. FullBodyReader (Converted to Iterator)

```rust
/// WHY: Stream known-size bodies, exposing Data::Retry for caller control.
/// WHAT: Iterator yielding Result<Data, io::Error> until size bytes read.
/// HOW: Wraps BatchReader, counts bytes, returns Data::Bytes or Data::Retry.
pub struct FullBodyReader<R: Read> {
    inner: BatchReader<R>,
    bytes_yielded: usize,
    target_size: usize,
}

impl<R: Read> Iterator for FullBodyReader<R> {
    type Item = Result<Data, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes_yielded >= self.target_size {
            return None;
        }

        match self.inner.next() {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_yielded += bytes.len();
                Some(Ok(Data::Bytes(bytes)))
            }
            other => other,  // Pass through Data::Retry and errors
        }
    }
}
```

### 3. EofReader (Converted to Iterator)

```rust
/// WHY: Stream until EOF, exposing Data::Retry for caller control.
/// WHAT: Iterator yielding Result<Data, io::Error> until EOF.
/// HOW: Wraps BatchReader with eof_on_zero_read=true, passes through all Data.
pub struct EofReader<R: Read> {
    inner: BatchReader<R>,
}

impl<R: Read> Iterator for EofReader<R> {
    type Item = Result<Data, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()  // Simply pass through - BatchReader handles EOF
    }
}
```

### 4. LimitedBatchStreamReader (New)

```rust
/// WHY: Stream limited-size bodies with byte cap, exposing Data::Retry.
/// WHAT: Iterator yielding Result<Data, BoxedError>, stops at cap.
/// HOW: Wraps BatchReader, counts bytes, returns None when cap reached.
pub struct LimitedBatchStreamReader<R: Read> {
    inner: BatchReader<R>,
    bytes_yielded: usize,
    byte_cap: usize,
}

impl<R: Read + Send> Iterator for LimitedBatchStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + Send + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes_yielded >= self.byte_cap {
            return None;
        }

        match self.inner.next() {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_yielded += bytes.len();
                Some(Ok(Data::Bytes(bytes)))
            }
            other => other.map(|r| r.map_err(|e| Box::new(e) as BoxedError)),
        }
    }
}
```

### 5. EOFStreamReader (New)

```rust
/// WHY: Stream until EOF via BoxedSendableIterator, exposing Data::Retry.
/// WHAT: Iterator yielding Result<Data, BoxedError> until EOF.
pub struct EOFStreamReader<R: Read> {
    inner: BatchReader<R>,
}

impl<R: Read + Send> Iterator for EOFStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + Send + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|r| r.map_err(|e| Box::new(e) as BoxedError))
    }
}
```

### 6. LimitedEOFStreamReader (New)

```rust
/// WHY: Stream until EOF with max size enforcement, exposing Data::Retry.
/// WHAT: Iterator yielding Result<Data, BoxedError>, errors if cap exceeded.
pub struct LimitedEOFStreamReader<R: Read> {
    inner: BatchReader<R>,
    bytes_yielded: usize,
    byte_cap: usize,
}

impl<R: Read + Send> Iterator for LimitedEOFStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + Send + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_yielded += bytes.len();
                if self.bytes_yielded > self.byte_cap {
                    return Some(Err(Box::new(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("body size {} exceeds max {}", self.bytes_yielded, self.byte_cap),
                    ))));
                }
                Some(Ok(Data::Bytes(bytes)))
            }
            Some(Ok(Data::Retry)) => Some(Ok(Data::Retry)),
            Some(Err(e)) => Some(Err(Box::new(e) as BoxedError)),
            None => None,
        }
    }
}
```

### 7. DataBytesIterator (New)

```rust
use std::marker::PhantomData;

/// WHY: Backward compatibility - restores old retry-absorbing behavior.
/// WHAT: Wraps Iterator<Item=Result<Data, E>> and loops on Data::Retry.
/// HOW: Transforms Data::Bytes into Vec<u8>, filters Data::Retry.
pub struct DataBytesIterator<I, E> {
    inner: I,
    _marker: PhantomData<E>,
}

impl<I, E> DataBytesIterator<I, E> {
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<I, E> Iterator for DataBytesIterator<I, E>
where
    I: Iterator<Item = Result<Data, E>>,
{
    type Item = Result<Vec<u8>, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Ok(Data::Retry)) => continue,  // Absorb and retry
                Some(Ok(Data::Bytes(bytes))) => return Some(Ok(bytes)),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }
    }
}

// Convenience extension trait
pub trait IntoDataBytes<E> {
    fn into_bytes(self) -> DataBytesIterator<Self, E>
    where
        Self: Iterator<Item = Result<Data, E>> + Sized,
    {
        DataBytesIterator::new(self)
    }
}

impl<I, E> IntoDataBytes<E> for I where I: Iterator<Item = Result<Data, E>> {}
```

## SimpleHttpBody Refactoring

### Before
```rust
impl BodyExtractor for SimpleHttpBody {
    fn extract<T: Read + Send + 'static>(...) -> Result<SendSafeBody, SendableBoxedError> {
        match body {
            Body::FullBody(...) => {
                // EAGER: EofReader::read_to_end() returns Vec<u8>
                EofReader::read_to_end(...).map(SendSafeBody::Bytes)
            }
            Body::LimitedBody(content_length, ...) => {
                if content_length <= self.1 {  // full_body_threshold
                    // EAGER: FullBodyReader::read_full() returns Vec<u8>
                    FullBodyReader::new(...).read_full(...).map(SendSafeBody::Bytes)
                } else {
                    // STREAM: BatchStreamReader
                    let stream_reader = Box::new(BatchStreamReader::new(batch));
                    Ok(SendSafeBody::Stream(Some(stream_reader)))  // Vec<u8>
                }
            }
        }
    }
}
```

### After
```rust
impl BodyExtractor for SimpleHttpBody {
    fn extract<T: Read + Send + 'static>(...) -> Result<SendSafeBody, SendableBoxedError> {
        match body {
            Body::FullBody(headers, optional_max) => {
                let batch = BatchReader::new(stream)
                    .batch_size(self.2)
                    .eof_on_zero_read(true)
                    .max_consecutive_retries(self.3);

                let stream: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> = 
                    if let Some(max) = optional_max {
                        Box::new(LimitedEOFStreamReader::new(batch, max as usize))
                    } else {
                        Box::new(EOFStreamReader::new(batch))
                    };

                Ok(SendSafeBody::Stream(Some(stream)))
            }
            Body::LimitedBody(content_length, ...) => {
                // Always stream - no threshold check
                let batch = BatchReader::new(stream)
                    .batch_size(self.2)
                    .max_consecutive_retries(self.3);

                let stream_reader = Box::new(LimitedBatchStreamReader::new(
                    batch,
                    content_length as usize
                ));

                Ok(SendSafeBody::Stream(Some(stream_reader)))  // Data, not Vec<u8>
            }
            Body::ChunkedBody => { /* unchanged */ }
            Body::SseBody => { /* unchanged */ }
            Body::LineFeedBody => { /* unchanged */ }
        }
    }
}
```

## Migration Guide

### For Callers Using collect_string/collect_bytes

These functions need internal updates to handle `Data::Retry`. The API remains the same:

```rust
// API unchanged - but internally handles Data::Retry
let body = collect_bytes(stream)?;  // Still returns Vec<u8>
```

### For Callers Iterating Over Streams Directly

**Option 1: Use DataBytesIterator (easiest)**
```rust
// Before
if let SendSafeBody::Stream(Some(iter)) = body {
    for chunk_result in iter {  // iter: Iterator<Item=Result<Vec<u8>, _>>
        let bytes = chunk_result?;
        process(bytes);
    }
}

// After
use crate::io::readers::DataBytesIterator;

if let SendSafeBody::Stream(Some(iter)) = body {
    let wrapped = DataBytesIterator::new(iter);
    for chunk_result in wrapped {  // wrapped: Iterator<Item=Result<Vec<u8>, _>>
        let bytes = chunk_result?;
        process(bytes);
    }
}

// Or use the extension trait
use crate::io::readers::IntoDataBytes;

if let SendSafeBody::Stream(Some(iter)) = body {
    for chunk_result in iter.into_bytes() {
        let bytes = chunk_result?;
        process(bytes);
    }
}
```

**Option 2: Handle Data Explicitly (valtron-friendly)**
```rust
if let SendSafeBody::Stream(Some(iter)) = body {
    for data_result in iter {  // iter: Iterator<Item=Result<Data, _>>
        match data_result? {
            Data::Bytes(bytes) => process(bytes),
            Data::Retry => {
                // In valtron task: yield and come back
                return Some(TaskStatus::Delayed(Duration::from_millis(10)));
            }
        }
    }
}
```

### For Custom Stream Processing

```rust
// Before: Expected Result<Vec<u8>, _>
fn process_stream(iter: impl Iterator<Item = Result<Vec<u8>, BoxedError>>) { ... }

// After: Change signature to accept Data
fn process_stream(iter: impl Iterator<Item = Result<Data, BoxedError>>) {
    for result in iter {
        match result {
            Ok(Data::Bytes(bytes)) => handle_bytes(bytes),
            Ok(Data::Retry) => handle_retry(),
            Err(e) => handle_error(e),
        }
    }
}

// Or wrap at call site to maintain old signature
fn process_stream(iter: impl Iterator<Item = Result<Vec<u8>, BoxedError>>) { ... }

// Call with wrapper
process_stream(DataBytesIterator::new(data_iter));
```

## Test Updates Required

### Summary of Test Impact

| Test File | Tests Affected | Action Required |
|-----------|----------------|-----------------|
| `readers/mod.rs` (inline tests) | 15 tests | Update existing, add 15+ new tests |
| `body_reader.rs` (inline tests) | ~20 tests | Update iteration patterns for Data handling |
| `compression_tests.rs` | 4 tests | May need updates for stream handling |
| `simple_http/request_tests.rs` | 5+ tests | Update SendSafeBody assertions |
| `event_source/*_tests.rs` | 4+ tests | Update SendSafeBody pattern matching |
| `simple_http/middleware_tests.rs` | 5+ tests | Update SendSafeBody assertions |

**Total:** ~50+ tests need updates

---

### 1. readers/mod.rs Tests (Inline Module)

#### Existing Tests - Status

| Test Name | Current Status | Required Change |
|-----------|----------------|-----------------|
| `batch_reader_normal_reads` | **UNCHANGED** | Already uses `Data` enum |
| `batch_reader_empty_source_eof` | **UNCHANGED** | No `Data` exposure |
| `batch_reader_would_block_handling` | **UNCHANGED** | Already matches on `Data::Retry` |
| `batch_reader_retry_limit_exceeded` | **UNCHANGED** | Tests BatchReader, not wrappers |
| `batch_reader_eof_on_zero_read_false` | **UNCHANGED** | Already matches on `Data` |
| `full_body_reader_complete_read` | **MODIFY** | Convert to iterator pattern |
| `full_body_reader_partial_reads` | **MODIFY** | Convert to iterator pattern |
| `full_body_reader_retry_handling` | **MODIFY** | Convert to iterator pattern |
| `full_body_reader_unexpected_eof` | **MODIFY** | Convert to iterator pattern |
| `full_body_reader_retry_limit_exceeded` | **MODIFY** | Convert to iterator pattern |
| `batch_stream_reader_absorbs_retries` | **MODIFY** | Update to test Data exposure |
| `batch_stream_reader_propagates_errors` | **MODIFY** | Update for Data enum |
| `batch_stream_reader_empty_source` | **MODIFY** | Update for Data enum |
| `eof_reader_complete_read` | **MODIFY** | Convert to iterator pattern |
| `eof_reader_with_max_size` | **MODIFY** | Convert to iterator pattern |
| `eof_reader_partial_reads` | **MODIFY** | Convert to iterator pattern |
| `eof_reader_retry_handling` | **MODIFY** | Convert to iterator pattern |
| `eof_reader_retry_limit_exceeded` | **MODIFY** | Convert to iterator pattern |
| `eof_reader_empty_source` | **MODIFY** | Convert to iterator pattern |

#### Modified Tests

**full_body_reader tests** - Change from function-based to iterator:

```rust
// BEFORE
#[test]
fn full_body_reader_complete_read() {
    let data = b"hello world";
    let mut cursor = Cursor::new(data.to_vec());
    let result = FullBodyReader::default().read_full(&mut cursor, data.len(), 10);
    assert_eq!(result.unwrap(), data);
}

// AFTER
#[test]
fn full_body_reader_complete_read() {
    let data = b"hello world";
    let reader = FullBodyReader::new(
        BatchReader::new(Cursor::new(data.to_vec())),
        data.len()
    );
    
    let collected: Vec<u8> = reader
        .filter_map(|r| r.ok())
        .filter_map(|d| match d {
            Data::Bytes(b) => Some(b),
            Data::Retry => None,
        })
        .flatten()
        .collect();
    
    assert_eq!(collected, data);
}
```

**batch_stream_reader tests** - Update to test Data exposure:

```rust
// BEFORE - tests that retries are absorbed
#[test]
fn batch_stream_reader_absorbs_retries() {
    let batch = BatchReader::new(AlternatingReader { ... });
    let stream = BatchStreamReader::new(batch);
    let results: Vec<_> = stream.collect();
    // Should only get bytes, no retries
    for r in &results { assert!(r.is_ok()); }
}

// AFTER - tests that retries ARE exposed
#[test]
fn batch_stream_reader_exposes_retries() {
    let batch = BatchReader::new(AlternatingReader { ... });
    let mut stream = BatchStreamReader::new(batch);
    
    // Should see Data::Retry exposed
    let mut got_retry = false;
    let mut got_bytes = false;
    
    while let Some(result) = stream.next() {
        match result.unwrap() {
            Data::Retry => got_retry = true,
            Data::Bytes(_) => got_bytes = true,
        }
    }
    
    assert!(got_retry, "Should expose Data::Retry");
    assert!(got_bytes, "Should expose Data::Bytes");
}
```

#### New Tests Required

```rust
// === DataBytesIterator Tests ===

#[test]
fn data_bytes_iterator_filters_retries() {
    let batch = BatchReader::new(AlternatingRetryReader { ... });
    let data_stream = BatchStreamReader::new(batch);
    let mut wrapped = DataBytesIterator::new(data_stream);
    
    // Should NOT see Data::Retry - only Bytes or None
    while let Some(result) = wrapped.next() {
        assert!(result.is_ok());
        // Result is Vec<u8>, not Data
    }
}

#[test]
fn data_bytes_iterator_propagates_errors() {
    let batch = BatchReader::new(ErrorReader);
    let data_stream = BatchStreamReader::new(batch);
    let mut wrapped = DataBytesIterator::new(data_stream);
    
    assert!(wrapped.next().unwrap().is_err());
}

#[test]
fn data_bytes_iterator_empty_source() {
    let batch = BatchReader::new(Cursor::new(Vec::new()));
    let data_stream = BatchStreamReader::new(batch);
    let mut wrapped = DataBytesIterator::new(data_stream);
    
    assert!(wrapped.next().is_none());
}

// === LimitedBatchStreamReader Tests ===

#[test]
fn limited_batch_stream_reader_stops_at_cap() {
    let data = b"hello world";
    let batch = BatchReader::new(Cursor::new(data.to_vec()));
    let mut limited = LimitedBatchStreamReader::new(batch, 5);
    
    let first = limited.next().unwrap().unwrap();
    assert!(matches!(first, Data::Bytes(b) if b == b"hello"));
    
    // Should stop at cap
    assert!(limited.next().is_none());
}

#[test]
fn limited_batch_stream_reader_exposes_retries() {
    let batch = BatchReader::new(RetryThenDataReader);
    let mut limited = LimitedBatchStreamReader::new(batch, 100);
    
    assert!(matches!(limited.next(), Some(Ok(Data::Retry))));
    assert!(matches!(limited.next(), Some(Ok(Data::Bytes(_)))));
}

#[test]
fn limited_batch_stream_reader_enforces_exact_cap() {
    // Test that last batch may exceed cap but we stop after
}

// === EOFStreamReader Tests ===

#[test]
fn eof_stream_reader_reads_until_eof() {
    let data = b"hello world";
    let batch = BatchReader::new(Cursor::new(data.to_vec()))
        .eof_on_zero_read(true);
    let mut eof_reader = EOFStreamReader::new(batch);
    
    let collected: Vec<u8> = eof_reader
        .filter_map(|r| r.ok())
        .filter_map(|d| match d {
            Data::Bytes(b) => Some(b),
            Data::Retry => None,
        })
        .flatten()
        .collect();
    
    assert_eq!(collected, data);
}

#[test]
fn eof_stream_reader_exposes_retries() {
    let batch = BatchReader::new(RetryReader);
    let mut eof_reader = EOFStreamReader::new(batch);
    
    // Should expose Data::Retry
    assert!(matches!(eof_reader.next(), Some(Ok(Data::Retry))));
}

// === LimitedEOFStreamReader Tests ===

#[test]
fn limited_eof_stream_reader_enforces_max() {
    let data = b"hello world this is long";
    let batch = BatchReader::new(Cursor::new(data.to_vec()));
    let mut limited = LimitedEOFStreamReader::new(batch, 10);
    
    let mut collected = Vec::new();
    while let Some(result) = limited.next() {
        match result {
            Ok(Data::Bytes(bytes)) => collected.extend(bytes),
            Ok(Data::Retry) => continue,
            Err(e) => {
                // Expected error at cap
                assert!(collected.len() > 10 || e.to_string().contains("exceeds"));
                return;
            }
        }
    }
    panic!("Should have errored at cap");
}

#[test]
fn limited_eof_stream_reader_under_limit() {
    let data = b"short";
    let batch = BatchReader::new(Cursor::new(data.to_vec()));
    let mut limited = LimitedEOFStreamReader::new(batch, 100);
    
    let collected: Vec<u8> = limited
        .filter_map(|r| r.ok())
        .filter_map(|d| match d {
            Data::Bytes(b) => Some(b),
            Data::Retry => None,
        })
        .flatten()
        .collect();
    
    assert_eq!(collected, data);
}

// === IntoDataBytes Trait Tests ===

#[test]
fn into_data_bytes_extension() {
    let batch = BatchReader::new(Cursor::new(b"test".to_vec()));
    let stream = BatchStreamReader::new(batch);
    
    // Use extension trait
    let mut wrapped = stream.into_bytes();
    
    while let Some(result) = wrapped.next() {
        let bytes: Vec<u8> = result.unwrap();
        // Process bytes
    }
}
```

---

### 2. body_reader.rs Tests

#### Functions to Update (~20 locations)

**Pattern changes in these test helper functions:**

1. `collect_string_strict` (lines ~175-234) - Update Data handling
2. `collect_bytes_strict` (lines ~336-417) - Update Data handling  
3. `collect_json` (lines ~496-538) - Update Data handling
4. `write_body_to` (lines ~761-800) - Update Data handling
5. `write_body_to_with_callback` (lines ~858-909) - Update Data handling
6. `process_stream_with_callback` (lines ~1168-1235) - Update Data handling
7. Test body creators (lines ~1595-1703) - Create Data-based streams

**Example update:**

```rust
// BEFORE: Expects Vec<u8>
SendSafeBody::Stream(mut opt_iter) => {
    let mut bytes = Vec::new();
    if let Some(iter) = opt_iter.take() {
        for chunk_result in iter {
            match chunk_result {
                Ok(data) => bytes.extend_from_slice(&data),  // data is Vec<u8>
                Err(e) => return Err(...),
            }
        }
    }
}

// AFTER: Handle Data enum
SendSafeBody::Stream(mut opt_iter) => {
    let mut bytes = Vec::new();
    if let Some(iter) = opt_iter.take() {
        for chunk_result in iter {
            match chunk_result {
                Ok(Data::Bytes(data)) => bytes.extend_from_slice(&data),
                Ok(Data::Retry) => continue,  // Or sleep, yield, etc.
                Err(e) => return Err(...),
            }
        }
    }
}

// ALTERNATIVE: Use DataBytesIterator (minimal changes)
use crate::io::readers::IntoDataBytes;

SendSafeBody::Stream(mut opt_iter) => {
    let mut bytes = Vec::new();
    if let Some(iter) = opt_iter.take() {
        for chunk_result in iter.into_bytes() {  // Extension trait
            match chunk_result {
                Ok(data) => bytes.extend_from_slice(&data),  // Still Vec<u8>
                Err(e) => return Err(...),
            }
        }
    }
}
```

---

### 3. Event Source Tests

**Files:**
- `tests/event_source/reconnecting_task_tests.rs` (line 122)
- `tests/event_source/reconnecting_integration_tests.rs` (lines 10, 70-72, 102, 191-193, 231)

**Changes:**

Tests match on `SendSafeBody` variants. Stream variant will now contain `Data`, not `Vec<u8>`:

```rust
// BEFORE
SendSafeBody::Bytes(v) => String::from_utf8_lossy(v).to_string(),
SendSafeBody::Text(s) => s.clone(),
SendSafeBody::None => String::new(),

// AFTER - Add Stream handling
SendSafeBody::Bytes(v) => String::from_utf8_lossy(v).to_string(),
SendSafeBody::Text(s) => s.clone(),
SendSafeBody::None => String::new(),
SendSafeBody::Stream(mut opt_iter) => {
    // Collect stream to string
    let mut bytes = Vec::new();
    if let Some(iter) = opt_iter.take() {
        for result in iter.into_bytes() {
            bytes.extend(result.unwrap());
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}
```

---

### 4. Simple HTTP Tests

**Files:**
- `tests/simple_http/http_redirect_edge_cases_tests.rs` (line 8, 28, 34)
- `tests/simple_http/request_tests.rs` (lines 11, 68, 83, 107-108, 129-130, 147)
- `tests/simple_http/middleware_tests.rs` (lines 38, 59, 70, 92, 99, 119, 127, 141)
- `tests/simple_http/eof_handling_tests.rs` (line 12)
- `tests/simple_http/compression_tests.rs` (lines 151, 179, 207, 238)

**Changes:**

Tests that match on `SendSafeBody::Bytes` or `SendSafeBody::Stream` need updating:

```rust
// BEFORE
assert!(matches!(prepared.body, SendSafeBody::Bytes(_)));

// AFTER
assert!(matches!(prepared.body, SendSafeBody::Stream(_)));

// Tests that inspect body content need DataBytesIterator
if let SendSafeBody::Stream(Some(iter)) = &prepared.body {
    let bytes: Vec<u8> = iter.into_bytes()
        .filter_map(|r| r.ok())
        .flatten()
        .collect();
    assert_eq!(bytes, expected);
}
```

---

### 5. Compression Tests

**File:** `tests/simple_http/compression_tests.rs`

Tests using `.read_to_end()` on decompression - verify these still work with streams.

---

### 6. New Integration Tests

**File to create:** `tests/readers/streaming_readers_tests.rs`

```rust
//! Integration tests for streaming body readers with Data exposure.

use foundation_core::io::readers::{
    BatchReader, BatchStreamReader, DataBytesIterator, 
    LimitedBatchStreamReader, EOFStreamReader, LimitedEOFStreamReader,
    Data, IntoDataBytes
};
use std::io::{Cursor, Read, ErrorKind};

#[test]
fn full_http_body_reading_flow() {
    // Test complete flow: create body reader -> extract -> iterate
}

#[test]
fn valtron_style_retry_handling() {
    // Simulate valtron task that yields on Data::Retry
}

#[test]
fn backward_compatible_data_bytes() {
    // Test that DataBytesIterator provides old behavior
}

#[test]
fn mixed_body_types_all_streaming() {
    // Test LimitedBody, FullBody, ChunkedBody all return streams
}

#[test]
fn size_limit_enforcement_streaming() {
    // Test that limits are enforced during streaming
}
```

---

### Test Migration Checklist

- [ ] Update `readers/mod.rs` inline tests (18 tests modified)
- [ ] Add new reader tests in `readers/mod.rs` (15+ new tests)
- [ ] Update `body_reader.rs` iteration patterns (23 locations)
- [ ] Update `body_reader.rs` tests (~20 tests)
- [ ] Update event source tests (4+ files)
- [ ] Update simple_http request tests
- [ ] Update simple_http middleware tests
- [ ] Update simple_http compression tests
- [ ] Create streaming readers integration tests
- [ ] Verify all tests pass with `cargo test`
- [ ] Document test patterns in migration guide

## Success Criteria

### Implementation
- [ ] `BatchStreamReader` exposes `Data` (no loop)
- [ ] `FullBodyReader` converted to `Data`-exposing iterator
- [ ] `EofReader` converted to `Data`-exposing iterator
- [ ] `LimitedBatchStreamReader` implemented with `Data` exposure
- [ ] `EOFStreamReader` implemented with `Data` exposure
- [ ] `LimitedEOFStreamReader` implemented with `Data` exposure
- [ ] `DataBytesIterator` implemented for backward compatibility
- [ ] `IntoDataBytes` extension trait implemented
- [ ] `SendSafeBody::Stream` type changed to `Iterator<Item=Result<Data, BoxedError>>`

### Body Integration
- [ ] `collect_string`, `collect_bytes` updated to handle `Data`
- [ ] All 23 usages in body_reader.rs updated
- [ ] All body extraction returns streaming readers (no eager Bytes)

### Tests
- [ ] 18 inline tests in readers/mod.rs updated
- [ ] 15+ new tests for new readers added
- [ ] body_reader.rs iteration patterns updated (23 locations)
- [ ] Event source tests updated (4+ files)
- [ ] Simple HTTP tests updated (request, middleware, compression)
- [ ] New integration test file created
- [ ] All tests pass with `cargo test`

### Documentation
- [ ] `full_body_threshold` deprecated/removed
- [ ] Migration guide updated with before/after examples
- [ ] Test patterns documented

## Related Files

| File | Lines | Changes |
|------|-------|---------|
| `backends/foundation_core/src/io/readers/mod.rs` | All reader impls | Convert to Data-exposing, add new readers |
| `backends/foundation_core/src/wire/simple_http/impls.rs` | 189, 202, 307, 397, 1647, 1834, 2148, 2501, 3546-3547, 4032-4033, 5162-5332 | Update for Data enum |
| `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` | 175-234, 336-417, 496-538, 761-800, 858-909, 1168-1235, 1595-1703 | Update iteration patterns |

## Breaking Change Notice

**This is a breaking change.** All code that:
1. Matches on `SendSafeBody::Stream` expecting `Vec<u8>`
2. Creates stream iterators manually
3. Assumes automatic retry absorption

Must be updated. Use `DataBytesIterator` to minimize changes.
