---
# Identification
spec_name: "02-build-http-client"
spec_number: 2
feature_name: connection-pooling
feature_number: 11

# Location Context
workspace_root: ewe_platform (CWD)
this_file: specifications/02-build-http-client/features/connection-pooling/feature.md

# Status
status: pending
priority: medium
depends_on:
  - connection
estimated_effort: small-medium
created: 2026-02-18
last_updated: 4/18/2026
author: Claude Opus AI with user decisions applied

---

*Created: 2026-02-18*
*Updated: 4/18/2026 with complete design specification and user decisions applied (PooledConnection public, case-insensitive keys, conservative defaults)*

# Context Optimization
machine_optimized: true
context_optimization: true

# Tasks
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0

# Files Required by Agents
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
    files:
      - ../requirements.md

  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md

---

# Connection Pooling Feature

## 📍 Location Reference (CRITICAL)

**Verify location**: Run `bash pwd` from this file's directory. Should output: ewe_platform

**Quick paths**:
```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform && test -f .agents/AGENTS.md && echo "✓ Workspace root verified"
cat specifications/02-build-http-client/features/connection-pooling/machine_prompt.md  # Read compressed version for sub-agents
```

---

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this feature MUST use retrieval-led reasoning.**

### Before Starting Implementation - MANDATORY CHECKLIST:

1. ✅ **Read existing stub**: `backends/foundation_core/src/wire/simple_http/client/pool.rs`
2. ✅ **Check Connection API dependency**: Read connection feature's implementation
3. ✅ **Search codebase for patterns**:
   ```bash
   grep -r "SharedByteBufferStream" backends/
   ```

### FORBIDDEN Approaches:

- ❌ Assuming pooling pattern from other projects without verifying project conventions
- ❌ Implementing stub methods instead of real logic

---

## 🚀 CRITICAL: Token and Context Optimization (Rule 14/15)

**Sub-agents MUST read compressed machine_prompt.md, NOT this verbose file.**

```bash
# Sub-agent reads THIS for context:
cat specifications/02-build-http-client/features/connection-pooling/machine_prompt.md
```

---

## Overview

Implement connection pooling to reuse TCP/TLS connections across requests.

**Purpose**: Reduce latency from repeated handshakes by reusing established connections.
**File**: `backends/foundation_core/src/wire/simple_http/client/pool.rs` (STUB - needs implementation)

---

# CONNECTION POOL STRUCTURE DESIGN SUMMARY

## User Decisions Confirmed ✅
1. **Configuration Defaults** → Conservative: 5 per host, 30s idle timeout (browser-like)
2. **Cleanup Strategy** → Hybrid checkout + manual full maintenance
3. **Metrics Level** → Comprehensive with per-host stats including eviction count and hit rate
4. **Integration Pattern** → User initializes pool via builder/creation time injection

---

## Connection Pool Structure - COMPLETE IDEATION BELOW:

### Current Spec (Incomplete) ❌
```rust
// TOO SIMPLE - missing critical components:
pub struct ConnectionPool {
    max_per_host: usize,
    max_idle_time: Duration
}
```

**Problems**: Missing:
- Thread-safe state storage structure (`Arc<Mutex<HashMap>>`)
- Configuration validation and builder pattern support
- Metrics tracking fields for observability
- Host key normalization logic (case-insensitive)
- Error type definitions

### Proposed Complete Structure ✅:

```rust
use std::sync::{StdMutex, Arc};
use foundation_nostd::comp::basic::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Main connection pool managing reusable TCP/TLS connections per destination.

pub struct ConnectionPool {
    /// Maximum number of idle connections to maintain per host:port bucket.
    config: PoolConfig,

    /// Thread-safe storage mapping "host:port" → VecDeque<PooledConnection>
    buckets: Arc<StdMutex<HashMap<String, VecDeque<PooledConnection>>>>,
}

/// Configuration for connection pool behavior.

#[derive(Debug)]
pub struct PoolConfig {
    pub max_per_host: usize,
    pub default_max_idle_time: Duration,

    /// Optional custom idle time per host (overrides defaults)
    #[cfg(feature = "per-host-timeout")]
    pub per_host_timeout_map: HashMap<String, Option<Duration>>,
}

/// Per-destination connection pool statistics for monitoring.

#[derive(Debug)]
pub struct PoolMetrics {
    /// Total active connections checked out across all hosts
    pub total_active_connections: usize,

    /// Total idle connections waiting in buckets (not expired)
    pub total_idle_connections: usize,
}
```

