---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/http11-compatibility-review"
this_file: "specifications/02-build-http-client/features/http11-compatibility-review/feature.md"

status: complete
priority: high
created: 2026-03-11
updated: 2026-03-11

depends_on:
  - request-response
  - public-api

tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100
---

# HTTP/1.1 Compatibility Review (RFC 7230-7235)

## Overview

Comprehensive review of the `simple_http` implementation for HTTP/1.1 specification compliance, edge case handling, attack vector resilience, and correctness. This feature audits the existing implementation against RFC 7230-7235 (HTTP/1.1 core specifications) and identifies gaps, vulnerabilities, and areas for improvement.

**Reference Specifications:**
- RFC 7230: Message Syntax and Routing
- RFC 7231: Semantics and Content
- RFC 7232: Conditional Requests
- RFC 7233: Range Requests
- RFC 7234: Caching
- RFC 7235: Authentication

## Goals

1. Audit HTTP message parsing and rendering for RFC compliance
2. Identify edge cases not currently handled
3. Analyze attack vector resilience (request smuggling, header injection, etc.)
4. Verify header handling (case-insensitivity, duplicates, forbidden headers)
5. Review connection management (keep-alive, connection close)
6. Validate error handling and recovery

## Scope

This review covers the following modules:
- `backends/foundation_core/src/wire/simple_http/`
  - `impls.rs` - Core types (SimpleBody, SimpleHeader, Status, etc.)
  - `errors.rs` - Error types
  - `client/` - Client implementation
  - `url/` - URL parsing

## Tasks

### Task 1: HTTP Message Format Compliance (RFC 7230 Section 3)

**Objective**: Verify HTTP message start-line and header formatting.

**Checklist:**
- [ ] Request line format: `METHOD SP request-target SP HTTP-version CRLF`
- [ ] Status line format: `HTTP-version SP status-code SP reason-phrase CRLF`
- [ ] Header field format: `field-name ":" OWS field-value OWS`
- [ ] Line folding (obs-fold) handling - deprecated in HTTP/1.1
- [ ] Message framing (Content-Length vs Transfer-Encoding: chunked)
- [ ] CRLF requirements (not just LF)

**Files to Review:**
- `impls.rs` - `RenderHttp` trait, `SimpleIncomingRequestBuilder`, `SimpleOutgoingResponseBuilder`
- `impls.rs` - `HttpResponseReader`, `IncomingResponseParts`
- `impls.rs` - Chunked encoding (`ChunkedData`, `ChunkedVecIterator`)

**Deliverable**: Gap analysis document with RFC citations

---

### Task 2: Header Field Handling (RFC 7230 Section 3.2)

**Objective**: Verify header parsing and validation.

**Checklist:**
- [ ] Case-insensitive header name comparison
- [ ] Header value whitespace handling (OWS - optional whitespace)
- [ ] Duplicate header fields (combine vs reject)
- [ ] Forbidden header names in requests
- [ ] Header size limits (DoS protection)
- [ ] Invalid header character detection (control chars, invalid bytes)
- [ ] Host header validation (RFC 7230 Section 5.4)

**Files to Review:**
- `impls.rs` - `SimpleHeaders`, `SimpleHeader` enum
- `impls.rs` - Header parsing in `HttpResponseReader`

**Deliverable**: Header handling compliance report

---

### Task 3: Request Method Semantics (RFC 7231 Section 4)

**Objective**: Verify HTTP method handling.

**Checklist:**
- [ ] Safe methods (GET, HEAD, OPTIONS, TRACE) - no side effects
- [ ] Idempotent methods (GET, HEAD, PUT, DELETE, OPTIONS, TRACE)
- [ ] Method case-sensitivity (uppercase canonical)
- [ ] Unknown method handling
- [ ] Method not allowed responses (405)
- [ ] Content-Length requirements for methods (no body for GET/HEAD/DELETE)

**Files to Review:**
- `impls.rs` - `SimpleMethod` enum
- `impls.rs` - `SimpleIncomingRequestBuilder`

**Deliverable**: Method semantics compliance report

---

### Task 4: Status Code Handling (RFC 7231 Section 6)

**Objective**: Verify status code coverage and handling.

**Checklist:**
- [ ] All standard 1xx-5xx status codes defined
- [ ] Reason phrase handling
- [ ] Status code extensibility (unknown codes)
- [ ] Response body requirements per status (e.g., 204 No Content)
- [ ] Redirect status codes (301, 302, 303, 307, 308) - see redirect feature

**Files to Review:**
- `impls.rs` - `Status` enum

