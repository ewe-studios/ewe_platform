---
feature: reader-eof-handling
description: Fix HttpRequestReader and HttpResponseReader infinite loop on EOF (read_line returns Ok(0))
status: pending
priority: critical
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
dependencies: []
---

# Feature: Reader EOF Handling

## Problem

`HttpRequestReader` and `HttpResponseReader` enter an infinite tight CPU loop when
the underlying TCP connection reaches EOF. `read_line()` returns `Ok(0)` (zero bytes
read — peer closed connection), but both readers treat this identically to a blank
line between pipelined requests — they return `SKIP` and stay in `HttpReadState::Intro`.

### Observed Behavior

The test server's connection handler threads spin at 100% CPU after the client closes
its connection. The log shows thousands of lines per second:

```
TRACE handle_connection{running=true}:next: foundation_core::wire::simple_http::impls:
  Pulling the next request data from connection: Ok(0)
```

The test (`test_pool_put_head_delete_head_sequence`) takes ~25-105s instead of <1s
because the handler threads burn CPU until the process exits.

### Root Cause

**`HttpRequestReader::next()`** at `impls.rs:3295-3322`:

```rust
HttpReadState::Intro => {
    let mut line = String::new();
    let line_read_result = self.reader
        .do_once_mut(|binding| binding.read_line(&mut line))
        .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

    if let Err(e) = line_read_result { ... }  // only checks errors

    let intro_parts: Vec<&str> = line.split_whitespace()...collect();

    if intro_parts.is_empty() {
        self.state = HttpReadState::Intro;       // ← stays in Intro
        return Some(Ok(IncomingRequestParts::SKIP)); // ← never returns None
    }
```

When `BufRead::read_line()` returns `Ok(0)`, `line` stays empty. The code falls through
to the `intro_parts.is_empty()` check, returns `SKIP`, and stays in `Intro` state.

Callers like the test server use `.filter(|item| matches SKIP => false).collect()` which
filters out `SKIP` and calls `next()` again — creating an unbreakable loop since the
iterator never returns `None`.

**`HttpResponseReader::next()`** at `impls.rs:3737-3764` has the identical pattern.

### Key Distinction

- `read_line()` returns `Ok(n)` where n > 0, line is `"\r\n"` → blank line between
  pipelined requests. `SKIP` is correct — the reader should continue.
- `read_line()` returns `Ok(0)`, line stays empty → **EOF**. The peer closed the
  connection. The iterator must return `None` to signal termination.

The header parser (`parse_headers` at line 2910) already handles this correctly — it
checks `line.is_empty()` and breaks on EOF. The intro readers don't.

## Approach

Check the return value of `read_line()`. If it's `Ok(0)`, transition to
`HttpReadState::Finished` and return `None`:

```rust
HttpReadState::Intro => {
    let mut line = String::new();
    let line_read_result = self.reader
        .do_once_mut(|binding| binding.read_line(&mut line))
        .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

    match line_read_result {
        Err(e) => {
            self.state = HttpReadState::Finished;
            return Some(Err(e));
        }
        Ok(0) => {
            // EOF — peer closed connection
            self.state = HttpReadState::Finished;
            return None;
        }
        Ok(_) => {}
    }

    // ... rest of intro parsing unchanged
```

This preserves the existing `SKIP` behavior for genuine blank lines (which have
`Ok(n)` where n > 0 for the `\r\n` bytes) while correctly terminating on EOF.

## Tasks

- [ ] TASK-EOF-01: In `HttpRequestReader::next()` (`impls.rs:3308-3313`): replace `if let Err(e) = line_read_result` with match that handles `Ok(0)` → transition to `Finished`, return `None`
- [ ] TASK-EOF-02: In `HttpResponseReader::next()` (`impls.rs:3750-3754`): same fix — match `Ok(0)` → `Finished` + `None`
- [ ] TASK-EOF-03: Add test: `HttpRequestReader` with a stream that returns EOF immediately — verify iterator returns `None` (not infinite `SKIP`)
- [ ] TASK-EOF-04: Add test: `HttpRequestReader` with stream that sends one complete request then EOF — verify first request is read, then iterator returns `None` on second `next_request()` call
- [ ] TASK-EOF-05: Add test: test server connection handler exits promptly when client closes connection — verify no tight `Ok(0)` loop in logs and handler thread terminates within 1s