### Internal Connection Tracking:

```rust
/// Public connection tracking with lifecycle timestamps.
///
/// Note: PooledConnection is PUBLIC - no private encapsulation needed per user decision.

pub struct PooledConnection {
    pub stream: SharedByteBufferStream<RawStream>,
    created_at: Instant,
    last_used_at: Instant,

    /// Per-host custom idle timeout (optional, overrides PoolConfig)
    #[cfg(feature = "per-host-timeout")]
    per_host_timeout: Option<Duration>,
}
```

### Helper Functions:

```rust
/// Normalize host to lowercase for case-insensitive bucket lookup.
fn make_key(host: &str) -> String {
    format!("{}:{}", host.to_lowercase(), port)
}

/// Check if connection is fresh (not stale).
fn is_fresh(conn: &PooledConnection, max_idle_time: Duration) -> bool {
    conn.last_used_at.elapsed() <= max_idle_time
}
```

---

## Key Design Decisions:

### 1. State Storage Pattern ✅

```rust
// Thread-safe bucket storage mapping host:port → idle connections deque
buckets: Arc<StdMutex<HashMap<String, VecDeque<PooledConnection>>>>
```

**Why this pattern?**
- HashMap provides O(1) lookup by normalized "host:port" key (case-insensitive)
- Mutex protects concurrent access (checkout/checkin are hot paths with high contention risk due to network ops blocking locks)
- VecDeque allows efficient LRU eviction from tail and auto-cleanup check of head

### 2. PooledConnection Visibility ✅ USER DECISION
```rust
pub struct PooledConnection { ... }
```
**Decision**: Public - no private encapsulation needed.

### 3. Host Key Normalization ✅ USER DECISION (4/18/2026)
Case-insensitive, always lowercased via make_key() function.
Matches existing implementation pattern in other parts of codebase if any exists.

---

## Checkout Logic Flow:

```rust
pub fn checkout(&self, host: &str, port: u16) -> Option<SharedByteBufferStream<RawStream>> {
    let key = make_key(host, port);

    // Attempt to acquire lock with short timeout (non-blocking)
    match self.buckets.try_lock_for(Duration::from_millis(10)) {
        Ok(guard) => guard.get(&key).and_then(|bucket| bucket.front()),
        Err(_) => return None,
    }
}
```

**Edge Case Handling Specified:**

| Scenario | Behavior |
|----------|-----------|
| Empty bucket for host:port? | Create new connection, add immediately as first item in deque (front = newest/cleanest) |
| All connections stale at front of deque? | Remove all expired from head until empty or non-stale found → create fresh one and return it |
| Checked-out stream expires during use (user holds too long)? | No auto-cleanup - wait for user to checkin. Staleness only checked on checkout before returning usable connection |

---

## Checkin Logic Flow:

```rust
pub fn checkin(&self, host: &str, port: u16, stream: SharedByteBufferStream<RawStream>) {
    let key = make_key(host, port);

    match self.buckets.try_lock_for(Duration::from_millis(10)) {
        Ok(guard) => guard.entry(key).or_insert_with(VecDeque::new),
            .push_back(PooledConnection { ... });
        Err(_) => drop(stream);  // Silently discard broken stream
    }
}
```

**LRU Eviction at Tail:**
- Enforces max_per_host limit
- Discards oldest idle connections when bucket exceeds size

---

## Cleanup Methods:

```rust
/// Remove all expired stale connections from ALL buckets.
pub fn cleanup_stale(&self) {
    let mut guard = self.buckets.lock().unwrap();
    for (_, bucket) in &mut *guard {
        // Remove items where last_used_at.elapsed() > max_idle_time
        while !bucket.is_empty()
            && is_fresh(bucket.front(), config.default_max_idle_time)
                .not()
        {
            drop(bucket.pop_front());
          stats.total_stale_connections += 1;
        }
    }
}

/// Atomically clear entire pool contents.
pub fn clear(&self) {
    *self.buckets.lock().unwrap() = HashMap::new();
}
```

**Hybrid Cleanup Strategy:**
- **Auto on checkout**: Remove expired from head of deque before returning
- **Explicit cleanup**: `cleanup_stale()` called for shutdown or scheduled maintenance (not every operation to avoid contention)
- **Drop trait fallback**: If user forgets to checkin, Drop cleans up and records metrics

---

## Integration Pattern Documentation:

**HttpClient Flow with Pool Checkout/Checkin:**