**Deliverable**: Status code coverage report

---

### Task 5: HTTP Attack Vector Resilience

**Objective**: Identify and mitigate HTTP-related attack vectors.

**Attack Vectors to Check:**

#### 5.1 HTTP Request Smuggling
- [ ] Content-Length + Transfer-Encoding: chunked handling (reject or prioritize)
- [ ] Chunked encoding validation (trailer handling, size parsing)
- [ ] Pipelining validation

#### 5.2 Header Injection
- [ ] CRLF injection prevention in header names/values
- [ ] Newline character validation

#### 5.3 Buffer Overflow / DoS
- [ ] Maximum header size enforcement
- [ ] Maximum body size enforcement
- [ ] Maximum URI length enforcement
- [ ] Slowloris protection (timeout on slow headers/body)
- [ ] Maximum chunk size validation

#### 5.4 Protocol Confusion
- [ ] HTTP version validation
- [ ] Invalid request line handling
- [ ] Malformed request rejection

**Files to Review:**
- `impls.rs` - All parsing code
- `errors.rs` - Error types for rejection
- `client/*.rs` - Client-side validation

**Deliverable**: Security audit report with mitigations

---

### Task 6: Connection Management (RFC 7230 Section 6)

**Objective**: Verify connection lifecycle handling.

**Checklist:**
- [ ] Keep-Alive default behavior (HTTP/1.1)
- [ ] Connection: close handling
- [ ] Connection header token parsing (case-insensitive)
- [ ] Proxy-Connection handling
- [ ] Connection reuse conditions
- [ ] Idle timeout for pooled connections

**Files to Review:**
- `client/connection.rs` - `HttpClientConnection`
- `client/pool.rs` - `HttpConnectionPool`
- `client/control.rs` - Connection lifecycle

**Deliverable**: Connection management compliance report

---

## Implementation Phases

### Phase 1: Audit (Tasks 1-4)
- Review code against RFC specifications
- Document gaps with specific RFC citations
- Identify missing edge cases

### Phase 2: Security Review (Task 5)
- Analyze attack vectors
- Test malformed input handling
- Propose mitigations

### Phase 3: Connection Review (Task 6)
- Review connection lifecycle
- Verify pool implementation
- Check timeout handling

### Phase 4: Remediation
- Create tasks for fixing identified gaps
- Prioritize security issues
- Update implementation as needed

---

## Verification Criteria

This feature is complete when:

1. **Documentation Complete**
   - All 6 tasks have deliverables
   - Gap analysis includes RFC citations
   - Security audit identifies all known vulnerabilities

2. **Issues Categorized**
   - Critical (security vulnerabilities)
   - High (RFC non-compliance)
   - Medium (missing edge cases)
   - Low (code quality, optional features)

3. **Remediation Plan**
   - Follow-up tasks created for critical/high issues
   - Timeline for fixes established

---

## Reference Materials

### RFC Documents
- [RFC 7230](https://datatracker.ietf.org/doc/html/rfc7230) - Message Syntax and Routing
- [RFC 7231](https://datatracker.ietf.org/doc/html/rfc7231) - Semantics and Content
- [RFC 7232](https://datatracker.ietf.org/doc/html/rfc7232) - Conditional Requests
- [RFC 7233](https://datatracker.ietf.org/doc/html/rfc7233) - Range Requests
- [RFC 7234](https://datatracker.ietf.org/doc/html/rfc7234) - Caching
- [RFC 7235](https://datatracker.ietf.org/doc/html/rfc7235) - Authentication

### HTTP Test Resources
- [httpbin.org](https://httpbin.org) - HTTP request/response testing service
- [RFC Compliance Test Suites](https://github.com/httpwg) - HTTP Working Group tests

### Known Attack Vectors
- [HTTP Request Smuggling (CVE-2023-25690)](https://nvd.nist.gov/vuln/detail/CVE-2023-25690)
- [OWASP HTTP Response Split](https://owasp.org/www-community/attacks/HTTP_Response_Splitting)
- [Slowloris DoS](https://owasp.org/www-community/attacks/Slowloris)

---

## Output Files

1. `specifications/02-build-http-client/features/http11-compatibility-review/REPORT.md`
   - Main audit report with findings

2. `specifications/02-build-http-client/features/http11-compatibility-review/SECURITY.md`
   - Security-specific findings and mitigations

3. `specifications/02-build-http-client/features/http11-compatibility-review/GAPS.md`
   - Detailed gap analysis with RFC citations

---

_Created: 2026-03-11_
_Last Updated: 2026-03-11_
