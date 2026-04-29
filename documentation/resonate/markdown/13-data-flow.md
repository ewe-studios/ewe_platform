# Resonate -- Data Flow

## Overview

This document traces data through the system during the key operations: function invocation, suspension and resumption, settlement chains, and crash recovery. Each flow shows the exact sequence of protocol messages, database operations, and transport deliveries.

## Flow 1: Simple Invocation (No Suspension)

A client invokes a function that completes in a single execution pass.

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    participant DB as Database
    participant T as Transport
    participant W as Worker

    C->>S: POST / {kind: "promise.create", data: {id: "job.1", ...}}
    S->>DB: INSERT INTO promises (id, state, param, tags, timeout_at)
    S->>DB: INSERT INTO promise_timeouts (timeout_at, id)
    S->>DB: INSERT INTO tasks (id, state='pending', version=0)
    S->>DB: INSERT INTO outgoing_execute (id, version, address)
    S-->>C: {status: 201, data: {id: "job.1", state: "pending"}}

    Note over S: Message processing loop (100ms)
    S->>DB: SELECT + DELETE FROM outgoing_execute LIMIT 100
    S->>T: dispatch("poll://any@workers", execute_payload)
    T->>W: SSE event: {kind: "execute", data: {task: {id: "job.1", version: 0}}}

    W->>S: POST / {kind: "task.acquire", data: {id: "job.1", version: 0}}
    S->>DB: UPDATE tasks SET state='acquired', version=1
    S->>DB: INSERT INTO task_timeouts (timeout_at, id, type=1/lease)
    S->>DB: SELECT * FROM promises WHERE branch='job.1' AND state != 'pending'
    S-->>W: {status: 200, data: {task: {...}, preload: []}}

    Note over W: Execute registered function
    W->>W: fn(ctx, args)
    
    W->>S: POST / {kind: "task.fulfill", data: {id: "job.1", version: 1, value: {...}}}
    S->>DB: BEGIN TRANSACTION
    S->>DB: UPDATE promises SET state='resolved', value=..., settled_at=...
    S->>DB: DELETE FROM promise_timeouts WHERE id='job.1'
    S->>DB: UPDATE tasks SET state='fulfilled'
    S->>DB: DELETE FROM task_timeouts WHERE id='job.1'
    S->>DB: COMMIT
    S-->>W: {status: 200}
```

## Flow 2: Invocation with Child Tasks

A workflow function creates child promises for sub-tasks.

```mermaid
sequenceDiagram
    participant W as Worker
    participant S as Server
    participant DB as Database

    Note over W: Executing workflow: process_order(ctx, order)
    
    Note over W: ctx.run(charge_card, payment)
    W->>S: promise.create {id: "job.1.0", tags: {invoke: "charge_card"}}
    S->>DB: INSERT promise + task for job.1.0
    S-->>W: {status: 201, state: "pending"}
    
    Note over W: Worker executes charge_card locally
    W->>W: charge_card(ctx, payment) → result
    
    W->>S: promise.settle {id: "job.1.0", state: "resolved", value: result}
    S->>DB: Settlement chain for job.1.0
    S-->>W: {status: 200}
    
    Note over W: ctx.run(ship_items, items)
    W->>S: promise.create {id: "job.1.1", tags: {invoke: "ship_items"}}
    S->>DB: INSERT promise + task for job.1.1
    S-->>W: {status: 201}
    
    W->>W: ship_items(ctx, items) → result
    W->>S: promise.settle {id: "job.1.1", state: "resolved", value: result}
    S-->>W: {status: 200}
    
    Note over W: All steps complete
    W->>S: task.fulfill {id: "job.1", version: 1, value: final_result}
    S->>DB: Settlement chain for job.1 (root)
    S-->>W: {status: 200}
