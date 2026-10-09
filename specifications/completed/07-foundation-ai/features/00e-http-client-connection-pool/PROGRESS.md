# Progress: HTTP Client Connection Pool Safety for No-Body Responses

**Status:** ✅ Fixed — Tests passing  
**Last updated:** 2026-04-12

---

## Current State

### ✅ Completed
- Root cause analysis completed
- Connection flow traced through `api.rs`, `connection.rs`, `pool.rs`
- Fix options documented in [feature.md](feature.md)
- Test failure pattern identified (204 → HEAD sequence)
- **Fix implemented**: Added `drain_stream_before_pooling()` in `api.rs:126-148`
- **R2 tests**: 5/5 pass (including previously failing `test_r2_blobstore_delete`)
- **D1 tests**: 7/7 pass (no regression)

### ⏳ Pending
- Add unit test for no-body response pooling
- Document in `CLAUDE.md` / `AGENTS.md` for future reference

---

## Implementation

### Fix Applied

Modified `backends/foundation_core/src/wire/simple_http/client/api.rs`:

1. **Added `drain_stream_before_pooling()` helper** (line 126-148):
   - Reads remaining buffered data into 1KB buffer
   - Loops until EOF or error
   - Ensures HTTP response reader state machine is fully consumed

2. **Modified `FinalizedResponse::drop()`** (line 151-163):
   - Calls `drain_stream_before_pooling()` before `pool.return_to_pool()`
   - Ensures connections returned to pool are in clean state

### Code Changes

```rust
fn drain_stream_before_pooling(
    stream: &mut HttpClientConnection,
) {
    let mut drain_buf = [0u8; 1024];
    loop {
        match stream.read(&mut drain_buf) {
            Ok(0) => break, // EOF - stream fully drained
            Ok(_) => continue, // More data read, keep draining
            Err(_) => break, // Read error - stop draining to avoid blocking
        }
    }
}

impl<T, R: DnsResolver + 'static> Drop for FinalizedResponse<T, R> {
    fn drop(&mut self) {
        if let (Some(pool), Some(mut stream)) = (self.1.take(), self.2.take()) {
            drain_stream_before_pooling(&mut stream);
            pool.return_to_pool(stream);
        }
        let _ = self.0.take();
    }
}
```

---

## Test Results

### Before Fix
| Test | Result |
|------|--------|
| D1: 7 tests | ✅ 7/7 pass |
| R2: put_get | ✅ pass |
| R2: exists | ✅ pass |
| R2: delete | ❌ ReadFailed |
| R2: list | ✅ pass |
| R2: delete_then_check | ✅ pass |

### After Fix
| Test | Result |
|------|--------|
| D1: 7 tests | ✅ 7/7 pass |
| R2: 5 tests | ✅ 5/5 pass |

---

## Related Files

- `backends/foundation_core/src/wire/simple_http/client/api.rs` — Drop impl, drain helper
- `backends/foundation_core/src/wire/simple_http/client/connection.rs` — return_to_pool()
- `backends/foundation_core/src/wire/simple_http/client/pool.rs` — checkin()
- `backends/foundation_db/tests/r2_blobstore_tests.rs` — test_r2_blobstore_delete