```rust
impl HttpClient {
    /// Send HTTP GET request using pooled connection if available.

    pub async fn get(&self, url: &str) -> Result<Response> {
        let parsed = ParsedUrl::parse(url)?;
        let (host, port) = (parsed.host.clone(), parsed.port);

        // Checkout attempt - returns existing idle stream or None
        match self.pool.checkout(host.as_str(), port).await? {
            Some(stream) => { /* use pooled connection */ }
            None => create_fresh_connection(&parsed),  // Fallback path
        };

        // ... send HTTP request ...
    }

    /// Return a checked-out connection back to the pool.

    pub fn checkin(&self, host: &str, port: u16, stream: SharedByteBufferStream<RawStream>) {
        self.pool.checkin(host.as_str(), port, stream);
    }
}
```

---

## Metrics Tracking (OpenTelemetry Integration):

Per-host metrics tracked via tracing spans:

- `connection.pool.hits` - Number of successful pool retrievals
- `connection.pool.misses` - Times no available idle connection, created fresh one
- `connection.pool.active_connections.count`
- `connection.pool.idle_bucket_size.count`

Example implementation:
```rust
fn record_hit(host_key: &str) {
    let mut stats = HOST_STATS.lock().unwrap();
    *stats.entry(host_key.to_string()).or_default()
        .pool_hits += 1;
}

fn record_miss(host_key: &str, active_count: usize, idle_bucket_size: usize) {
    tracing::span!(
        "connection-pool-create",
        host = host_key,
        pool_active_connections = active_count
    );
}
```

---

## Success Criteria (Pipe-Delimited)

1. [ ] ConnectionPool implements full logic, NOT stub
2. [ ] checkout() returns Option<stream> correctly handling pool exhaustion and stale items
3. [ ] checkin() adds to correct "host:port" bucket respecting size limit with LRU eviction at tail
4. [ ] Pool enforces max_per_host - discards excess connections when full (evicts oldest from tail)
5. [ ] Stale detection works on all methods:
    * `checkout()` removes expired items from head before returning
    * `cleanup_stale()` scans and clears ALL expired across buckets
6. [ ] Manual cleanup callable for maintenance tasks (`cleanup_stale()`)
7. [ ] clear() removes ALL pooled connections atomically without panic (for testing/shutdown)
8. [ ] Public PooledConnection struct exposed with stream, created_at, last_used_at fields

---

## Dependencies

- **Depends on**: `connection` feature
  - Reason: Uses HttpClientConnection for actual connection instances via SharedByteBufferStream<RawStream>
- **Required by**: No features list this as dependency yet

**Verification needed before starting**:
```bash
# Verify Connection API exists and is complete
grep -r "HttpClientConnection" backends/foundation_core/src/
```

---

## Requirements Summary (Pipe-Delimited)

### Core Pooling Structure ✅ COMPLETE IDEATION:

#### Configuration with Conservative Defaults (`PoolConfig`)
- `max_per_host` = 5 per host, default configuration for conservative browser-like behavior
- `default_max_idle_time` = Duration::from_secs(30) seconds

```rust
pub struct PoolConfig {
    pub max_per_host: usize,
    pub default_max_idle_time: Duration,

    #[cfg(feature = "per-host-timeout")]
    pub per_host_timeout_map: HashMap<String, Option<Duration>>,
}
```

#### Main ConnectionPool Structure ✅ COMPLETE IDEATION:
- Thread-safe storage mapping normalized host:port → VecDeque<PooledConnection>
- LRU eviction at tail of deque on checkin
- Auto-cleanup from head during checkout

```rust
pub struct ConnectionPool {
    config: PoolConfig,
    buckets: Arc<StdMutex<HashMap<String, VecDeque<PooledConnection>>>>,
}
```

### Checkout/Checkin Pattern ✅ COMPLETE IDEATION:

#### Public API with Non-blocking Lock Handling:
```rust
#[must_use]
pub fn checkout(&self, host: &str, port: u16) -> Option<SharedByteBufferStream<RawStream>>

pub fn checkin(&self, host: &str, port: u16, stream: SharedByteBufferStream<RawStream>)
```

**Key Behavior Notes:**
- `checkout()` returns existing idle or creates fresh one
- Returns None if all connections stale and exhausted (caller falls back to new connection)
- Auto-cleans expired from head before returning

### Cleanup Methods ✅ COMPLETE IDEATION:

```rust
/// Remove ALL expired stale connections across buckets.
pub fn cleanup_stale(&self)

/// Atomically clear entire pool contents for testing/shutdown.
pub fn clear(&self)
```