```

## Flow 3: Suspension and Resumption

A workflow suspends when it encounters an unresolved remote dependency.

```mermaid
sequenceDiagram
    participant W1 as Worker A
    participant S as Server
    participant DB as Database
    participant W2 as Worker B

    Note over W1: Executing workflow, hits ctx.rpc("validate", data)
    
    W1->>S: promise.create {id: "job.1.2", target: "validation-workers"}
    S->>DB: INSERT promise (pending) + task (pending)
    S->>DB: INSERT outgoing_execute (job.1.2, validation-workers)
    S-->>W1: {status: 201, state: "pending"}
    
    Note over W1: Remote promise is pending → must suspend
    W1->>S: task.suspend {id: "job.1", version: 1, awaited: ["job.1.2"]}
    S->>DB: UPDATE tasks SET state='suspended' WHERE id='job.1'
    S->>DB: INSERT INTO callbacks (awaited='job.1.2', awaiter='job.1', ready=0)
    S->>DB: DELETE FROM task_timeouts WHERE id='job.1'
    S-->>W1: {status: 200}
    Note over W1: Worker A freed — task lease released

    Note over S: Message loop delivers to Worker B
    S->>W2: SSE: {kind: "execute", task: {id: "job.1.2", version: 0}}
    
    W2->>S: task.acquire {id: "job.1.2", version: 0}
    S-->>W2: acquired
    W2->>W2: validate(data) → validation_result
    
    W2->>S: task.fulfill {id: "job.1.2", value: validation_result}
    S->>DB: BEGIN TRANSACTION
    S->>DB: UPDATE promises SET state='resolved' WHERE id='job.1.2'
    S->>DB: UPDATE callbacks SET ready=1 WHERE awaited='job.1.2'
    Note over DB: Check: all callbacks for job.1 ready?
    S->>DB: SELECT COUNT(*) FROM callbacks WHERE awaiter='job.1' AND ready=0
    Note over DB: Count = 0 → all ready!
    S->>DB: UPDATE tasks SET state='pending' WHERE id='job.1'
    S->>DB: INSERT INTO outgoing_execute (id='job.1', version=2, address)
    S->>DB: COMMIT
    S-->>W2: {status: 200}
    
    Note over S: Message loop re-dispatches job.1
    S->>W1: SSE: {kind: "execute", task: {id: "job.1", version: 2}}
    
    W1->>S: task.acquire {id: "job.1", version: 2}
    S->>DB: Preload: return resolved promises [job.1.0, job.1.1, job.1.2]
    S-->>W1: {task: {...}, preload: [job.1.0=resolved, job.1.1=resolved, job.1.2=resolved]}
    
    Note over W1: Replay: feed cached results for .0 and .1
    Note over W1: Continue: use resolved .2 value
    Note over W1: Complete remaining work
    
    W1->>S: task.fulfill {id: "job.1", value: final_result}
    S->>DB: Settlement chain for job.1
    S-->>W1: {status: 200}
```

## Flow 4: Crash Recovery

A worker crashes mid-execution. The server's lease timeout triggers re-dispatch.

```mermaid
sequenceDiagram
    participant W1 as Worker A (crashes)
    participant S as Server
    participant DB as Database
    participant W2 as Worker B (recovery)

    W1->>S: task.acquire {id: "job.1", version: 0}
    S->>DB: UPDATE tasks version=1, INSERT task_timeout (lease, 15s)
    S-->>W1: acquired

    W1->>W1: Execute step 1 (charge_card)
    W1->>S: promise.settle {id: "job.1.0", resolved}
    S-->>W1: ok

    Note over W1: 💥 CRASH (mid-execution, before step 2)
    Note over W1: No heartbeat sent
    
    Note over S: 15 seconds pass...
    Note over S: Timeout processing loop detects expired lease
    
    S->>DB: SELECT FROM task_timeouts WHERE timeout_at <= now AND type=1
    S->>DB: UPDATE tasks SET state='pending', version=2 WHERE id='job.1'
    S->>DB: DELETE FROM task_timeouts WHERE id='job.1'
    S->>DB: INSERT INTO outgoing_execute (job.1, version=2, address)
    
    Note over S: Message loop delivers to any available worker
    S->>W2: SSE: {kind: "execute", task: {id: "job.1", version: 2}}
    
    W2->>S: task.acquire {id: "job.1", version: 2}
    S->>DB: Preload: [job.1.0=resolved]
    S-->>W2: {task: {...}, preload: [job.1.0=resolved]}
    
    Note over W2: Replay: step 1 result from preload (no re-execution)
    Note over W2: Execute: step 2 (ship_items) runs fresh
    
    W2->>S: promise.settle {id: "job.1.1", resolved}
    W2->>S: task.fulfill {id: "job.1", value: result}
    S-->>W2: ok
```

### Key Insight: No Double Execution

Step 1 (charge_card) was already settled before the crash. On recovery:
1. Worker B acquires with version=2
2. Server preloads all resolved promises in the branch
3. When Worker B replays step 1, the SDK returns the cached result from preload
4. Step 2 (ship_items) executes normally — it was never settled

## Flow 5: Scheduled Execution

A cron schedule creates promises on a recurring basis.

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    participant DB as Database
    participant TL as Timeout Loop
    participant W as Worker

    C->>S: schedule.create {id: "daily-cleanup", cron: "0 2 * * *", func: "cleanup"}
    S->>DB: INSERT INTO schedules (id, cron, promise_id, promise_timeout, ...)
    S->>DB: INSERT INTO schedule_timeouts (timeout_at=next_2am, id)
    S-->>C: {status: 201}

    Note over TL: 2:00 AM arrives
    TL->>DB: SELECT FROM schedule_timeouts WHERE timeout_at <= now
    TL->>DB: Process: create promise from schedule template
    TL->>DB: INSERT INTO promises (id="daily-cleanup/2026-04-29", ...)
    TL->>DB: INSERT INTO tasks (id="daily-cleanup/2026-04-29", ...)
    TL->>DB: INSERT INTO outgoing_execute
    TL->>DB: UPDATE schedules SET last_run_at=now, next_run_at=tomorrow_2am
    TL->>DB: UPDATE schedule_timeouts SET timeout_at=tomorrow_2am

    Note over S: Message loop dispatches
    S->>W: execute: {task: {id: "daily-cleanup/2026-04-29"}}
    W->>W: cleanup(ctx, args)
    W->>S: task.fulfill
```

