# GCP Spec Fetch Debugging History

## Problem

The GCP spec fetch command was timing out:
```bash
timeout 60 ./target/debug/ewe_platform gen_provider_specs --provider gcp
# Exit code 124 (timeout)
```

Debug logs showed:
- "Reading chunk size: 0" - HTTP connection established but no data received
- Valtron executor stuck in `State::Pending(None)` loops
- Tasks received HTTP responses but couldn't complete

## Root Cause Analysis

### The Valtron Executor Architecture

The Valtron executor has two modes:
1. **Single-threaded mode** (`feature = "single"`) - tasks run inline
2. **Multi-threaded mode** (`feature = "multi"`) - tasks split between:
   - 5 task executor threads (run `TaskIterator` implementations)
   - 2 background job threads (run blocking operations via `run_background_job()`)

### The Deadlock Pattern

The original GCP fetcher used `SimpleHttpClient` with `execute()`:

```rust
pub fn fetch_gcp_specs(client: &SimpleHttpClient, output_dir: PathBuf) -> Result<impl StreamIterator<...>> {
    // This creates a TaskIterator that gets scheduled on the task executor
    execute(transform(fetch_directory_task(client)))  // Scheduled on task threads
}
```

**The problem:**

1. Task executor threads poll sockets for I/O readiness
2. But the multi-threaded executor has **no epoll/kqueue integration**
3. Tasks polling on socket I/O return `Pending` repeatedly
4. The network stack never signals readiness because there's no proper I/O integration
5. Meanwhile, the main thread blocks waiting for results from `execute()`
6. **Deadlock**: Task executor spins on `Pending`, main thread waits forever

### Why `run_until_stream_has_value()` Doesn't Help

From `backends/foundation_core/src/valtron/executors/drivers.rs:194-212`:

```rust
#[cfg(any(not(feature = "multi"), target_arch = "wasm32"))]
impl<D, P, S: Iterator<Item = Stream<D, P>>> DrivenStreamIterator<S> {
    // In single-threaded mode: actually waits for stream progress
}

#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
impl<D, P, S: Iterator<Item = Stream<D, P>>> DrivenStreamIterator<S> {
    // In multi-threaded mode: NO-OP, does nothing
    fn wait_for_progress(&self) {}
}
```

In multi-threaded mode, `run_until_stream_has_value()` is a no-op because it assumes background tasks will complete asynchronously. But the background tasks are waiting for I/O that never arrives.

## Attempted Solutions

### Attempt 1: `from_future()` wrapper

**Idea:** Wrap the fetch in `from_future()` to use `FutureTask`

```rust
let fetch_task = from_future(async move {
    fetch_gcp_directory(client).await
});
execute(fetch_task)
```

**Result:** Same deadlock. Still uses `execute()` internally, still schedules on task executor threads.

### Attempt 2: `run_future_iter()`

**Idea:** Use `run_future_iter()` which spawns background jobs via `run_background_job()`

```rust
run_future_iter(|| async {
    fetch_gcp_specs_inner(client).await
}, None, None)
```

**Result:** Still deadlocked. The issue is that the fetch logic still used `SimpleHttpClient.execute()` internally, which schedules tasks on the task executor, not the background job threads.

### Attempt 3: `std::thread::spawn` with `execute()` inside

**Idea:** Spawn a std::thread, then call `execute()` inside it

```rust
std::thread::spawn(move || {
    let task = execute(fetch_task(client.clone()));
    for item in task { /* process */ }
});
```

**Result:** Same deadlock. The `execute()` call still schedules on the global task executor, which has the same I/O polling problem.

### Attempt 4: `reqwest` blocking client

**Idea:** Use reqwest's blocking HTTP client instead of Valtron-based SimpleHttpClient

```rust
let client = reqwest::blocking::Client::new();
let response = client.get(url).send()?;
```

**Result:** Build failed with OpenSSL linker errors:
```
mold: error: undefined symbol: SSL_write_ex
mold: error: undefined symbol: SSL_read_ex
mold: error: undefined symbol: SSL_CTX_ctrl
```

These came from transitive dependencies (`hf-hub`, `foundation_auth`) pulling in `native-tls/openssl`.

### Attempt 5: `ureq` synchronous HTTP (SUCCESS)

**Idea:** Use `ureq` - a purely synchronous HTTP client with no async runtime dependencies

```rust
use ureq;

let response = ureq::get(GCP_DISCOVERY_URL)
    .timeout(std::time::Duration::from_secs(60))
    .call()?;
let text = response.into_string()?;
```

**Result:** SUCCESS! The fetch completed in ~11 seconds.

**Why it works:**
1. `ureq` is purely synchronous - no futures, no async runtime
2. Blocking I/O happens on the `std::thread::spawn` background thread
3. No Valtron executor involvement in the I/O path
4. Results sent through `mpsc::channel` to main thread
5. `ReceiverStreamIterator` wraps the channel as a `StreamIterator` for API compatibility

## Final Implementation

### File: `backends/foundation_deployment/src/providers/gcp.rs`

The GCP provider uses synchronous HTTP via `ureq`:

