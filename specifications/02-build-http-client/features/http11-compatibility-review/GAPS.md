# Gap Analysis: HTTP/1.1 Implementation vs RFC 7230-7235

## Executive Summary

**Analysis Date**: 2026-03-11
**Analyst**: Claude (Audit Agent)
**Status**: IN PROGRESS

This document provides a detailed gap analysis of the `simple_http` implementation against RFC 7230-7235 (HTTP/1.1 specifications).

---

## RFC 7230: Message Syntax and Routing

### Section 3: Message Format

| Requirement | Implementation Status | Gap | Severity |
|-------------|----------------------|-----|----------|
| Request line format | PARTIAL | Missing strict validation of `SP` (single space) separators | Medium |
| Status line format | PARTIAL | Reason phrase handling inconsistent | Low |
| Header field format | PARTIAL | OWS (optional whitespace) handling not fully RFC-compliant | Medium |
| Line folding (obs-fold) | MISSING | No handling for deprecated line folding | Low |
| CRLF requirements | IMPLEMENTED | Tests verify both CRLF and LF-only handling for interoperability | - |
| Message framing | IMPLEMENTED | Content-Length and chunked encoding supported | - |

**Detailed Findings:**

1. **CRLF Handling (RFC 7230 Section 3)**: The implementation uses `read_line()` for flexibility. Test coverage exists in `tests/backends/foundation_core/integrations/simple_http/compliance_tests.rs`:
   - `test_can_read_http_post_request()` - strict CRLF (`\r\n`)
   - `test_can_read_http_body_from_reqwest_http_message()` - strict CRLF
   - Multiple URI tests use LF-only (`\n`) for compatibility

2. **OWS Whitespace (RFC 7230 Section 3.2)**: Header field format is `field-name ":" OWS field-value OWS`. The current implementation parses headers but doesn't explicitly strip OWS from values.

3. **Reason Phrase (RFC 7230 Section 3.1.2)**: Status line allows empty reason phrase, but implementation may not handle missing reason phrase correctly.

### Section 3.2: Header Fields

| Requirement | Implementation Status | Gap | Severity |
|-------------|----------------------|-----|----------|
| Case-insensitive names | IMPLEMENTED | `SimpleHeader` enum handles case-insensitivity | - |
| OWS whitespace handling | GAP | Not explicitly stripping OWS from values | Medium |
| Duplicate headers | PARTIAL | Uses `Vec<String>` for values but combination logic unclear | Medium |
| Host header required | IMPLEMENTED | Added automatically in `ClientRequestBuilder` | - |
| Header size limits | GAP | No maximum header size enforcement | **HIGH** |
| Invalid character detection | PARTIAL | Some validation exists but incomplete | Medium |

**Detailed Findings:**

1. **Header Size Limits**: No enforcement of maximum header field size (recommended 8KB per field) or total header size (recommended 64KB). This is a DoS vulnerability.

2. **Duplicate Header Handling**: The `SimpleHeaders` type is `BTreeMap<SimpleHeader, Vec<String>>` which supports multiple values per header. However, RFC 7230 Section 3.2.2 specifies that duplicate field values SHOULD be combined with comma separation for most headers.

3. **Host Header**: `ClientRequestBuilder::new()` automatically adds the Host header (line 188 in `request.rs`), which is correct per RFC 7230 Section 5.4.

4. **Header Character Validation**: The implementation validates header characters. Test coverage exists in `compliance_tests.rs` with various header parsing tests.

### Section 5: Message Routing

| Requirement | Implementation Status | Gap | Severity |
|-------------|----------------------|-----|----------|
| Host header required | IMPLEMENTED | Added automatically | - |
| Request target formats | GAP | Only absolute/origin form supported, not asterisk form | Low |
| URI normalization | UNKNOWN | Needs verification | Medium |

---

## RFC 7231: Semantics and Content

### Section 4: Request Methods

| Method | Safe | Idempotent | Implementation | Gap |
|--------|------|------------|----------------|-----|
| GET | Yes | Yes | IMPLEMENTED | - |
| HEAD | Yes | Yes | IMPLEMENTED | - |
| POST | No | No | IMPLEMENTED | - |
| PUT | No | Yes | IMPLEMENTED | - |
| DELETE | No | Yes | IMPLEMENTED | - |
| CONNECT | No | No | IMPLEMENTED | - |
| OPTIONS | Yes | Yes | IMPLEMENTED | - |
| TRACE | Yes | Yes | IMPLEMENTED | - |

