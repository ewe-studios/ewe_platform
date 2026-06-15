# Datastar Rendering & Signal Pipeline — Learnings

## Source

Datastar's rendering signals deep dive (`17-rendering-signals-deep-dive.md`) and SSE streaming exploration (`08-sse-streaming.md`).

---

## 1. The Reactive Signal System

### Core Primitives

Three primitives, all in `signals.ts`:

| Primitive | Purpose | Writable |
|-----------|---------|----------|
| **Signal** | Mutable reactive container | Yes — `signal(newValue)` returns true if changed |
| **Computed** | Derived value, lazily re-evaluates | No — getter only |
| **Effect** | Side effect, re-runs when accessed signals change | No — runs for side effects |

A signal is a function. No args → read. One arg → write.

### Dependency Graph

Doubly-linked list of `Link` nodes:

```
Link {
  dep_: ReactiveNode    // The dependency (signal/computed)
  sub_: ReactiveNode    // The subscriber (computed/effect)
  prevSub_, nextSub_    // Linked list in dep's subscriber chain
  prevDep_, nextDep_    // Linked list in sub's dependency chain
}
```

When a signal changes, `propagate()` walks the subscriber chain marking nodes as `Pending` or `Dirty`, then `notify()` queues effects for execution.

### Reactivity Flags (Bitfield)

```
None           = 000000
Mutable        = 000001  — can be written to (signals, computeds)
Watching       = 000010  — is an effect
Pending        = 100000  — a dependency might have changed (check lazily)
Dirty          = 010000  — a dependency did change (must re-evaluate)
Queued         = 1000000 — effect is in the queue, don't add again
```

**Pending vs Dirty distinction** — same concept as R3's Check vs Dirty:
- `Pending` = "a dependency *might* have changed" — check lazily during re-evaluation
- `Dirty` = "a dependency *did* change" — must re-evaluate

### Propagation Algorithm

```
signal(newValue)
  ├─ if value === oldValue → return false (skip)
  ├─ set value, mark self Mutable|Dirty
  ├─ propagate(subs) → walk subscriber chain
  │   ├─ for each subscriber:
  │   │   ├─ if no flags → mark Pending
  │   │   ├─ if Watching (effect) → notify() → queue for execution
  │   │   └─ if Mutable (computed) with subs → recurse into subs
  │   └─ uses explicit stack (not recursion) to avoid stack overflow
  └─ if not batching → flush() → drain effect queue
```

Propagation only **marks** nodes, doesn't re-evaluate anything. Effects are queued and run during `flush()`. Multiple signal changes are coalesced.

### Batching

```typescript
let batchDepth = 0
beginBatch() → batchDepth++
endBatch()   → if !--batchDepth: flush() → dispatch()
```

`mergePatch()` wraps all mutations in a batch. 50 signals updated in one SSE event cause only **one** round of effect execution.

### Deep Reactivity via Proxy

The root store is `deep({})` — a Proxy that intercepts property access:

**Get trap**: Returns `deepObj[prop]()`, which calls the signal getter → subscribes the active effect.

**Set trap**: Wraps values in signals, calls setter, dispatches patch event with path and value.

This means `root.user.name = "Alice"` automatically creates nested signal containers and subscribes effects.

---

## 2. How Attributes Become Reactive

### Attribute Scanning

```
MutationObserver fires
  → for added nodes: applyEls([node, ...descendants])
  → for removed nodes: cleanupEls([node, ...descendants])
  → for changed attributes: applyAttributePlugin(target, key, value)
```

### Expression Compilation (`genRx`)

Transforms attribute expressions into executable JavaScript:

```
"$count > 5 ? 'highlight' : ''"
  ↓
1. Signal replacement: $count → $['count']
2. Action replacement: @get('/url') → __action("get", evt, '/url')
3. Return wrapping: last statement gets return
4. Function construction: new Function('el', '$', '__action', 'evt', ..., expr)
```

Compiled once per attribute, cached. The `rx()` wrapper re-executes it whenever an effect re-runs.

### Example: `data-text="$count"`

```
1. Engine finds data-text="$count" on <span>
2. Creates ctx.rx = () => genRx("$count")(el)
3. text plugin: apply({ el, rx }) → effect(() => { el.textContent = rx() })
4. effect() runs immediately:
   - Reads root['count'] → subscribes to 'count' signal
   - Sets el.textContent
5. When count changes → effect dirty → re-runs → updates textContent
```

### Cleanup Lifecycle

```
Element removed from DOM
  → MutationObserver fires (removedNodes)
  → cleanupEls(removedNodes)
    → For each element:
      → Call every cleanup function (effect disposer, abort controllers, event listener removers)
      → Delete element from removals map
```

---

## 3. SSE Streaming Pipeline

### Why SSE over WebSockets

1. **Unidirectional** — Server pushes state; browser sends via standard HTTP requests
2. **HTTP/2 multiplexing** — Multiple SSE streams share one TCP connection
3. **Automatic reconnection** — Built into SSE spec + enhanced retry logic
4. **Simpler server** — Just write text to response. No upgrade handshake, no frame encoding
5. **Infrastructure friendly** — Works through proxies, load balancers, CDNs

### Why `fetch` over `EventSource`

Native `EventSource` only supports GET. Datastar needs POST/PUT/PATCH/DELETE with request bodies. Building SSE on `fetch` + `ReadableStream` gets full HTTP method support.

### 3-Stage Parsing Pipeline

