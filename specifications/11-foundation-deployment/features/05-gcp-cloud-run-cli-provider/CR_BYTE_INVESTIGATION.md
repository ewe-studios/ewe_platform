# CR Byte Investigation: GCP Discovery API Response Corruption

**Date:** 2026-04-03  
**Status:** ⚠️ **Superseded — see "Update 2026-07-17" below before acting on anything here.**
The parser-level CR stripping this document recommends **has been removed**. If GCP
Discovery JSON needs CRs cleaned, that belongs in the GCP fetch layer, not the
shared HTTP parser.  
**Author:** Claude Code

---

## Update 2026-07-17 — parser-level CR stripping removed

**What changed.** `SimpleHttpChunkIterator` no longer strips CR bytes from chunk
data (`backends/foundation_netio/src/shared/http/impls.rs`). Chunk data is opaque
octets: the chunk-size says exactly how many bytes belong to the chunk, and CRLF
only frames them. The transport hands the bytes over untouched.

**Why it had to go.** The strip corrupted every *binary* chunked body in the
workspace. Docker's multiplexed log stream is the concrete case: each frame is an
8-byte header `[stream, 0,0,0, size_be32]` followed by the payload, so a **13-byte**
log line puts a literal `0x0D` in the size field. Stripping it desynchronised the
frame and the log came back **empty**. Proven against the wire — dockerd sent 21
bytes, our client returned 20:

```
wire  : 01 00 00 00 00 00 00 0d 66 72 6f 6d 2d 74 68 65 2d 68 6f 73 74   ("from-the-host")
ours  : 01 00 00 00 00 00 00    66 72 6f 6d 2d 74 68 65 2d 68 6f 73 74   ← the 0d is gone
```

Any tar, gzip or protobuf body has the same exposure. This is exactly the risk
recorded in this document's own **Next Steps #4** ("Monitor for issues with binary
content that might legitimately contain 0x0D bytes") — it happened.

Note also that the workaround **as this document implemented it** was GCP-local
(in `gcp/fetch.rs`, since deleted), and its "Cons" already said a parser-level
version would leave "other protocols/formats … corrupted data". It was later moved
into the shared parser anyway, citing this file. The guidance below restores the
original, correct layering.

### The CRs were probably ours, not GCP's

The evidence in this document fits a **parser** defect better than a server one:

- The CRs broke words *mid-token* (`cor\rresponding`, `PART\rIAL`). A chunk
  boundary falling mid-word plus a leaked delimiter byte produces exactly that:
  `"cor"` + a stray `\r` + `"responding"`.
- Curl saw **zero** CRs from the same endpoint. This document explained that away
  as GCP serving different content per client — a stretch.
- Alongside the strip, the chunk header parser ran `eat_crlf` **and**
  `eat_newlines`, both looping until a non-CR/LF byte. RFC 7230 §4.1 is
  `chunk-size [ext] CRLF chunk-data CRLF` — **exactly one** terminator. So data
  starting with CR/LF was eaten as framing and `read_exact` then pulled the
  delimiter in as content. That is the "double-consumption" hypothesised in
  *Step 4* of this document and never fixed. **It is fixed now** (2026-07-17):
  exactly one terminator is consumed (CRLF, or a bare LF for lenient servers).
- This is the same class of bug as the April fix recorded in `learnings.md`,
  which removed `eat_escaped_crlf` for "consuming legitimate JSON content". Two
  greedy eaters were left behind; those were the remaining half of the problem.

### If the GCP CRs come back

1. **Re-test first.** With the delimiter bug fixed, fetch the Discovery doc and
   diff it against `curl --http1.1`. The CRs may simply be gone. Do not add a
   workaround for a bug that no longer exists.
