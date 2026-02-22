---
spec_name: "02-build-http-client"
spec_number: 02
feature_name: "public-api"
feature_number: 1
description: User-facing API for HTTP client, including redirect-capable connection loop, ergonomic request handling, and integration with connection pooling.
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/public-api"
this_file: "specifications/02-build-http-client/features/public-api/feature.md"
status: in-progress
priority: high
depends_on:
  - foundation
  - connection
  - request-response
  - task-iterator
estimated_effort: medium
created: 2026-01-18
last_updated: 2026-02-21
author: Main Agent
machine_optimized: true
machine_prompt_file: ./machine_prompt.md
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: true
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0
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
      - ./templates/
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

# Public API Feature

---

## Concrete Patterns & Processes

### State Machine Pattern
- The redirect-capable connection loop is implemented as a state machine in `client/tasks/request_redirect.rs`.
- States: `Init`, `Trying`, `WriteBody`, `Done`.
- Each state transition is explicit and logged with `tracing` for observability.
- The state machine is flattened for clarity and maintainability.

### Error Handling
- All errors are surfaced via `HttpRequestRedirectResponse` (variants: `Done`, `Error`, `FlushFailed`).
- Error mapping is robust: connection, write, flush, and redirect resolution errors are handled and surfaced.
- Errors are mapped to `HttpClientError` in the public API (`client/api.rs`).

### Header Stripping for Redirects
- Sensitive headers (e.g., `Authorization`) are stripped when the host changes during a redirect.
- Logic is implemented in `client/redirects.rs` via `strip_sensitive_headers_for_redirect`.
- Ensures security and compliance with redirect policies.

### POST→GET Semantics
- When a redirect occurs, follow-up requests default to `GET` with no body.
- Content headers are removed, and sensitive headers are stripped if the host changes.
- Implemented in `client/redirects.rs` and used in redirect task logic.

### Integration with Connection Pool
- All redirects use `HttpConnectionPool` for connection reuse.
- Pool integration ensures efficient connection management and resource utilization.
- Implemented in `client/pool.rs` and integrated throughout the redirect task and public API.

### Test-Driven Development (TDD)
- All new functionality is developed using TDD.
- Tests are written first for state transitions, error handling, header stripping, POST→GET, and connection pooling.
- Unit tests: `tests/backends/foundation_core/units/simple_http/http_redirect_task.rs`
- Integration tests: `tests/backends/foundation_core/integrations/simple_http/http_redirect_flow.rs`
- WHY/WHAT comments are included in all tests for clarity and traceability.

### Verification & Acceptance
- Verification agent checks for zero incomplete implementations (NO TODO, FIXME, unimplemented!(), todo!(), stub methods).
- All verification commands must pass: `cargo fmt`, `cargo clippy`, tests, build, security, coverage.
- User approval is required before marking the feature complete.

### Reference Code Files
- State machine: `backends/foundation_core/src/wire/simple_http/client/tasks/request_redirect.rs`
- Error handling: `backends/foundation_core/src/wire/simple_http/client/api.rs`
- Header stripping & POST→GET: `backends/foundation_core/src/wire/simple_http/client/redirects.rs`
- Connection pool: `backends/foundation_core/src/wire/simple_http/client/pool.rs`
- Unit tests: `tests/backends/foundation_core/units/simple_http/http_redirect_task.rs`
- Integration tests: `tests/backends/foundation_core/integrations/simple_http/http_redirect_flow.rs`

---

These patterns and processes ensure the feature is robust, maintainable, and compliant with project standards and rules.

## 📍 Location Reference

- Parent spec: `specifications/02-build-http-client/requirements.md`
- This feature: `specifications/02-build-http-client/features/public-api/`
- This file: `specifications/02-build-http-client/features/public-api/feature.md`
- Machine prompt: `specifications/02-build-http-client/features/public-api/machine_prompt.md`
- Templates: `specifications/02-build-http-client/features/public-api/templates/`
- Progress: `specifications/02-build-http-client/features/public-api/PROGRESS.md`
- Learnings: `specifications/02-build-http-client/LEARNINGS.md`
- Agent rules: `.agents/rules/`
- Stack files: `.agents/stacks/`

## 🔍 Retrieval-Led Reasoning (MANDATORY)

**Before implementation, agents MUST:**
1. Search the codebase for similar features and patterns (use Grep/Glob).
2. Read existing code in related modules:
   - [ClientRequest API](backends/foundation_core/src/wire/simple_http/client/api.rs)
   - [SimpleHttpClient](backends/foundation_core/src/wire/simple_http/client/client.rs)
   - [Redirect Task](backends/foundation_core/src/wire/simple_http/client/tasks/request_redirect.rs)
   - [Redirects Helper](backends/foundation_core/src/wire/simple_http/client/redirects.rs)
   - [SendRequest Task](backends/foundation_core/src/wire/simple_http/client/tasks/send_request.rs)