```
ReadableStream → getBytes() → getLines() → getMessages() → onmessage
```

**Stage 1: `getBytes()`** — Reads `ReadableStream<Uint8Array>` chunk by chunk via `reader.read()`.

**Stage 2: `getLines()`** — Byte-level line parser:
- Uses raw byte values (58 = `:`, 13 = `\r`, 10 = `\n`) for performance
- Maintains buffer with `subarray` compaction (zero-copy where possible)
- Handles partial chunks — a single SSE line might span multiple `Uint8Array` chunks
- `discardTrailingNewline` flag handles `\r\n` sequence

**Stage 3: `getMessages()`** — SSE message assembler:
- Parses `field: value` lines per HTML spec
- Handles both `field: value` (with space) and `field:value` (without)
- Empty line signals end-of-message → fire `onmessage`
- `data` fields concatenate with `\n` for multi-line values

### Content-Type Routing

| Content-Type | Behavior |
|-------------|----------|
| `text/event-stream` | SSE stream parsing pipeline |
| `text/html` | Dispatch `datastar-patch-elements` with HTML |
| `application/json` | Dispatch `datastar-patch-signals` with JSON |
| `text/javascript` | Create `<script>`, append to `<head>` |

Non-SSE responses still work — a server can return plain HTML without SSE wrapping.

### Retry with Exponential Backoff

```
retryInterval = 1000ms → 2000ms → 4000ms → 8000ms → 16000ms → 30000ms (capped)
retryMaxCount = 10
```

**Retry modes**: `auto` (network errors only), `error` (4xx/5xx), `always` (long-lived SSE), `never`.

### Visibility Handling

- GET requests: `openWhenHidden = false` → abort when tab hidden, reconnect when visible
- Mutating requests: `openWhenHidden = true` → don't lose writes

### Request Cancellation

- `auto` (default): Abort previous request from same element
- `cleanup`: Abort on element removal
- `disabled`: No automatic cancellation

### `Last-Event-ID` Header

SSE `id` field is sent back as `Last-Event-ID` header on reconnection, allowing server to resume from last known event.

---

## 4. Signal Patching (Server → Browser)

```
1. Server sends: event: datastar-patch-signals, data: signals {"count":42}
2. Watcher receives and parses JSON
3. mergePatch({count: 42}, {ifMissing: false})
   - beginBatch()
   - null value → delete signal
   - POJO value → recurse into nested object
   - primitive → set on root (creates or updates signal)
   - endBatch() → flush() effects → dispatch() patch event
4. All effects subscribed to changed signals re-run
```

**RFC 7386 JSON Merge Patch semantics**:
```json
{"count": 42}           → Set count to 42
{"count": null}         → Delete count signal
{"user": {"name": "A"}} → Set user.name (preserves user.email)
{"user": null}          → Delete entire user object
```

**`onlyIfMissing` mode**: Only sets values for signals that don't already exist. Used for initializing defaults without overwriting user-modified state.

---

## 5. The Complete Reactive Cycle

```
User click → data-on:click → compiled expression → @get('/api/increment')
  → fetch with filtered signals in query param
  → server processes, returns SSE stream
  → SSE parser: getBytes → getLines → getMessages
  → datastar-patch-signals → mergePatch({count: 1})
  → beginBatch() → signal setter → propagate() → mark effects Dirty
  → endBatch() → flush() → run effects
  → data-text effect: el.textContent = "1"
  → data-class effect: el.classList.add("active")
  → dispatch DATASTAR_SIGNAL_PATCH_EVENT
```

---

## 6. Relevance to Spec-39

### What We Should Incorporate

1. **Three-primitive reactive system** — Signal (mutable), Computed (derived, lazy), Effect (side effects). This matches R3's model but adds Effects as first-class citizens.

2. **Pending vs Dirty distinction** — Same as R3's Check vs Dirty. `Pending` = "might have changed, check lazily"; `Dirty` = "did change, must re-evaluate".

3. **Batching with depth counter** — `beginBatch()` / `endBatch()` allows coalescing multiple signal changes into one effect flush cycle.

4. **Deep reactivity via Proxy** — The root store is a Proxy that auto-creates signals for new properties. We could do this in JS, but in Rust/WASM we'd need a different approach (perhaps a path-based store).

5. **Signal dispatch event** — Every signal mutation accumulates `[path, value]` tuples. When batch ends, dispatch a `CustomEvent` with the accumulated patch. This allows any code to react to signal changes.

6. **Expression compilation** — `genRx()` transforms `$signalName` expressions into executable functions. Our WASM approach is different (compiled Rust), but the concept of compiling template expressions to reactive bindings is the same.

7. **Effect queue with deduplication** — Effects are queued with a `Queued` flag to prevent duplicates. Multiple signal changes to the same effect result in one execution.

8. **SSE as the transport** — SSE is unidirectional, works through proxies/CDNs, and has built-in reconnection. Our spec-39 plans SSE for server-driven UI.

9. **Content-Type routing** — The fetch plugin inspects `Content-Type` and routes to the appropriate handler. Our Arrow batches could use a custom content type.

10. **Visibility handling** — Abort SSE when tab is hidden (for GET requests), reconnect when visible. Prevents stale connections.

11. **`onlyIfMissing` for initialization** — Server can send default values without overwriting user state. Useful for initial hydration.

12. **JSON Merge Patch (RFC 7386)** — `null` = delete, object = recursive merge, primitive = set. Clean protocol for server-to-client state updates.