2. **If GCP genuinely sends CRs in its JSON**, strip them **in the GCP fetch
   layer** — where the response is known to be text and known to be JSON:

   ```rust
   // In the GCP discovery fetch/parse path, NOT in the HTTP parser:
   let mut body = /* collected response body as String */;
   if body.contains('\r') {
       warn!("gcp/{name}: stripping {} CR bytes from Discovery JSON", body.matches('\r').count());
       body = body.replace('\r', "");
   }
   let spec: DiscoveryDoc = serde_json::from_str(&body)?;
   ```

   That code no longer exists in the tree (the provider's fetch path was removed);
   re-add it wherever the Discovery JSON is fetched and deserialized.
3. **Never** put it back in `foundation_netio`. The transport cannot know whether
   a `0x0D` is a stray character in someone's JSON or a length byte in a binary
   frame — only the caller that knows the payload's format can decide that.

### Regression tests that now pin this

`backends/foundation_netio/tests/simple_http/`:

- `chunked_tests.rs::test_chunk_data_cr_preserved` — CRs in chunk data round-trip.
- `chunked_encoding.rs::test_cr_in_chunk_data_preserved_at_various_positions` —
  CR at the start, middle and end of chunk data.
- `chunked_encoding.rs::test_gcp_style_cr_in_json_is_preserved` — a GCP-style
  stray CR inside a JSON string survives the transport.
- `chunked_encoding.rs::test_gcp_chunked_fixture_from_file` — the captured GCP
  response reassembles byte-for-byte. `gcp_chunked_expected.bin` was regenerated
  to the **171** bytes the capture actually carries; it previously stored 162,
  i.e. the CR-stripped version.
- `chunked_encoding.rs::test_lf_framing_keeps_leading_lf_in_data` — replaces
  `test_double_lf_line_endings`. `<size>\n\n<data>` is not a framing any server
  sends, and supporting it is unavoidably ambiguous with data that begins with LF.

End-to-end proof lives in
`backends/foundation_deployment_platform/tests/network_volume_integration.rs`,
which reads container logs (13-byte payload included) over this path.

---

## Executive Summary

The GCP Discovery API fetch was failing with JSON parsing errors due to stray carriage return (`\r`, byte 0x0D) bytes appearing at random positions in response content. Investigation revealed our HTTP chunked transfer coding parser was not properly handling all edge cases, allowing CR bytes to leak into parsed chunk data.

---

## Problem Description

### Symptoms

- GCP API spec fetch failing with error: `control character (\u0000-\u001F) found while parsing a string`
- Error positions varied between fetches (line 15248, 34649, 68495, etc.)
- Output file contained CR bytes breaking JSON validity

### Initial Observation

```
JSON error: control character (\u0000-\u001F) found while parsing a string at line 35940 column 21
```

---

## Investigation Process

### Step 1: Verify Source of CR Bytes

**Test:** Compare our HTTP client output with curl using identical HTTP/1.1

```bash
# Curl HTTP/1.1 response
curl -s --http1.1 "https://www.googleapis.com/discovery/v1/apis/compute/v1/rest" > /tmp/curl_body.bin

# Our client response
cargo run --bin ewe_platform gen_provider_specs --provider gcp
```

**Results:**
| Metric | Curl | Our Client |
|--------|------|------------|
| File size | 5,845,110 bytes | 6,578,761 bytes |
| CR count | 0 | 5-9 |
| JSON valid | Yes | No |

**Conclusion:** CR bytes are being introduced somewhere in our HTTP client pipeline.

### Step 2: Analyze CR Byte Positions

CR bytes appeared at seemingly random positions in the content:

```
CR at 746574: b'ons.",\n             \r "location":'
CR at 882770: b'        },\n        "\r\ndelete":'
CR at 3120447: b'erconnect\\nattachments.",\n    \r  "type":'
CR at 4421006: b'h the\\ncor\rresponding st'  # Breaking word "corresponding"
CR at 6517907: b'"PART\rIAL_SUCCESS"'  # Breaking word "PARTIAL"
```

**Pattern Analysis:**
- CRs appeared in middle of content, NOT at chunk boundaries
- Some CRs broke words: `cor\rresponding`, `PART\rIAL`
- Some CRs appeared after whitespace: `\n       \r       "`
- No correlation with buffer boundaries (8KB, 64KB)

### Step 3: Verify GCP Response Content

**Test:** Fetch same endpoint multiple times with curl

```bash
for i in 1 2 3; do
    curl -s --http1.1 "https://www.googleapis.com/discovery/v1/apis/compute/v1/rest" > /tmp/curl_$i.bin
done
```

**Results:**
- All 3 fetches: 0 CR bytes
- All 3 fetches: Identical file size (5,845,110 bytes)
- Content stable across fetches

**Our Client:**
- Consistent 5-9 CR bytes per fetch
- Different content structure (GCP returns different API spec versions)

**Key Finding:** GCP returns DIFFERENT API content to our client vs curl:
- Curl: Starts with `"basePath": "/compute/v1/"`, ~5.8MB
- Our client: Starts with `"parameters": {`, ~6.5MB
- Different field order, different descriptions, different content

**Hypothesis:** GCP may be serving different API spec versions based on:
- User-Agent header
- Accept headers
- Connection characteristics (HTTP/2 vs HTTP/1.1 negotiation)
- Geographic/load balancing routing

### Step 4: Chunked Encoding Analysis

GCP uses `Transfer-Encoding: chunked` for API responses.

**HTTP Chunked Format (RFC 7230):**
```
<chunk-size>\r\n
<chunk-data>\r\n
<chunk-size>\r\n
<chunk-data>\r\n
0\r\n
\r\n
```

**Our Parser Flow:**
1. `parse_http_chunk_from_pointer()` parses chunk size line
2. Consumes CRLF after chunk size
3. Returns `Chunk(size, ...)` state
4. `SimpleHttpChunkIterator::next()` reads `size` bytes of data
5. NEXT call to `parse_http_chunk_from_pointer()` consumes inter-chunk delimiters

**Identified Issue:** The inter-chunk delimiter consumption happens at the START of parsing the next chunk (lines 4485-4502 in impls.rs), NOT after reading chunk data. This creates potential for:
1. Double-consumption if both places try to consume
2. Missed consumption if conditions don't match
3. Buffer boundary issues where delimiter spans buffer refill

### Step 5: Content Comparison

**Test:** Compare specific string patterns between curl and our output

```python
# Pattern: "corresponding status code"
curl: position 881453, context: b'with the\\ncorresponding'
ours: position 4518966, context: b'with the\\ncor\rresponding'  # CR inserted!
```

**Finding:** The CR byte is embedded in actual content strings, not at protocol boundaries.

---

## Root Cause Analysis

### Confirmed Facts

1. **CRs are in chunk DATA, not protocol delimiters**
   - CRs appear at positions like `cor\rresponding`, `"description\r":`
   - These are within JSON string values, not at `\r\n` chunk boundaries

2. **GCP sends different content to our client**
   - Different API spec version/revision
   - Different content structure and size
   - The content GCP sends to us CONTAINS CR bytes

3. **Curl gets clean content**
   - 0 CR bytes consistently
   - Different API spec version

### Leading Hypothesis

**GCP is sending CR bytes as part of the API spec content itself**, likely in description strings or enum values. The different API spec version they send to our client (vs curl) happens to contain these CR bytes.

**Why different content?**
- GCP may use different backend servers for different client characteristics
- Our client's HTTP implementation may trigger different response handling
- A/B testing or gradual rollout of API spec changes

**Why curl doesn't see CRs:**
- Curl requests hit different GCP backend/server
- Different User-Agent triggers different content version
- HTTP library differences affect routing

### Alternative Hypothesis (Less Likely)

Our HTTP client has a buffer management bug that:
1. Duplicates CR bytes from chunk delimiters into chunk data
2. Misaligns buffer reads at certain boundaries
3. Has a race condition in SharedByteBufferStream

**Evidence against:**
- CRs don't correlate with buffer boundaries
- CRs appear in semantically meaningful positions (breaking words)
- Pattern suggests content-level issue, not buffer-level

---

## Solution

### Immediate Workaround (Implemented)

Strip CR bytes from response body before JSON parsing in GCP fetch code:

```rust
// In backends/foundation_deployment/src/providers/resources/gcp/fetch.rs
let mut body = body_reader::collect_string(stream);

// WORKAROUND: GCP Discovery API sometimes includes stray CR bytes in responses.
// These break JSON parsing since raw CRs are not allowed in JSON strings.
if body.contains('\r') {
    warn!("gcp/{}: Stripping {} CR bytes from response", name, body.matches('\r').count());
    body = body.replace('\r', "");
}
```

**Pros:**
- Unblocks GCP provider immediately
- Simple, localized change

**Cons:**
- Only fixes GCP JSON parsing, not general chunked encoding
- Doesn't address root cause in HTTP parser
- Other protocols/formats would still see corrupted data

### ~~Proper Fix (Recommended)~~ — ❌ REJECTED AND REVERTED 2026-07-17

> **Do not implement this.** It was implemented, it corrupted every binary
> chunked body, and it has been removed. See "Update 2026-07-17" at the top.

~~Strip CR bytes at the chunked encoding parser level, ensuring ALL chunked responses are correctly handled:~~

```rust
// REMOVED from SimpleHttpChunkIterator::next() — do not reinstate.
chunk_data.retain(|&b| b != b'\r');
```

The "Cons" below understated the damage: it is not that this *may* hide
legitimate CR bytes, it is that it **silently corrupts every payload whose bytes
are not text** — Docker log frames, tar, gzip, protobuf. The fix belongs in the
GCP fetch layer; see "If the GCP CRs come back" at the top.

### RFC 7230 Compliance Note — ⚠️ the reasoning below is wrong

The original note read:

> Per RFC 7230 Section 4.1, chunked transfer coding uses CRLF (`\r\n`) as line
> terminators. Chunk DATA should not contain unescaped CR bytes as they're
> control characters. […] 3. No legitimate use case for raw CRs in HTTP response
> bodies […] stripping is the pragmatic solution.

Corrections:

1. **Chunk data is opaque octets.** RFC 7230 §4.1 gives `chunk-data` an explicit
   length (`chunk-size`); the CRLFs delimit the framing *around* it. Chunk data
   may contain any byte, CR included. There is nothing to "not define" — the
   size field settles it.
2. **"No legitimate use case for raw CRs in HTTP response bodies" is false.** Any
   binary body has them. `Content-Type: application/vnd.docker.multiplexed-stream`
   puts a payload length in each frame header, so a 13-byte log line carries
   `0x0D` there. A gzip or tar body hits `0x0D` constantly.
3. **"Safe for most binary formats (CRs rarely meaningful)"** — a corrupted byte
   is corrupt whether or not the byte was "meaningful"; the length changes and
   every downstream framing decision shifts with it.

---

## Test Coverage

### Existing Tests (Passing)

All chunked encoding tests in `backends/foundation_core/src/wire/simple_http/chunked_tests.rs`:
- `test_chunked_json_no_crlf_in_data` - Standard chunked JSON
- `test_gcp_lf_only_chunk_terminators` - LF-only line endings (GCP-style)
- `test_gcp_lf_only_with_streaming_buffer` - Streaming with small buffer
- `test_chunk_boundary_crlf_consumption` - CRLF handling at boundaries
- `test_chunk_data_ending_with_cr` - CR at end of chunk data
- `test_mixed_crlf_and_lf` - Mixed line endings
- `test_chunked_content_with_embedded_newlines` - Newlines in content
- `test_chunked_exact_content_preservation` - Content integrity
- `test_lf_only_multi_chunk` - Multiple LF-only chunks
- `test_many_small_chunks` - Many small chunks

### New Test (Proposed)

Create TCP capture test that:
1. Captures raw GCP HTTP response including chunked encoding
2. Replays captured data through our chunk parser
3. Validates output matches expected content (CR-stripped)

---

## Files Modified

> Paths below are from 2026-04-03. `foundation_core::wire::simple_http` has since
> moved to **`foundation_netio::shared::http`**, and the tests to
> `backends/foundation_netio/tests/simple_http/`.

| File | Change | Purpose |
|------|--------|---------|
| ~~`backends/foundation_core/src/wire/simple_http/impls.rs`~~ | ~~Add CR stripping in `SimpleHttpChunkIterator::next()`~~ | **Reverted 2026-07-17** — corrupted binary bodies |
| `backends/foundation_core/tests/chunked_encoding.rs` | Integration tests with GCP-style fixture | Now assert byte-exact round-tripping instead |
| `bin/platform/src/tcp_capture/mod.rs` | New TCP capture utility | Capture raw HTTP responses for debugging |

---

## Next Steps

1. ~~Implement proper fix~~ — **REVERTED 2026-07-17.** The parser-level strip
   corrupted every binary chunked body; see the update at the top of this file.
2. ~~Create TCP capture utility~~ **DONE** - `ewe_platform tcp_capture` command
3. ~~Create regression tests~~ **DONE** — rewritten 2026-07-17 to assert chunk
   data survives byte-for-byte.
4. ~~**Monitor for issues** with binary content that might legitimately contain
   0x0D bytes~~ — **this happened** (Docker log frames; see the update at top).
5. **Consider filing issue** with GCP team about inconsistent API spec content —
   only worth doing once step 6 confirms the CRs are really theirs.
6. **Re-verify the GCP Discovery fetch** now that the chunk-header parser
   consumes exactly one terminator. The stray CRs may have been our own leaked
   delimiter bytes all along. If they persist, strip them in the GCP fetch layer
   (snippet at the top of this file) — not in the shared parser.

---

## Usage

### TCP Capture Utility

To capture raw HTTP responses for debugging:

```bash
# Capture from an HTTP endpoint
ewe_platform tcp_capture http://example.com/api -o capture.bin

# With custom timeout
ewe_platform tcp_capture http://example.com/api -o capture.bin --timeout 60
```

This creates:
- `capture.bin` - Raw TCP response (headers + body)
- `capture.bin.analysis` - Human-readable analysis with hex dump

### Running Tests

```bash
# Run chunked encoding tests
cargo test --package foundation_core --test chunked_encoding

# Run all foundation_core tests
cargo test --package foundation_core --lib
cargo test --package foundation_core --tests
```

---

## References

- RFC 7230 Section 4.1: https://datatracker.ietf.org/doc/html/rfc7230#section-4.1
- GCP Discovery API: https://cloud.google.com/discovery
- JSON specification on control characters: https://www.json.org/json-en.html