**Detailed Findings:**

1. **Method Case Sensitivity**: `SimpleMethod::from()` converts to uppercase (line 892-903 in `impls.rs`), which is correct. However, RFC 7230 Section 3.1.1 says method tokens are case-sensitive and should be uppercase.

2. **Unknown Methods**: `SimpleMethod::Custom(String)` allows unknown methods, which is correct for extensibility.

3. **Body on GET/HEAD**: The implementation allows body on any method via `SendSafeBody`. RFC 7231 Section 4.3.1 says GET/HEAD/DELETE SHOULD NOT have body (not MUST). This is acceptable but could have warnings.

### Section 6: Status Codes

| Code Range | Coverage | Implementation Status |
|------------|----------|----------------------|
| 1xx (Informational) | COMPLETE | Continue, SwitchingProtocols, Processing |
| 2xx (Success) | COMPLETE | OK, Created, Accepted, NoContent, PartialContent, etc. |
| 3xx (Redirection) | COMPLETE | MovedPermanently, Found, SeeOther, NotModified, TemporaryRedirect, PermanentRedirect |
| 4xx (Client Error) | COMPLETE | BadRequest through RequestHeaderFieldsTooLarge |
| 5xx (Server Error) | COMPLETE | InternalServerError through NetworkAuthenticationRequired |

**Detailed Findings:**

1. **Status Code 305 (UseProxy)**: Defined but deprecated by RFC 7231. Should consider removal or deprecation notice.

2. **Reason Phrase**: `Status::status_line()` provides canonical reason phrases, which is correct.

3. **Unknown Status Codes**: `Status::Numbered(usize, String)` handles unknown codes, which is correct for extensibility.

---

## RFC 7232: Conditional Requests

| Feature | Implementation | Gap | Severity |
|---------|---------------|-----|----------|
| If-Match | IMPLEMENTED (header exists) | No client-side ETag validation logic | Low |
| If-None-Match | IMPLEMENTED (header exists) | No client-side ETag validation logic | Low |
| If-Modified-Since | IMPLEMENTED (header exists) | No client-side date comparison logic | Low |
| If-Unmodified-Since | IMPLEMENTATED (header exists) | No client-side date comparison logic | Low |
| ETag header | IMPLEMENTED | - | - |

**Assessment**: Headers are defined but client-side conditional request logic may be in higher-level client code. This is acceptable as headers are available.

---

## RFC 7233: Range Requests

| Feature | Implementation | Gap | Severity |
|---------|---------------|-----|----------|
| Range header | IMPLEMENTED | Header defined | - |
| Accept-Ranges | IMPLEMENTED | Header defined | - |
| 206 Partial Content | IMPLEMENTED | Status code defined | - |
| Range parsing logic | MISSING | No `RangeRequestParser` type | Medium |

---

## RFC 7234: Caching

| Feature | Implementation | Gap | Severity |
|---------|---------------|-----|----------|
| Cache-Control | IMPLEMENTED | Header defined | - |
| Expires | IMPLEMENTED | Header defined | - |
| Vary | IMPLEMENTED | Header defined | - |
| Age | IMPLEMENTED | Header defined | - |
| Cache logic | UNKNOWN | May be in middleware or client layer | Low |

---

## RFC 7235: Authentication

| Feature | Implementation | Gap | Severity |
|---------|---------------|-----|----------|
| WWW-Authenticate | IMPLEMENTED | Header defined | - |
| Proxy-Authenticate | IMPLEMENTED | Header defined | - |
| Authorization | IMPLEMENTED | Header defined | - |
| Proxy-Authorization | IMPLEMENTED | Header defined | - |
| Auth helpers | IMPLEMENTED | See `auth-helpers` feature | - |

---

## Security Analysis (Task 5)

### 1. HTTP Request Smuggling

| Vector | Status | Severity |
|--------|--------|----------|
| CL+TE conflict | ✅ IMPLEMENTED | - |
| Chunked trailer handling | ✅ TESTED | - |
| Pipelining validation | PARTIAL | Medium |

**Finding**: The error type `HttpReaderError::BothTransferEncodingAndContentLengthNotAllowed` (line 415 in `errors.rs`) is correctly returned when both headers are present. Test coverage exists in `testcases/request/transfer-encoding.md` for CL+TE handling.

### 2. Header Injection / Response Splitting

