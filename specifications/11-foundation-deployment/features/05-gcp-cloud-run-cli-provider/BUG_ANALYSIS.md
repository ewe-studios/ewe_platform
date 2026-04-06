# Bug Analysis: GCP Chunked HTTP Response Parsing Failure

## Executive Summary

The GCP Discovery API fetch was failing due to incorrect handling of LF-only (`\n`) line endings in HTTP chunked transfer encoding. The chunked decoder expected strict RFC 7230 CRLF (`\r\n`) line endings, but GCP sends non-standard LF-only terminators.

**Impact:** All GCP API spec fetches failed with JSON parsing errors, preventing cloud provider integration.

**Root Cause:** The `SimpleHttpChunkIterator` in `impls.rs` consumed bytes incorrectly when parsing chunk boundaries with LF-only terminators, causing data corruption.

---

## Technical Background

### HTTP Chunked Transfer Encoding (RFC 7230 Section 4.1)

Standard chunked encoding format:
```
<chunk-size>\r\n
<chunk-data>\r\n
<chunk-size>\r\n
<chunk-data>\r\n
0\r\n
\r\n
```

Each chunk consists of:
1. Size in hexadecimal followed by CRLF
2. Chunk data (exactly `<chunk-size>` bytes)
3. Trailing CRLF

### GCP's Non-Standard Implementation

GCP Discovery API sends LF-only line endings:
```
<chunk-size>\n
<chunk-data>\n
0\n
\n
```

This is technically a protocol deviation but is accepted by many HTTP clients.

---

## Bug Analysis

### Original Symptom

```
Error: "Invalid chunked encoding: expected CRLF (0x0d 0x0a) after chunk data, got [0a, 36]"
```

The error shows:
- `0a` = `\n` (LF)
- `36` = ASCII `'6'` (hex digit for next chunk size)

The 2-byte CRLF read was consuming `\n` + first hex digit of the next chunk header.

### Cascade of Failed Fix Attempts

#### Attempt 1: Accept LF+hex as valid CRLF

**Change:** Modified CRLF validation to accept `[b'\n', hex_digit]` as valid.

**Problem:** Both bytes were consumed, losing the hex digit from the next chunk size.

**Result:** Chunk size parsing went out of sync after several chunks, causing progressive data corruption.

#### Attempt 2: Put back hex digit with `unforward()`

**Change:** After detecting LF+hex, call `unforward()` + `skip()` to restore the hex digit.

**Problem:** The CRLF was being read with `reader.read()` which consumes bytes from the buffer. Calling `unforward()` afterward couldn't restore bytes already consumed by the underlying stream read.

**Result:** Hex digits were still lost, corruption continued.

#### Attempt 3: Use `nextby2` instead of `read()`

**Change:** Switched to `nextby2` for CRLF reading to keep bytes in the buffer for `unforward()`.

**Problem:** Introduced new bug - stray `\r` characters appeared in output data.

**Root cause discovered:** When chunk data ends with `\r` (as part of content, not line terminator), and the trailer is LF-only, the inter-chunk CRLF consumption at the start of `next()` was incorrectly handling the boundary.

#### Attempt 4: Strip `\r` from output in body_reader.rs

**Change:** Added `.filter(|&b| b != b'\r')` to remove stray `\r` characters from chunked stream output.

**Problem:** This is a workaround, not a fix. Also introduces potential OOM issues by collecting all data before filtering.

**Result:** "Worked" but is architecturally wrong - the chunked parser should handle this correctly, not the body collector.

---

## Data Corruption Pattern Analysis

### Observed Corruption

After various fix attempts, stray `\r` characters appeared at positions:
```
Position 2996792: b' which replaces\r the list of r'
Position 4578476: b'}\\\\.)*(?:[a-z](\r?:[-a-z0-9]{0,'
Position 5776365: b'ch the entire f\rield.\\n\\nFor e'
Position 6484201: b'list of router \rbgp routes ava'
Position 6549737: b'\n            }\n\r          },\n '
```

All `\r` characters appeared mid-word, proving they were introduced during decoding (original GCP response has zero `\r` characters).

### Size Analysis

```
Original GCP alpha API:  7,151,215 bytes
Our decoded output:      7,151,215 bytes (with 5 stray \r)
Expected (no corruption): 7,151,210 bytes (5 bytes less = 5 stray \r removed)
```

After removing stray `\r`, JSON content matched exactly (different key ordering only).

---

## Code Locations

### Primary Bug Location

`backends/foundation_core/src/wire/simple_http/impls.rs`

**Function:** `SimpleHttpChunkIterator::next()` - trailing CRLF consumption logic (lines ~5105-5170)

