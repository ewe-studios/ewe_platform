# GCP Cloud Run Provider - Learnings

This document captures key learnings from implementing and debugging GCP API integration, particularly around HTTP chunked transfer encoding and the GCP Discovery API.

---

## 2026-04-03: HTTP Chunked Transfer Encoding Bug

### Problem

The `test_real_gcp_captured_response` test was failing when parsing a 5.8MB captured GCP Discovery API response. The test expected ~191 chunks totaling 5.8MB, but only received 2 chunks totaling 65,535 bytes.

### Root Cause

The bug was in `backends/foundation_core/src/wire/simple_http/impls.rs` in the `eat_escaped_crlf_pointer` function:

```rust
fn eat_escaped_crlf_pointer<T: Read>(
    acc: &mut ByteBufferPointer<T>,
) -> Result<(), ChunkStateError> {
    while let Ok(b) = acc.nextby2(2) {
        if b != b"\\r" && b != b"\\n" {
            let _ = acc.unforward_by(2);
            acc.skip();
            break;
        }
    }
    Ok(())
}
```

This function was designed to eat literal escape sequences (`\r` = 0x5c 0x72 and `\n` = 0x5c 0x6e) that might appear in non-standard chunked responses. However, it was being called **after parsing chunk headers**, which caused it to consume actual JSON content bytes that happened to match these patterns.

**Example from GCP response:**
```
8000\r\n
\n\nCurrently a...
```

After parsing chunk size `8000\r\n`, the parser should be positioned at the start of chunk data. But `eat_escaped_crlf_pointer` consumed the first 4 bytes (`\n\n` = two literal backslash-n sequences in the JSON), misaligning the parser by 4 bytes for every subsequent chunk.

### Why This Happened

1. GCP Discovery API sends different API spec versions to different clients
2. The version sent to our client contains stray CR bytes and literal `\n` escape sequences in JSON string values
3. The parser was designed for a non-standard extension but broke standard RFC 7230 parsing
4. The 4-byte offset compounded across chunks, eventually causing the parser to read "00" instead of actual chunk sizes, terminating early

### Fix

Removed the call to `eat_escaped_crlf` in the chunk header parser:

```rust
// Before (buggy)
Self::eat_space(pointer.clone())?;
Self::eat_crlf(pointer.clone())?;
Self::eat_escaped_crlf(pointer.clone())?;  // <-- This was consuming JSON content
Self::eat_newlines(pointer.clone())?;

// After (fixed)
Self::eat_space(pointer.clone())?;
Self::eat_crlf(pointer.clone())?;
// Note: eat_escaped_crlf removed - was consuming legitimate JSON content
// that contained literal \n sequences as part of escaped strings
Self::eat_newlines(pointer.clone())?;
```

Also removed the unused `eat_escaped_crlf_pointer` and `eat_escaped_crlf` functions from the `ChunkState` implementation.

### Test Coverage Added

Added three new integration tests for non-standard line endings:

1. **`test_lf_only_line_endings`** - Tests LF-only (`\n`) instead of standard CRLF
2. **`test_mixed_line_endings`** - Tests mixed CRLF and LF across chunks
3. **`test_double_lf_line_endings`** - Tests double-LF (`\n\n`) terminators
   — *replaced 2026-07-17* by `test_lf_framing_keeps_leading_lf_in_data`:
   `<size>\n\n<data>` is not a framing any server sends, and accepting it is
   indistinguishable from `<size>\n` + data that begins with LF, so it cost
   binary correctness for a case that does not occur.

All tests pass, confirming the parser handles both standard and non-standard line endings correctly.

### Files Changed

- `backends/foundation_core/src/wire/simple_http/impls.rs` - Fixed chunk parser
- `backends/foundation_core/tests/chunked_encoding.rs` - Added tests
- `backends/foundation_core/src/io/ioutils/mod.rs` - Removed debug tracing

> **Follow-up 2026-07-17:** this fix was right but incomplete — see
> *"2026-07-17: the rest of the greedy chunk-header eaters"* below.

---

## 2026-07-17: the rest of the greedy chunk-header eaters

### What this entry is for

If you are here because **GCP Discovery JSON has stray CR bytes**: the
parser-level CR strip that used to hide that is **gone**, on purpose. Read
[`CR_BYTE_INVESTIGATION.md`](./CR_BYTE_INVESTIGATION.md) → *"Update 2026-07-17"*
before adding anything. Short version: re-test first (the CRs may have been our
own bug), and if they are real, strip them **in the GCP fetch layer** where the
body is known to be JSON — never in `foundation_netio`'s chunked parser, which
cannot know a `0x0D` from a length byte.

### The April fix caught one of three

The April entry above removed `eat_escaped_crlf` from the chunk header parser
because it "consumed legitimate JSON content". Correct — but two eaters of the
same shape were left in place:

```rust
Self::eat_space(pointer.clone())?;
Self::eat_crlf(pointer.clone())?;      // ← loops until a non-CR/LF byte
Self::eat_newlines(pointer.clone())?;  // ← same
```

RFC 7230 §4.1 is `chunk-size [ chunk-ext ] CRLF chunk-data CRLF` — **exactly one**
terminator after the header. Looping past it eats the first bytes of the chunk
*data* whenever that data starts with CR or LF, after which `read_exact(size)`
runs late and pulls the framing delimiter in as content. A chunk carrying
`"\rstart"` came back as `"start\r"`.