```rust
use std::sync::mpsc;
use std::path::PathBuf;
use std::time::Duration;

/// Wrapper that converts mpsc Receiver into a StreamIterator.
pub struct ReceiverStreamIterator<T> {
    receiver: Option<mpsc::Receiver<Result<T, DeploymentError>>>,
}

impl<T: Send + 'static> Iterator for ReceiverStreamIterator<T> {
    type Item = Stream<Result<T, DeploymentError>, GcpFetchPending>;
    // ... implementation
}

pub fn fetch_gcp_specs(
    _client: &SimpleHttpClient,  // unused, kept for API compatibility
    output_dir: PathBuf,
) -> Result<impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = GcpFetchPending> + Send + 'static, DeploymentError> {
    let (tx, rx) = mpsc::channel::<Result<PathBuf, DeploymentError>>();
    
    std::thread::spawn(move || {
        // Fetch directory using ureq
        let directory_text = match ureq::get(GCP_DISCOVERY_URL)
            .timeout(Duration::from_secs(60))
            .call()
        {
            Ok(response) => response.into_string().unwrap(),
            Err(e) => {
                let _ = tx.send(Err(DeploymentError::Generic(format!("Failed: {e}"))));
                return;
            }
        };
        
        // Parse JSON, filter APIs, fetch each spec
        // ... (see full implementation in gcp.rs)
        
        let _ = tx.send(Ok(output_path));
    });
    
    Ok(ReceiverStreamIterator::new(rx))
}
```

### File: `backends/foundation_deployment/Cargo.toml`

```toml
[dependencies]
ureq = "2.12"  # Added for synchronous HTTP
```

## Test Results

```bash
$ timeout 120 ./target/debug/ewe_platform gen_provider_specs --provider gcp

2026-04-01T16:21:20.347883Z  INFO foundation_deployment::providers::spec_fetch::gcp: Fetching GCP API directory...
2026-04-01T16:21:20.548607Z  INFO foundation_deployment::providers::spec_fetch::gcp: Found 12 GCP APIs in directory, filtered to 12 relevant APIs
2026-04-01T16:21:20.548619Z  INFO foundation_deployment::providers::spec_fetch::gcp: Fetching compute (alpha)...
2026-04-01T16:21:21.888139Z  INFO foundation_deployment::providers::spec_fetch::gcp:   Loaded: compute (alpha)
# ... fetches all 12 API versions ...
2026-04-01T16:21:31.432438Z  INFO foundation_deployment::providers::spec_fetch::gcp: GCP spec saved to: artefacts/cloud_providers/gcp/openapi.json

$ ls -lh artefacts/cloud_providers/gcp/
-rw-r--r-- 1 darkvoid dark void 2.5K Apr  2 00:21 _manifest.json
-rw-r--r-- 1 darkvoid dark void  23M Apr  2 00:21 openapi.json
```

**Completed in ~11 seconds** (well under the 120-second timeout)

## Lessons Learned

### When to use `std::thread::spawn` vs Valtron `execute()`

| Use Case | Mechanism |
|----------|-----------|
| CPU-bound tasks, async operations | `execute()` with `TaskIterator` |
| Blocking I/O (HTTP, file, DB) | `std::thread::spawn` or `run_background_job()` |
| Background jobs that run to completion | `run_background_job()` via `BackgroundJobRegistry` |

### Why the confusion?

The Valtron executor naming is misleading:
- `execute()` sounds like it should "just work" for any operation
- But it's designed for `TaskIterator` polling, not blocking I/O
- `run_background_job()` is the correct mechanism for blocking operations
- `BackgroundJobRegistry` runs on dedicated threads separate from task executors

### The proper pattern for blocking I/O in Valtron

```rust
// Option 1: Direct std::thread (what we used)
std::thread::spawn(move || {
    // blocking I/O here
});

// Option 2: Via BackgroundJobRegistry (better for resource management)
run_background_job(move || {
    // blocking I/O here
});
```

Both avoid the task executor threads that lack I/O integration.

## Open Questions

1. **Should `SimpleHttpClient` be fixed?** The current implementation uses `execute()` internally for HTTP operations, which means it has the same deadlock risk. Either:
   - Fix it to use `run_background_job()` internally
   - Deprecate it in favor of synchronous HTTP for spec fetching
   - Document the limitation clearly

2. **Should there be a helper for blocking I/O?** A pattern like:
   ```rust
   run_on_background_thread(move || {
       // blocking operation
       result
   })
   ```
   That wraps `run_background_job` with channel handling.

3. **Why does `run_until_stream_has_value()` no-op in multi mode?** This seems like a design flaw - if anything, multi-threaded mode needs MORE waiting logic, not less, because results come from separate threads.

## Files Modified

- `backends/foundation_deployment/Cargo.toml` - Added `ureq = "2.12"`
- `backends/foundation_deployment/src/providers/gcp.rs` - Updated to use synchronous HTTP

## Files Read (for understanding)

- `backends/foundation_core/src/valtron/executors/future_task.rs` - `from_future`, `run_future_iter`
- `backends/foundation_core/src/valtron/executors/drivers.rs` - `DrivenStreamIterator`, `run_until_stream_has_value`
- `backends/foundation_core/src/valtron/executors/unified.rs` - `execute()`, `run_background_job`
- `bin/platform/src/gen_provider_specs/fetcher.rs` - Consumption pattern for spec fetchers
