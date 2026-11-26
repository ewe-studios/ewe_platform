---
feature: "Sandbox lifetime and stream cost"
status: "pending"
priority: "medium"
model: "sonnet"
phase: "2"
depends_on: ["07-cloudflare-control-plane", "08-cloudflare-worker"]
crates: ["foundation_sandbox"]
---

# Feature 18: Sandbox lifetime and stream cost on Cloudflare

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Durable Object (DO); remote procedure call (RPC);
Cloudflare Worker (Worker); time to live (TTL); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Two changes reduce the cost of a Cloudflare sandbox. Both come from the Durable Streams
study in `../../research.md`.

1. **One alarm, two deadlines.** A sandbox can have a lifetime limit. The idle deadline and
   the lifetime deadline share the one alarm of the DO. The sandbox stops at the earlier
   deadline.
2. **A bound on a stream.** An open `Exec` stream keeps the DO awake and billed. A stream
   that runs too long ends, and the slave kills its program.

A sandbox that nobody uses must not cost money. A sandbox that somebody forgot must not
live for ever. A program that never ends must not hold a DO awake for ever.

## 2. Story

- **S1**: "I create a sandbox for one build. It must stop by itself after one hour, even if
  it is busy."
- **S2**: "I create a sandbox with no lifetime. It must stop when it is idle, as it does
  today."
- **S3**: "A command writes output for two hours. I want a clear error at the limit, and no
  process that survives it."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Limited lifetime | caller | `Create` with `ttl` | The record holds `expires_at`; the alarm is at the earlier of the two deadlines. |
| U2 | Idle stop | the alarm | the idle deadline passed | `stop`; the record is gone. |
| U3 | Lifetime stop | the alarm | the lifetime deadline passed | `stop`; the record is gone. |
| U4 | Stream limit | an `Exec` stream | `stream_max_duration` passed | The stream ends with `DeadlineExceeded`; the slave kills the program. |

## 3. Bounded context

**Inside:** the `ttl` field of `SandboxSpec`, the lifetime deadline in the record, the one
alarm rule, the `stream_max_duration` limit, and their tests.

**Outside:** the alarm API (feature 07, section 6.6); the idle rule (feature 07); the slave
(feature 01); the Worker entry (feature 08).

**Words.** *Lifetime deadline*: the time after which a sandbox stops, whatever it does.
*Idle deadline*: the time after which a sandbox with no request stops. The DO has exactly
one alarm, so the control plane sets it to the earlier of the two.

## 4. Rules

### 4.1 Invariants

| # | Invariant |
|---|---|
| I1 | The alarm is set to the earlier of the idle deadline and the lifetime deadline. The control plane never needs two alarms. |
| I2 | When both deadlines pass, the reason is `Expired`, and the record is removed. |
| I3 | A sandbox with a lifetime that passed while the DO was out of memory is removed on `restore`. A later call answers `NotFound`. |
| I4 | `touch` moves the idle deadline only. It never moves the lifetime deadline. |
| I5 | An `Exec` stream open longer than `stream_max_duration` ends with `Rpc { code: "deadline_exceeded" }`. The program does not survive the stream (feature 01, invariant I8). |
| I6 | `ttl` is a request field with a default of `None`. An older client that does not send it gets today's behaviour. |

### 4.2 Must not

- Must not add a second timer, a cron trigger or a timer task per sandbox.
- Must not change the RPC surface other than the additive `ttl` field and the stream error.

## 5. Technical breakdown

### 5.1 The contract change

```rust
// shared/spec.rs
pub struct SandboxSpec {
    // ...
    /// The time after which the sandbox stops, whatever it does.
    ///
    /// `None` (the default) gives no lifetime limit. The record keeps the
    /// deadline, so a restart of the control plane does not extend it.
    pub ttl: Option<Duration>,
}
```

`Duration` uses the same serde form as the other limit fields of this crate. The record
gains `expires_at_unix_ms: Option<u64>` in `sandbox` storage. The `SandboxInfo` of `Get` and
`List` gains `expires_at_unix_ms`.

### 5.2 The one-alarm rule (feature 07, section 6.6)

```
arm = min(now + idle_timeout, expires_at)
alarm.set(arm)
```

`touch` sets a new idle deadline and re-arms. The alarm handler:

| Condition | Action |
|---|---|
| `now >= expires_at` | `stop` with reason `Expired`; remove the record. |
| `now >= idle_deadline` | `stop` with reason `Idle`; remove the record. |
| a sandbox exists | `alarm.set(min(next idle deadline, expires_at))`. |
| no sandbox | `alarm.clear()`. |

`restore` reads both deadlines. A record whose `expires_at` passed is destroyed and removed,
as a `Creating` record is today. A record whose idle deadline passed but whose container
still runs is treated as idle: the next request touches it; an alarm stops it.

### 5.3 The stream limit

`CloudflareConfig` gains:

| Field | Meaning | Default |
|---|---|---|
| `stream_max_duration` | The longest time that one `Exec`, `FileWrite` or `FileRead` stream stays open. | 1 hour |

The `DoControlPlane` wraps the streaming responses of `ControlService` in a deadline guard on
the `Ctx` clock. At the deadline the stream sends no more events and ends with `Rpc { code:
"deadline_exceeded" }`. The slave sees its stream end and kills the program (feature 01,
invariant I8). The default of one hour is a configuration value; an operator can raise it.

The value is per stream, not per sandbox. A second command gets a new stream and a new
deadline.

### 5.4 Why not hibernation

The Durable Streams project keeps a DO asleep behind a hibernatable WebSocket. Our service
is RPC over HTTP: an open response stream is an open request, and the runtime keeps the DO
awake for it. Hibernation needs a WebSocket and a stored attachment, which changes the
protocol of `SandboxService`.

This feature does not change the protocol. It bounds the wake time instead. A later feature
may move large output to R2 behind a signed URL; that removes the long stream rather than
sleeping through it.

## 6. Tests

Native simulation tests in `backends/foundation_sandbox/tests/cloudflare/lifetime.rs` and
`tests/cloudflare/stream_cost.rs`, with the fakes of feature 07 (`FakeAlarm`,
`MemoryDoStorage`, the simulated clock). The real-slave case is Linux only.

| Law | Test | Oracle |
|---|---|---|
| I1, I2 | `alarm_at_earlier_deadline`, `expires_before_idle`, `idle_before_expires` | stateful model |
| I3 | `restore_expired_removes` | expected outcome |
| I4 | `touch_keeps_expiry` | example |
| I6 | `old_client_has_no_ttl` (a JSON body with no `ttl` decodes to `None`) | example |
| I5 | `stream_cap_ends_with_deadline` (a stream that never ends; the simulated clock steps past the cap) | expected outcome |
| I5, the program dies | `stream_cap_kills_program` (real slave behind the fake container; Linux only) | conservation |
| the model | `lifetime_model` (proptest: random `ttl`, idle timeouts and touches; the count of stops and the last stop reason equal the model) | stateful model |

## 7. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_sandbox --features "testing cloudflare" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_sandbox --features "testing cloudflare" --profile uat cloudflare::lifetime cloudflare::stream_cost
cargo check -p foundation_sandbox --no-default-features --features "control cloudflare" --target wasm32-unknown-unknown --profile uat
```

## 8. Done when

- A sandbox with `ttl` stops at the lifetime deadline, and a sandbox without one behaves as
  today.
- The control plane sets one alarm, at the earlier deadline.
- A stream that passes `stream_max_duration` ends with `DeadlineExceeded`, and the program
  does not survive.
- `research.md` holds the reason for each choice, with the Cloudflare fact that supports it.