That is very likely where GCP's "stray CRs" came from: a chunk boundary landing
mid-word plus a leaked delimiter byte yields exactly the reported
`cor\rresponding` / `PART\rIAL`. Curl saw zero CRs from the same endpoint.

**Fixed:** the parser now consumes exactly one terminator (CRLF, or a bare LF for
LF-only servers) and returns chunk data untouched.

### Why it surfaced now

Docker's log stream (`application/vnd.docker.multiplexed-stream`) frames each
write as `[stream, 0,0,0, size_be32] + payload`. A **13-byte** log line puts a
literal `0x0D` in the size field — which the CR strip deleted, desyncing the
frame so container logs came back empty. That broke spec-53's F03 volume tests
and would have broken any image build/pull stream too.

### The rule

The transport moves bytes; it does not clean them. Only the layer that knows the
payload's format (this provider, for its JSON) can decide a byte is noise.

### Files Changed

- `backends/foundation_netio/src/shared/http/impls.rs` — removed the CR strip;
  chunk-header terminator consumption is now exactly one CRLF/LF
- `backends/foundation_netio/tests/simple_http/chunked_tests.rs`,
  `chunked_encoding.rs` — tests inverted to assert byte-exact round-tripping
- `backends/foundation_netio/tests/simple_http/gcp_chunked_expected.bin` —
  regenerated to the 171 bytes the capture carries (was 162, CR-stripped)

---

## 2026-04-03: GCP API Filter Feature

### Requirement

The GCP Discovery API returns 510+ APIs, but most applications only need a subset. Initially, we hard-coded a filter list, but this was inflexible.

### Solution

Added `--gcp-apis` CLI argument to `gen_provider_specs` command:

```bash
# Fetch all APIs (default behavior)
ewe_platform gen_provider_specs

# Fetch only specific APIs
ewe_platform gen_provider_specs --gcp-apis=compute,storage,iam

# Fetch for specific provider with API filter
ewe_platform gen_provider_specs --provider=gcp --gcp-apis=compute,container
```

### Implementation

Modified `fetch_gcp_specs` function signature:

```rust
pub fn fetch_gcp_specs(
    client: &SimpleHttpClient,
    output_dir: PathBuf,
    api_filter: Option<Vec<String>>,  // NEW: optional filter
) -> Result<impl StreamIterator<...>, DeploymentError>
```

When `api_filter` is `None`, all APIs are fetched. When `Some(vec)`, only matching APIs are fetched.

### Files Changed

- `bin/platform/src/gen_provider_specs/mod.rs` - Added CLI argument
- `bin/platform/src/gen_provider_specs/fetcher.rs` - Pass filter through
- `backends/foundation_deployment/src/providers/resources/gcp/fetch.rs` - Apply filter

---

## Technical Deep Dives

### HTTP Chunked Transfer Encoding (RFC 7230 Section 4.1)

Standard format:
```
<chunk-size-in-hex>\r\n
<chunk-data>\r\n
<chunk-size-in-hex>\r\n
<chunk-data>\r\n
0\r\n
\r\n
```

Key insight: After parsing the chunk size line (`<size>\r\n`), the parser should be positioned exactly at the start of chunk data. Any additional byte consumption at this point will misalign all subsequent parsing.

### ByteBufferPointer Buffer Management

The `SharedByteBufferStream` uses a fill/truncate cycle:
1. Fill buffer from underlying reader
2. Track `pos` (consumed) and `peek_pos` (read but not consumed)
3. Truncate when 40%+ consumed to free space
4. Refill as needed

Debug tracing was added to track buffer positions across truncate operations, which helped map file positions to buffer positions during investigation.

### Escape Sequence Handling

The problematic function was designed for escape sequences like:
```
31\r\n
{\r\n
  "field"\r
}
```

Where literal `\r` in JSON values might need special handling. However, standard HTTP chunked encoding doesn't define such escapes - content bytes should pass through unchanged.

---

## Debugging Techniques Used

1. **`#[traced_test]` macro** - Added structured logging to tests without manual setup
2. **Buffer position tracing** - Tracked `pos`, `peek_pos`, and `buffer.len()` across operations
3. **File position mapping** - Correlated buffer positions with file offsets to identify exactly which bytes were being misread
4. **Binary fixture analysis** - Used `xxd` to examine raw chunked response bytes and verify expected vs actual parsing

---

## Best Practices Established

1. **Never consume bytes after chunk size header** - Parser must be positioned exactly at chunk data start
2. **Test with real captured responses** - Synthetic tests may not catch edge cases in real server responses
3. **Prefer standard compliance over non-standard extensions** - RFC 7230 is well-defined; extensions should be opt-in
4. **Add integration tests for edge cases** - Non-standard line endings, embedded control characters, large responses
5. **Use tracing over eprintln** - Structured logging integrates better with test frameworks and can be toggled

---

## Related Documentation

- `CR_BYTE_INVESTIGATION.md` - Detailed investigation of CR byte handling
- `BUG_ANALYSIS.md` - Full bug analysis and resolution timeline
- `backends/foundation_core/tests/chunked_encoding.rs` - Integration test suite