**Cleanup Strategy:**
1. Auto on checkout: Removes expired from head of deque before returning usable connection
2. Explicit `cleanup_stale()`: Called manually for shutdown or scheduled maintenance (not every operation to avoid contention)
3. Drop trait fallback: If user forgets to checkin, clean up and record metrics

### Connection Tracking Struct ✅ COMPLETE IDEATION:

```rust
/// Public struct - no private encapsulation needed per user decision.
pub struct PooledConnection {
    pub stream: SharedByteBufferStream<RawStream>,
    created_at: Instant,
    last_used_at: Instant,

    #[cfg(feature = "per-host-timeout")]
    per_host_timeout: Option<Duration>,
}
```

### Helper Functions ✅ COMPLETE IDEATION:

```rust
/// Normalize host to lowercase for case-insensitive bucket lookup.
fn make_key(host: &str, port: u16) -> String {
    format!("{}:{}", host.to_lowercase(), port)
}

/// Check if connection is fresh (not stale).
fn is_fresh(conn: &PooledConnection, max_idle_time: Duration) -> bool
```

### Thread Safety Pattern ✅ COMPLETE IDEATION:
- Use `Arc<StdMutex<HashMap<String, VecDeque<PooledConnection>>>>` for pool state storage

**Decision**: Matches existing project patterns (foundation_nostd::comp::basic::Mutex)

---

## Implementation Requirements (Pipe-Delimited)

1. **Remove stub**: Replace TODO comments with real implementations
2. **Implement `PoolConfig` struct**:
   - Fields: max_per_host, default_max_idle_time
3. **Implement ConnectionPool main structure**:
   - Thread-safe bucket storage using Arc<StdMutex<HashMap>>
   - LRU eviction at tail of deque on checkin when exceeding max_per_host limit
4. **Checkout logic** (`pub fn checkout`):
   - Return valid stream OR None when exhausted/stale items only exist in head (caller falls back to new connection)
   - Auto-cleanup: Remove expired from front before returning usable item
5. **Checkin logic** (`pub fn checkin`):
   - Add connection to appropriate bucket with normalized key ("host:port")
   - Enforce max_per_host limit by evicting oldest idle (tail) if needed
6. **Cleanup methods**:
   - `cleanup_stale()`: Scan all buckets, remove entries where last_used_at.elapsed() > config.default_max_idle_time
   - `clear()`: Atomically clear entire HashMap contents for testing/shutdown cleanup
7. **Connection tracking struct (`PooledConnection`)** with public visibility and fields: stream, created_at (Instant), last_used_at (Instant)
8. **Stale detection**: Check connections where `last_used_at.elapsed() > max_idle_time`
9. **Metrics integration**:
   - Track per-host connection pool hits/misses
   - Per-destination eviction count when limit exceeded

---

## Verification Commands (Pipe-Delimited)

```bash
# Check for stubs/TODO comments before any other checks
grep -rn "TODO\|FIXME\|unimplemented!\|todo!" backends/foundation_core/src/wire/simple_http/client/pool.rs || echo "✓ No stubs"

cargo fmt --check backends/foundation_core/src/wire/simple_http/client/pool.rs
cargo clippy --package foundation_core -- -D warnings 2>&1 | grep pool || echo "✓ No clippy issues"
cargo test --package foundation_core --lib wire::simple_http::client::pool

# Verify connection pooling integration with HttpClient (after implementation)
grep -rn "ConnectionPool" backends/foundation_core/src/wire/simple_http/client/*.rs
```

---

## Agent Instructions (Pipe-Delimited)

**Implementation Agents MUST**: Read this spec for complete design details, implement all methods:
- PoolConfig struct construction and defaults (max_per_host = 5, default_max_idle_time = Duration::from_secs(30))
- ConnectionPool main structure with Arc<StdMutex<HashMap>>
- checkout() logic: auto-cleanup from head + return or None fallback to new connection creation
- checkin() logic: add to bucket via normalized key (lowercase) + LRU eviction at tail if exceeding max_per_host

**Verification Agents MUST**: Run incomplete implementation scan FIRST before any other checks:
```bash
grep -rn "TODO\|FIXME\|unimplemented!\|todo!" backends/foundation_core/src/wire/simple_http/client/pool.rs || echo "✓ No stubs"
```

---

*Created: 2026-02-18*
*Updated: 4/18/2026 with complete design specification and user decisions applied (PooledConnection public, case-insensitive keys, conservative defaults)*