## Flow 6: Listener Notification

An external client registers a listener and receives a webhook when the promise settles.

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    participant DB as Database
    participant ML as Message Loop
    participant WH as Webhook Endpoint

    C->>S: promise.register_listener {awaited: "job.1", address: "https://my.app/webhook"}
    S->>DB: INSERT INTO listeners (promise_id='job.1', address='https://my.app/webhook')
    S-->>C: {status: 200}

    Note over S: Later: job.1 settles
    S->>DB: Settlement chain includes:
    S->>DB: INSERT INTO outgoing_unblock (promise_id='job.1', address='https://my.app/webhook')
    S->>DB: DELETE FROM listeners WHERE promise_id='job.1'

    Note over ML: Message processing loop
    ML->>DB: SELECT + DELETE FROM outgoing_unblock
    ML->>WH: POST https://my.app/webhook {kind: "unblock", data: {promise: {...}}}
    WH-->>ML: 200 OK
```

## Flow 7: Multiple Awaited Promises (Multi-Callback)

A task suspends on multiple dependencies. It resumes only when ALL settle.

```mermaid
sequenceDiagram
    participant W as Worker
    participant S as Server
    participant DB as Database

    Note over W: ctx.all([ctx.rpc("a"), ctx.rpc("b"), ctx.rpc("c")])
    W->>S: promise.create {id: "job.1.0"} (a)
    W->>S: promise.create {id: "job.1.1"} (b)
    W->>S: promise.create {id: "job.1.2"} (c)
    
    W->>S: task.suspend {id: "job.1", awaited: ["job.1.0", "job.1.1", "job.1.2"]}
    S->>DB: INSERT callbacks: (awaited=.0, awaiter=job.1), (.1, job.1), (.2, job.1)
    
    Note over S: Promise .0 settles
    S->>DB: UPDATE callbacks SET ready=1 WHERE awaited='job.1.0'
    S->>DB: Check: unready callbacks for job.1? → 2 remaining
    Note over S: Task stays suspended
    
    Note over S: Promise .1 settles
    S->>DB: UPDATE callbacks SET ready=1 WHERE awaited='job.1.1'
    S->>DB: Check: unready callbacks for job.1? → 1 remaining
    Note over S: Task stays suspended
    
    Note over S: Promise .2 settles
    S->>DB: UPDATE callbacks SET ready=1 WHERE awaited='job.1.2'
    S->>DB: Check: unready callbacks for job.1? → 0 remaining
    S->>DB: UPDATE tasks SET state='pending' WHERE id='job.1'
    S->>DB: INSERT INTO outgoing_execute (job.1, version=2)
    Note over S: Task resumes!
    
    S->>W: execute: {task: {id: "job.1", version: 2}}
```

## Data Flow Summary

```
┌──────────────────────────────────────────────────────────────────┐
│                      HAPPY PATH                                    │
│                                                                    │
│  Client → promise.create → task.create → outgoing_execute         │
│       → transport dispatch → worker SSE                            │
│       → task.acquire (preload) → execute function                  │
│       → promise.settle (children) → task.fulfill                   │
│       → settlement chain → promise resolved                        │
│                                                                    │
├──────────────────────────────────────────────────────────────────┤
│                    SUSPENSION PATH                                 │
│                                                                    │
│  Worker → promise.create (remote) → task.suspend                   │
│       → callbacks registered → worker freed                        │
│       → remote settles → callbacks marked ready                    │
│       → all ready → task resumed → outgoing_execute                │
│       → worker acquires (preload) → replay + continue              │
│                                                                    │
├──────────────────────────────────────────────────────────────────┤
│                    RECOVERY PATH                                   │
│                                                                    │
│  Worker crashes → heartbeat stops → lease expires                  │
│       → timeout loop detects → task released (pending)             │
│       → outgoing_execute → new worker acquires (preload)           │
│       → replay completed steps from cache → continue               │
│                                                                    │
└──────────────────────────────────────────────────────────────────┘
```

## Source Paths

| Flow | Key Files |
|------|-----------|
| Request handling | `resonate/src/server.rs` |
| Settlement chain | `resonate/src/persistence/persistence_sqlite.rs` |
| Timeout processing | `resonate/src/processing/processing_timeouts.rs` |
| Message delivery | `resonate/src/processing/processing_messages.rs` |
| Transport dispatch | `resonate/src/transport/mod.rs` |
| SDK execution | `resonate-sdk-rs/resonate/src/core.rs` |
| SDK replay/preload | `resonate-sdk-rs/resonate/src/effects.rs` |