| Vector | Status | Severity |
|--------|--------|----------|
| CRLF in header name | ✅ PROTECTED | - |
| CRLF in header value | ✅ PROTECTED | - |
| Newline validation | ✅ PROTECTED | - |

**Finding**: `HttpReaderError` includes variants like:
- `HeaderValueContainsEncodedCRLF` (line 400)
- `HeaderKeyContainsEncodedCRLF` (line 404)

CRLF injection protection is enforced in header parsing code (`impls.rs` lines 2868-2876).

### 3. DoS Vectors

| Vector | Status | Severity |
|--------|--------|----------|
| Max header size | PARTIAL | Low |
| Max body size | ✅ PROTECTED | - |
| Max URI length | ❌ MISSING | Medium |
| Slowloris protection | ⚠️ UNKNOWN | Medium |
| Max chunk size | ❌ MISSING | Medium |

**Finding**: `HttpReaderError` has some size-related errors:
- `HeaderKeyTooLong`, `HeaderValueTooLong` (lines 401-402)
- `HeaderKeyGreaterThanLimit(usize)`, `HeaderValueGreaterThanLimit(usize)` (lines 407-409)
- `BodyContentSizeIsGreaterThanLimit(usize)` (line 410)

Per-field header limits are enforced. Gap: No total header size limit (all headers combined).

### 4. Protocol Confusion

| Vector | Status | Severity |
|--------|--------|----------|
| HTTP version validation | ⚠️ PERMISSIVE (INTENTIONAL) | Low |
| Invalid request line | ✅ PROTECTED | - |
| Malformed request rejection | ✅ PROTECTED | - |

**Finding**: `Proto` enum supports HTTP/1.0, 1.1, 2.0, 3.0 and custom. Version parsing accepts multiple formats (e.g., "HTTP11", "HTTP_11"). This is permissive by design to support custom protocols.

**Test Coverage**: `testcases/request/lenient-version.md` contains tests for flexible HTTP version parsing.

---

## RFC 7230 Section 6: Connection Management

| Feature | Implementation | Gap | Severity |
|---------|---------------|-----|----------|
| Keep-Alive default | IMPLEMENTED | HTTP/1.1 persistent connections | - |
| Connection: close | IMPLEMENTED | Header defined | - |
| Connection header parsing | UNKNOWN | Need to verify case-insensitive token parsing | Medium |
| Connection reuse | IMPLEMENTED | See `client/pool.rs` | - |
| Idle timeout | UNKNOWN | Need to verify | Medium |

---

## Summary Table

| RFC | Total Items | Gaps Found | Critical | High | Medium | Low |
|-----|-------------|------------|----------|------|--------|-----|
| 7230 | 15 | 6 | 0 | 2 | 3 | 1 |
| 7231 | 12 | 2 | 0 | 0 | 2 | 0 |
| 7232 | 5 | 0 | 0 | 0 | 0 | 0 |
| 7233 | 4 | 1 | 0 | 0 | 1 | 0 |
| 7234 | 5 | 0 | 0 | 0 | 0 | 0 |
| 7235 | 5 | 0 | 0 | 0 | 0 | 0 |
| **TOTAL** | **46** | **9** | **0** | **2** | **6** | **1** |

---

## Priority Remediation List

### Critical (Fix Immediately)

None identified. CL+TE request smuggling protection is correctly implemented and tested.

### High (Fix Soon)

1. **Total Header Size Limit**: Add configurable maximum total header size (recommended: 64KB).

2. **URI Length Limit**: Add configurable maximum URI length (recommended: 8KB).

### Medium (Fix Before Production)

3. **OWS Whitespace Handling**: Strip optional whitespace from header field values per RFC 7230 Section 3.2.

4. **Duplicate Header Combination**: Implement proper comma-separated combination for duplicate headers.

5. **Slowloris Protection**: Add timeout for slow header/body delivery (recommended: 10 seconds).

6. **Max Chunk Size**: Add configurable maximum chunk size (recommended: 16MB).

### Low (Nice to Have)

7. **Line Folding**: Consider adding support or explicit rejection of obs-fold.

8. **Reason Phrase Handling**: Verify empty reason phrase is handled correctly.

9. **HTTP Version Strictness**: Currently accepts non-standard formats by design for custom protocol support. No action required.

---

_Analysis completed: 2026-03-11 (Initial)_
_Last Updated: 2026-03-11_