**Original buggy code:**
```rust
// Read CRLF with read() - consumes bytes irreversibly
let mut crlf_buffer = [0u8; 2];
let mut crlf_bytes_read = 0;
while crlf_bytes_read < 2 {
    match reader.read(&mut crlf_buffer[crlf_bytes_read..]) {
        // ...
    }
}

// Accept LF+hex but bytes already consumed
let crlf_ok = crlf_buffer == [b'\r', b'\n']
    || crlf_buffer == [b'\n', b'\n']
    || (crlf_buffer[0] == b'\n' && crlf_buffer[1].is_ascii_hexdigit());
```

### Secondary Issue Location

`backends/foundation_core/src/wire/simple_http/impls.rs`

**Function:** Inter-chunk CRLF consumption at start of `next()` (lines ~5019-5035)

**Code:**
```rust
// Consumes ALL consecutive \r and \n between chunks
while let Ok(b) = reader.nextby2(1) {
    if b[0] == b'\r' || b[0] == b'\n' {
        continue;  // Consume
    }
    let _ = reader.unforward();
    reader.skip();
    break;
}
```

This is correct behavior, but interacts badly with incorrect trailing CRLF handling.

---

## Correct Fix Strategy

### Key Insight

The chunked parser operates in real-time as data arrives. When consuming the trailing CRLF after chunk data:

1. If we see `\r\n` - consume both (standard RFC 7230)
2. If we see `\n\n` - consume both (double-LF quirk)
3. If we see `\n` followed by hex digit - consume ONLY the `\n`, leave the hex digit for next chunk size parsing
4. If we see `\r` NOT followed by `\n` - this is an error (invalid chunk framing)

### Proper Implementation

Use `peekby2` to inspect bytes WITHOUT consuming, then use `nextby2` + `skip()` to consume only what's needed:

```rust
// Peek at first byte
let first = reader.peekby2(1)?[0];

if first == b'\n' {
    // LF-only terminator
    // Peek at second byte to check what follows
    if let Ok(second) = reader.peekby2(2).and_then(|b| b.get(1).copied()) {
        if second.is_ascii_hexdigit() {
            // LF followed by chunk size - consume only LF
            let _ = reader.nextby2(1);
            reader.skip();
            return;
        }
    }
}
```

### Why This Works

1. `peekby2` advances `peek_pos` but NOT `pos`
2. `nextby2` advances `peek_pos` to capture bytes
3. `skip()` advances `pos` to match `peek_pos`, effectively consuming
4. If we only call `nextby2(1)` + `skip()`, we consume exactly 1 byte
5. The second byte remains available for the next iteration's parsing

---

## Lessons Learned

1. **Never use `read()` for protocol parsing** when you need to potentially "put back" bytes - it consumes from the underlying buffer irreversibly.

2. **Use `peekby2` + `nextby2` + `skip()` pattern** for conditional byte consumption in streaming parsers.

3. **Test with real server data** - synthetic tests passed because they used properly formatted CRLF.

4. **Don't add workarounds in body_reader.rs** - fix the root cause in the protocol parser.

5. **Buffer management matters** - collecting all data before filtering (like `.filter(|b| b != b'\r')`) can cause OOM on large responses.

---

## Test Cases Required

1. **LF-only chunk terminator followed by hex digit** - verifies hex digit is preserved
2. **CRLF chunk terminator** - verifies standard RFC 7230 still works
3. **Double-LF between chunks** - handles server quirks
4. **Chunk data ending with `\r`** - verifies content `\r` is preserved
5. **Multi-chunk LF-only stream** - verifies no cumulative corruption
6. **Mixed CRLF and LF streams** - verifies robustness

---

## Files Modified (To Be Reverted and Fixed Properly)

### To Revert:
- `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` - Remove `\r` filtering workaround
- `backends/foundation_core/src/wire/simple_http/impls.rs` - Remove broken CRLF handling

### To Fix Properly:
- `backends/foundation_core/src/wire/simple_http/impls.rs` - Implement correct peek-based trailing CRLF consumption
- `backends/foundation_core/src/wire/simple_http/chunked_tests.rs` - Add LF-only test cases

---

## Verification Commands

```bash
# Test GCP fetch
cargo run --bin ewe_platform gen_provider_specs --provider gcp --debug

# Verify output
python3 -c "
import json
data = open('artefacts/cloud_providers/gcp/openapi.json', 'rb').read()
print(f'Size: {len(data)} bytes')
print(f'CR count: {data.count(b\"\r\")}')
json.loads(data)  # Should not raise
print('JSON valid!')
"

# Run chunked tests
cargo test --package foundation_core -- chunked
```

---

## Acceptance Criteria

- [ ] All 3 GCP compute APIs (alpha, beta, v1) fetch successfully
- [ ] Output JSON is valid with zero stray `\r` characters
- [ ] No memory-inefficient filtering (no collecting full body before processing)
- [ ] All existing chunked encoding tests pass
- [ ] New LF-only test cases pass
- [ ] Chunk parser handles real-time streaming correctly (no backtracking)