3. Check stack files for language conventions: `.agents/stacks/rust.md`
4. Read parent specification for context: `../requirements.md`
5. Read module documentation for affected modules.
6. Check dependencies by reading other feature files referenced in `depends_on`.
7. Follow discovered patterns consistently.

**FORBIDDEN:**
- Assuming patterns based on typical practices without checking this codebase.
- Implementing without searching for similar features first.
- Applying generic solutions without verifying project conventions.
- Guessing at naming conventions, file structures, or patterns.
- Using pretraining knowledge without validating against actual project code.

**Retrieval Checklist:**
- [ ] What similar features exist in this project? (use Grep)
- [ ] What patterns do they follow? (read implementations)
- [ ] What naming conventions are used? (observe code)
- [ ] How are errors handled? (check error patterns)
- [ ] What testing patterns exist? (read test files)
- [ ] Are there helper functions to reuse? (search thoroughly)

## 🚀 Token and Context Optimization

- Main Agent generates `machine_prompt.md` from this file.
- Sub-agents read `machine_prompt.md` (NOT verbose feature.md).
- 58% token savings.
- Regenerate `machine_prompt.md` when feature.md updates.
- Generate `COMPACT_CONTEXT.md` before starting any task.
- Embed machine_prompt.md content for current task in COMPACT_CONTEXT.md.
- Regenerate COMPACT_CONTEXT.md after updating PROGRESS.md.
- Clear and reload context from COMPACT_CONTEXT.md only (97% reduction).

## Overview

This feature implements the user-facing API for the HTTP client, including:
- Clean ergonomic request handling (progressive reading, one-shot, streaming).
- Redirect-capable connection loop.
- Integration with connection pooling.
- Hides internal TaskIterator complexity from users.

## Dependencies

- foundation: DnsResolver, errors
- connection: HttpClientConnection, ParsedUrl
- request-response: ClientRequestBuilder, ResponseIntro
- task-iterator: execute_task(), HttpRequestTask

## Requirements

### Functional Requirements

1. Provide ergonomic API for HTTP requests (intro, body, send, parts).
2. Support redirect-capable connection loop (handles 3xx, Location header, redirect limits).
3. Integrate with connection pooling for connection reuse.
4. Surface all relevant errors (connection, write, flush, redirect resolution).
5. Strip sensitive headers (e.g., Authorization) when host changes during redirect.
6. Support POST→GET semantics for redirects.
7. Ensure robust tracing/logging for state transitions and errors.

### Technical Requirements

- State machine pattern: `Init`, `Trying`, `WriteBody`, `Done` states in `request_redirect.rs`.
- Error handling: All errors surfaced via `HttpRequestRedirectResponse` (`FlushFailed`, `TooManyRedirects`, `InvalidLocation`, etc.).
- Header stripping: `strip_sensitive_headers_for_redirect` in `redirects.rs`.
- POST→GET semantics: Follow-up requests default to GET, content headers removed, sensitive headers stripped if host changes.
- Integration with pool: All redirects use `HttpConnectionPool` for connection reuse.

## Implementation Details

### Key Structures

- `ClientRequest` (API): `backends/foundation_core/src/wire/simple_http/client/api.rs`
- `SimpleHttpClient`: `backends/foundation_core/src/wire/simple_http/client/client.rs`
- `GetHttpRequestRedirectTask`: `backends/foundation_core/src/wire/simple_http/client/tasks/request_redirect.rs`
- `HttpConnectionPool`: `backends/foundation_core/src/wire/simple_http/client/pool.rs`

### Key Functions

| Function | Purpose | Location |
|----------|---------|----------|
| `ClientRequest::introduction()` | Get intro and headers | `client/api.rs` |
| `ClientRequest::body()` | Get response body | `client/api.rs` |
| `ClientRequest::send()` | One-shot request | `client/api.rs` |
| `ClientRequest::parts()` | Streaming parts | `client/api.rs` |
| `GetHttpRequestRedirectTask::next()` | State machine for redirects | `client/tasks/request_redirect.rs` |
| `strip_sensitive_headers_for_redirect()` | Header stripping | `client/redirects.rs` |

### Templates

See `templates/` directory for:
- `example-struct.rs` - Base structure template
- `example-impl.rs` - Implementation template

---

### Complete Code Example: Public API Usage

Below is a concrete example showing how the public API is used, based on actual code patterns:

```rust
use foundation_core::wire::simple_http::client::{
    SimpleHttpClient, ClientConfig, HttpConnectionPool, SystemDnsResolver,
};
use foundation_core::wire::simple_http::{SendSafeBody, IncomingResponseParts};

fn main() {
    // Create a client with default config and system DNS resolver
    let client = SimpleHttpClient::<SystemDnsResolver>::new();

    // Option 1: Progressive reading (intro, headers, then body)
    let mut request = client.get("http://example.com").expect("Failed to build request");
    let (intro, headers) = request.introduction().expect("Failed to get intro and headers");
    let body = request.body().expect("Failed to get body");
    match body {
        SendSafeBody::Text(text) => println!("Body: {}", text),
        SendSafeBody::Bytes(bytes) => println!("Body (bytes): {:?}", bytes),
        _ => println!("No body"),
    }

    // Option 2: One-shot execution
    let response = client.get("http://example.com").expect("Failed to build request").send().expect("Failed to send request");
    println!("Status: {:?}", response.get_status());
    match response.get_body_mut() {
        SendSafeBody::Text(text) => println!("Response body: {}", text),
        SendSafeBody::Bytes(bytes) => println!("Response body (bytes): {:?}", bytes),
        _ => println!("No body"),
    }

    // Option 3: Streaming parts (iterator)
    let request = client.get("http://example.com").expect("Failed to build request");
    let (part_iter, _) = request.parts().expect("Failed to get parts iterator");
    for part_result in part_iter {
        let part = part_result.expect("Failed to get part");
        match part {
            IncomingResponseParts::Intro(status, proto, reason) => {
                println!("Intro: {:?} {:?} {:?}", status, proto, reason);
            }
            IncomingResponseParts::Headers(headers) => {
                println!("Headers: {:?}", headers);
            }
            IncomingResponseParts::SizedBody(body) => {
                match body {
                    SendSafeBody::Text(text) => println!("Body: {}", text),
                    SendSafeBody::Bytes(bytes) => println!("Body (bytes): {:?}", bytes),
                    _ => println!("No body"),
                }
            }
            _ => {}
        }
    }

    // Option 4: Custom config and connection pool
    let config = ClientConfig {
        max_redirects: 3,
        ..Default::default()
    };
    let pool = HttpConnectionPool::default();
    let client = SimpleHttpClient::with_resolver(SystemDnsResolver)
        .config(config)
        .enable_pool(10);

    let request = client.get("http://example.com").expect("Failed to build request");
    let response = request.send().expect("Failed to send request");
    println!("Status: {:?}", response.get_status());
}
```

This example demonstrates:
- Progressive reading (intro, headers, body)
- One-shot execution (`send`)
- Streaming parts with iterator
- Custom configuration and connection pooling
- Handling of response bodies and headers

---

## Tasks

> **Task Tracking:** Mark tasks as `[x]` after completing AND verifying each task. Update frontmatter counts immediately. Commit after task completion + verification pass (Rule 04).

### Implementation Tasks
- [ ] Implement core API structures and methods.
- [ ] Integrate redirect-capable connection loop.
- [ ] Add header stripping logic for redirects.
- [ ] Integrate connection pooling.
- [ ] Implement POST→GET semantics for redirects.

### Testing Tasks
- [ ] Write unit tests for state machine transitions and error handling.
- [ ] Write integration tests for redirect chains, edge cases, header stripping, POST→GET, flush failures.
- [ ] Run verification commands.
- [ ] Execute HTTP client tests with:
      ```
      cargo test --package ewe_platform_tests --features std -- http_client_body_reading
      ```

### Documentation Tasks
- [ ] Document public APIs.
- [ ] Add usage examples.
- [ ] Document architectural decisions in LEARNINGS.md.

## Success Criteria

- [ ] Ergonomic API for HTTP requests (intro, body, send, parts).
- [ ] Redirect-capable connection loop integrated and tested.
- [ ] All relevant errors surfaced and mapped.
- [ ] Sensitive headers stripped on host change.
- [ ] POST→GET semantics for redirects implemented.
- [ ] All unit and integration tests pass.
- [ ] Code passes `cargo fmt` and `cargo clippy`.
- [ ] Documentation updated.
- [ ] All verification requirements met.

## Verification Requirements for Completion

- [ ] Zero incomplete implementations (NO TODO, FIXME, unimplemented!(), todo!(), stub methods).
- [ ] All verification checks pass (format, lint, type, tests, build, security, coverage).
- [ ] All tasks and success criteria checked off.
- [ ] Integration with dependent features verified.
- [ ] Documentation complete.
- [ ] Explicit user approval to mark feature complete.

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core --features multi
cargo build --package foundation_core
```

## Agent Instructions

### Main Agent

- Load all rules specified in files_required.main_agent.rules.
- Work autonomously, make informed decisions based on loaded context and rules.
- Review current implementation status before starting.
- Do NOT ask unnecessary questions if feature is clearly defined.
- Only ask when genuinely ambiguous or blocking.

### Implementation Agent

- Load role-specific rules from files_required.implementation_agent.rules.
- Read required context: parent requirements.md, this feature.md, templates, stack files.
- Verify dependent features are complete.
- Follow TDD: Write tests FIRST, verify they fail, then implement.
- Self-review before reporting completion.
- Document learnings in LEARNINGS.md.
- Do NOT commit code directly; report completion to Main Agent.

### Verification Agent

- Load role-specific rules from files_required.verification_agent.rules.
- Read required context: parent requirements.md, this feature.md, templates, stack files.
- Verify dependent features are complete.
- Run all verification commands.
- Ensure zero incomplete implementations.
- Present verification report to user for approval.

---

*Created: 2026-01-18*
*Last Updated: 2026-02-21*
