# Discussion — Spec 36 review of inline answers (2026-06-15)

This file gathers everything from your `ADD: todos` pass that **needs a conversation** (you wrote
"explain to me / lets discuss / I don't understand" or asked for web research), plus the **cross-cutting
decisions** and **new-crate proposals** your answers imply, and a proposed **machinery-first
re-numbering**. Items you answered *clearly* are listed in §F (RESOLVED — will be folded into the
features); they're here only so you can sanity-check my reading before I edit.

Legend: **[RESOLVED]** = clear directive, will apply. **[DISCUSS]** = needs your call. **[RESEARCH]** =
needs web research + a `foundation_docs` writeup before we decide.

---

## A. Cross-cutting decisions (these gate many features — settle first)

### A1. Async-first traits, `_async`-suffixed, valtron-wraps-sync — [RESOLVED, one sub-point to DISCUSS]
You said this in F06, F07, F16, F20: *"we already write async_traits, why not write an async trait with
an async method… then valtron it for the sync version"* and *"keep async traits and sync traits separate,
let async methods have `*_async` suffixes."*

**Decision (applies spec-wide):** the canonical trait is **async** with `*_async` methods; a **separate
sync trait** (plain method names) is provided as a **valtron wrapper** over the async impl. Implementers
write the async version; callers pick the trait that fits their environment.

This **reverses** several earlier OD "recommendations" that proposed sync/object-safe traits to dodge
async — specifically **F20 OD-20-1** (ToolImpl `execute`), and the framing in **F04/F06** (sync
`DocumentStore` vs `AsyncDocumentStore`). New rule: async-first everywhere, sync is the valtron shim.

**[DISCUSS] the one open piece — `Send`/`?Send` on wasm.** Async traits today are split `Send` (native)
vs `?Send` (wasm/CF, because JS Promises aren't `Send`). You wrote (F06, F07 OD-07-5): *"just add the
`Send` trait in wasm to satisfy send warnings, it doesn't matter in single-threaded situations, but we may
need care in wasm targets that support multi-threading — let's review and talk."*
- **My take:** on `wasm32-unknown-unknown`/CF Workers everything is single-threaded, so a `Send` bound is
  *vacuously satisfiable* — we can keep **one** `Send` async trait and, on wasm, wrap the `!Send` JS
  futures so they present as `Send` (sound because there's no real concurrency). On
  `wasm32-unknown-emscripten` (threads) we'd need genuine care.
- **Proposal:** one `Send` async-trait surface; a wasm adapter that makes JS futures `Send` on
  single-threaded targets; revisit only for emscripten-threads. **Your call** — this removes the whole
  `?Send` mirror (and kills F07 OD-07-5, simplifies F06 OD-06-11).

### A2. KV is **only** the Memory cache — never a DocumentStore — [RESOLVED]
F06: *"KvDocumentStore should not exist, use it as our Memory cache, that idea is useless"*; F07: KV is
*"a great way to cache the Memories for a sessionId… does not need any scan capability, just keys."*

**Decision:** delete `KvDocumentStore` (F06). `DocumentStore` backends = SQL, D1, R2, VFS/Fjall. **KV is
the backing for the `MemoryStore`** latest-snapshot cache (point get/put by `session_id`, no scan). This
cleanly resolves all the "KV can't honor scan_from ordering" gymnastics in F06.

### A3. MemoryStore redesign — store `SessionRecord` directly, don't wrap DocumentStore — [RESOLVED + 1 DISCUSS]
F07: *"should not what we store just be the `SessionRecord`?"*, *"why does MemoryStore need to wrap a
DocumentStore… whatever owns those two should own the drop-down to DocumentStore, not MemoryStore
itself,"* *"why does MemoryStore use `SessionId` — which are just scru128 ids?"*

**Decision:**
1. **MemoryStore stores `SessionRecord` values directly** (the memory variants), keyed by id — **no
   separate `*Snapshot` structs**, no `MemoryBundle` factoring. This dissolves F07 OD-07-1 entirely
   (the serde-flatten hole disappears — we store the record we already have).
2. **MemoryStore does NOT wrap DocumentStore.** They're siblings. A higher-level coordinator (the session
   / context layer) owns *both* and decides when to fall back from the cache to the audit log. MemoryStore
   stays a dumb, fast latest-pointer cache.
3. **Key by the raw scru128 id type**, not a `SessionId` newtype, since `SessionId` *is* a scru128
   (`foundation_compact::Id`). (Minor: confirm we want the bare id everywhere vs the newtype for
   type-safety — **[DISCUSS]** if you care; I lean bare id per your note.)

**[DISCUSS]:** who is the "coordinator that owns both"? Likely the `AgentSession` (F31) or a small
`SessionStore` facade. I'll propose `AgentSession` owns `{ MessageApi(DocumentStore), MemoryStore }` and
does the fallback. Confirm.

### A4. Models/providers already expose usage stats — read them, don't reinvent — [RESOLVED]
F02/F03, repeatedly: *"if the models already expose usage and stats method, what's stopping us from
getting this info every turn?"*

**Confirmed in code:** the `Model` trait has **`fn costing(&self) -> GenerationResult<UsageReport>`**
(`types/mod.rs:1408`), implemented by every provider (anthropic/openai/openai_responses/llamacpp/candle),
backed by a cumulative `CostAccumulator` (`costing.rs:98`). So:
- **tokens_so_far / cumulative usage** come from `Model::costing()` — call it any turn. F03's ledger
  *aggregates across models*, it doesn't re-count (already resolved as OD-03-10).
- **per-turn delta** = the `Assistant` message's `UsageReport`.
- This resolves the *source* question in **OD-02-2, OD-02-4, OD-03-9**. The only thing `costing()` can't
  give is **true mid-generation partial** token counts (see B/§D3 "Generating vs Generated" discuss item).

### A5. `foundation_compact` owns ALL rand for every platform — [RESOLVED]
F00 OD-00-5: *"the whole capability — let's own everything once and for all."* F00b OD-00b-4: *"all rand
usage comes from foundation_compact; we present the right rand for native / wasm / wasm+wasi /
wasi+emscripten — that's why foundation_compact exists."*

**Decision:** vendor the **whole** `rand` (+ `getrandom`), not a minimal subset; **all** `rand`/
`rand_chacha`/`fastrand` use across `foundation_ai` (and elsewhere) routes through `foundation_compact`,
which selects the right backend per target. Remove the dead `chrono` dep (OD-00b-5). License headers +
`VENDORED.md` (OD-00-7).

---

## B. New-crate proposals (you asked me to think and tell you)

### B1. `foundation_http` — a fetch-based HTTP client for native + wasm — [DISCUSS, lean YES]
F00c: *"add fetch-based clients… a design that works for native and wasm… even the HTTP API client has
`Send()` and methods we can represent with fetch… come up with a design that feels right."*

**Why:** today built-in remote providers (anthropic/openai) are native-only because the HTTP client isn't
wasm-capable. A unified client unlocks **real providers on wasm/CF** and removes the "machinery-only on
wasm" caveat (F00c OD-00c-1/3).

**Proposed shape:** a single `HttpClient` trait with **one async surface**, two feature-gated backends:
- native → `reqwest`/our `foundation_netio` transport;
- wasm → `web-sys`/`fetch` (and a `foundation_wasm` host variant, as you suggested), incl. streaming
  responses (SSE) via `ReadableStream`.

`foundation_http` owns the trait + both backends; providers depend on it instead of a native client. SSE
streaming maps to fetch's `ReadableStream`. **[DISCUSS]:** crate name (`foundation_http` vs fold into
`foundation_netio`), and whether the first cut is request/response only with SSE as a fast-follow.

### B2. `foundation_wasmtime` — wasmtime host wrapper — [RESOLVED direction, scope to DISCUSS]
F00d: *"wasmtime always — ignore everything else; a `foundation_wasmtime` should own this layer and give a
nice API to set the imports and get an executable that exposes the exports… a lot of value here even for
future work."*

**Decision:** new `foundation_wasmtime` crate wrapping `wasmtime`: build/instantiate a `.wasm`, wire host
imports, expose typed exports; used by the WASI harness in F00d (replaces the wasmtime-vs-wasmer OD-00d-3).
**[DISCUSS]:** minimum API for *now* (load + run + assert exports for the testbed) vs the fuller
imports/exports host you hinted at for future work. I'd build the minimal testbed-driving API first.

### B3. Build tooling — `foundation_buildtools` vs fold into `foundation_testbed` — [DISCUSS]
F00b: *"invest in getting the build platform right… it's ok to move a lot of `build.rs` logic into
`foundation_testbed`, or create a `foundation_buildtools` to own such concerns — think about it and tell
me."*

**My recommendation: a dedicated `foundation_buildtools`.** Reason: `foundation_testbed` is a *test
harness* (pulled in as a dev-dependency / runner); build-time concerns (EMSDK wiring, target detection,
`build.rs` helpers, codegen) are a *different lifecycle* and many crates need them at build time without
pulling a whole test harness. A small `foundation_buildtools` that crates use in `build.rs` (and which the
testbed can also use) keeps the dependency direction clean. **Your call.**

### B4. `foundation_docs` — zero-to-hero teaching docs — [RESOLVED direction, scope to DISCUSS]
F08–F11, F14, repeatedly: *"I've reached the limits of my knowledge — web research, select the best
answers, add `foundation_docs` to teach me from zero to hero."*

**Decision:** a `foundation_docs` home for the deep "fundamentals" writeups (vector search & ANN
algorithms, BM25/RRF, code-graphs, embeddings, LSM/fjall, arrow, wasm targets, etc.). The per-feature
"Fundamentals Documentation (zero-to-expert)" sections become **chapters in `foundation_docs`** rather
than scattered. **[DISCUSS]:** is `foundation_docs` a crate of markdown (mdBook-style) or doc-comment
modules? I lean an mdBook-style markdown crate, web-published, with runnable examples where possible.

---

## C. The vectors / RAG group is RESEARCH-gated → defer behind core machinery
F08–F11, F14 are dominated by *"I've reached the limits of my knowledge — research the web, pick the best,
teach me via foundation_docs."* These are genuinely **enhancement** work and need a research pass each:

- **[RESEARCH] F08:** SIMD (what blocks it: portable_simd is nightly; `wide`/`std::simd` options), the
  **no_std `sqrt`** problem (yes — the "DOOM trick" is the *fast inverse square root*; for us the clean
  no_std answers are `libm::sqrtf`, or rank on squared-distance and store normalized vectors so cosine==dot
  and no sqrt is needed at query time), cosine zero-vector policy, owned-vs-borrowed iterator (F14 path).
- **[RESEARCH] F09:** IVF/HNSW parameters & wasm viability.
- **[RESEARCH] F10:** BM25 + RRF fusion specifics.
- **[RESEARCH] F11:** code-graph — **[RESOLVED] focus Rust first** (your call), JS/TS/Python as a later
  feature, others deferred; **[RESOLVED]** prefer a wasm-safe clustering crate else own it;
  **[RESOLVED]** it's fine if graph-build is native-only and wasm just loads a prebuilt `graph.json`.
- **[RESEARCH] F14:** **[RESOLVED]** if CF has a vector API use it, else skip — don't build one; don't
  waste effort.

**Recommendation:** move the whole vectors/embeddings/RAG/search-context stack to **Phase 3** of the
roadmap (§E) and pair it with the `foundation_docs` research chapters. Two cross-cutting [DISCUSS] items
from here that aren't purely research:
- **F08 "move it all into `foundation_vectors`":** yes — `VectorStore` (currently F12 in `foundation_db`)
  and the `VectorMatch`/`DistanceMetric` types **move fully into `foundation_vectors`**; `foundation_db`
  re-exports if needed. Confirms OD-08-8. **[RESOLVED]**
- **F08 "why rayon, not valtron?":** **[DISCUSS]** — rayon is a data-parallel work-stealing pool for
  CPU-bound fan-out (brute-force scan). valtron is our cooperative task executor. We *can* express
  parallel scan as valtron `broadcast` tasks and drop the rayon dep (keeps wasm clean, one concurrency
  model). Cost: valtron isn't a CPU-saturating thread pool the way rayon is, so very large native scans
  may be slower. **Proposal:** default to a plain sequential scan (wasm-safe, no dep); offer an *optional*
  native parallel path built on valtron `broadcast` rather than rayon. Confirm.

---

## D. Per-feature DISCUSS items (my explanation + recommendation for each)

### D1. F00b OD-00b-2 — *"I don't understand this, ask me with clarity."*
The OD asked: is the wasm build surface just `--no-default-features` (which drops llamacpp), and is the
`agentic` cargo feature merely a *marker* until 00c decides what it must gate? **Plain-English question
for you:** do you want a dedicated **`agentic` feature flag** that turns the agentic API on/off
independently of providers, or should the agentic API always be compiled in (no flag)? My rec: keep an
`agentic` feature so a consumer can depend on `foundation_ai` purely for models without the agentic layer.

### D2. F02 OD-02-7 — *"don't understand, explain."* (forward-reference / soft dependency)
F02 uses the error type `AgenticError`, but that type is *defined* in F30 (error handling), which is built
later. A **forward-reference** means F02 names a type that doesn't exist yet; until F30 lands, F02 compiles
against a **stub** `AgenticError` with the required trait bounds, and F30 later fills it in. "Soft
dependency" = F02 can be built first as long as the stub exists. Nothing to decide — it's a build-ordering
note. (The machinery-first re-numbering will put F30 early precisely to avoid stubs.)

### D3. F02/F03 — "Generating vs Generated" and "why no tokens_so_far?" — [DISCUSS]
`ModelState::GeneratingTokens(Option<UsageReport>)` is the *in-progress* state; providers currently emit
`None` (no partial usage mid-stream). Your question: *"why can't they provide tokens-so-far — models tell
us this, not providers?"* Reality: the **cumulative** number is always available via `Model::costing()`
(A4). The **mid-stream partial** count would require the provider to populate `Some(usage)` on each delta,
which the streaming providers don't do today. **Proposal:** use `costing()` for budget/accounting (exact,
per turn); treat live mid-stream `tokens_so_far` as an optional provider enhancement (populate
`Some(usage)` from the provider's own counter). Rename the state to make sense if you prefer. Confirm
whether mid-stream live counts matter to you now, or are a later nicety.

### D4. F03 OD-03-3 — what counts toward the "rolling" memory-trigger count? — [DISCUSS/RESEARCH]
The rolling counter decides when to summarize context (observation ~30k). Question is whether it counts
**input+output** or **output only**. *What other harnesses do:* context-window managers (e.g. Mastra,
Letta/MemGPT) measure **total context size = input+output of recent turns**, because the trigger is "the
prompt is getting too big." **Rec (matches your instinct):** input+output of recent turns. I'll verify
against F18's context-assembly definition so the number means the same thing in both places.

### D5. F03 OD-03-5 — *"why is `total_tokens` inconsistent, for my learning."*
Providers compute the `total_tokens` field differently: **Anthropic** reports `input + output` and lists
cache read/write **separately** (cache isn't in the total); **OpenAI** reports a `total_tokens` that
**includes** cached tokens. So summing the providers' own `total_tokens` across a multi-provider session
double-counts/under-counts depending on provider. That's why our ledger computes its **own** total from
the four explicit buckets (`input+output+cache_read+cache_write`) — consistent regardless of provider.
(Learning note; no decision needed.)

### D6. F07 — the cluster of "explain to me" ODs (07-5/6/7/8/9)
All of these collapse once we adopt A3 (store `SessionRecord` directly, MemoryStore doesn't wrap
DocumentStore) and A1 (async-first, drop the `?Send` mirror):
- **OD-07-5 (`?Send` "why do we need this")** → gone under A1.
- **OD-07-6 (version/CAS)** → the snapshot had a `version` field; "CAS" = compare-and-swap (only overwrite
  if version matches, to catch two writers racing). Under single-agent-per-session it's unnecessary →
  last-writer-wins. Keeping it on the page only to explain the term. Confirm: **no CAS**.
- **OD-07-7 (fallback addressing)** → "how do we find the latest reflection in the audit log if the cache
  misses?" → the coordinator (A3) scans the DocumentStore filtered by `record_type` (F04 promoted column).
  Not MemoryStore's job anymore.
- **OD-07-8/9** → resolved by A3 (store the record; `&`-taking API).

### D7. F16 OD-16-1/2/3 — pub/sub + backpressure + WAL — [RESOLVED, one DISCUSS]
- **OD-16-2 [RESOLVED]:** bounded inner queue; if it fills and nobody's consuming, **panic and report
  fast** (something is broken). Your call.
- **OD-16-3 [RESOLVED]:** **WAL it** (you pointed at `cacache`) — crash-safe + makes backpressure easier.
  I'll spec a `cacache`-backed write-ahead log for the message buffer.
- **OD-16-1 [DISCUSS]:** *"ConcurrentQueueOfReceivers — isn't that better?"* than a Mutex broadcaster. For
  fan-out pub/sub, a concurrent queue of receivers avoids a central lock and is lock-free on the hot path
  — **agreed, it's better** for the broadcast case; I'll spec the receiver-queue. Confirm.

### D8. F18 OD-18-5 — ContextProvider vs MemoryHierarchy boundary — [DISCUSS]
*"Explain the difference so I know the best division."* **MemoryHierarchy (F19)** *generates and owns* the
memory tiers (it runs the memory model, writes working/observation/reflection). **ContextProvider (F18)**
*consumes* those outputs and **assembles the prompt** for the next turn (packs system + memory + recent
messages + retrieved context within the token budget) and *triggers* F19 when thresholds are hit. Split:
F19 = "make and store memories," F18 = "pick what goes into this prompt." **Rec:** keep them separate with
that boundary. Confirm.

### D9. F08 OD-08-9 — borrowed vs streaming iterator — [DISCUSS, vectors/Phase 3]
`flat_top_k` takes a `(&str, &[f32])` iterator — fine for in-memory stores, but the D1/KV/R2 fetch path
(F14) deserializes vectors on the fly and can't lend `&[f32]`. **Rec:** add an owned/streaming variant of
`flat_top_k` when F14 needs it; keep the borrowed one for in-memory. (Detail for Phase 3.)

---

## E. Items you answered clearly — RESOLVED, will be folded into the features
(Listed so you can catch any misread. No action needed unless you object.)

- **F00:** vendor whole rand (A5); both foundation_wasm-host AND js-sys crypto, feature-gated; vendor
  licenses.
- **F00b:** generalize provider errors → one held+gated variant per provider; all rand via
  foundation_compact; remove chrono.
- **F00c:** verify/fix `foundation_auth` on wasm (own sub-feature if needed); gguf provider gated behind
  llamacpp.
- **F00d:** wasmtime always (→ B2); two features for wasip1 vs wasip2 (review scope); gate native+emscripten
  where it works; do the spec-wide target_os cfg refactor, this feature owns the canonical pattern.
- **F02:** `D = SessionRecord` (already applied); usage from models (A4).
- **F03:** round f64→u64; **persist the ledger as a new `SessionRecord` type via the Message API + a cheap
  cache** (your OD-03-4 answer); budget basis = 4 buckets.
- **F11:** Rust-first, JS/TS/Python later; wasm-safe clustering crate or own; prebuilt graph.json on wasm ok.
- **F15:** separate **EmbeddingProvider router** API layer; memory backend for tests **and** test fjall;
  **do sentence-level chunking** (rust crate; whole-text fallback) — not deferred.
- **F16:** embed user/assistant text + observation/reflection, skip raw blobs.
- **F17:** **arrow-rs split crates** (confirmed, no arrow2); I choose the promoted column set sensibly;
  JSON-string content column.
- **F18:** wasm `search_file` uses VFS filesystem ops if available, else unsupported.
- **F19:** memory-model extraction is seamless — model marks explicit working-memory items + explicit user
  requests; observation/reflection generation can **broadcast to background workers** (non-blocking);
  reflection-failure keeps prior memory + emits error + retries.
- **F20:** **async `execute_async`** (A1, reverses sync rec); consolidate `ToolDefinition`→`Tool` if no
  value lost; `Arc<RwLock<ToolManagerInner>>` interior mutability; **`Option<ToolShed>`** at session
  level; **duplicate registration = error** (so no re-embed needed).
- **F21:** `Option<ToolShed>`, but when present the mandatory tools are **always** filled (defaults);
  tools work on wasm (impl-aware); **full schema, compact/no-pretty-print**; no re-register ⇒ no re-embed;
  category = **enum with `Custom(String)`**.
- **F22:** use the **`fff-search`** crate; rename `search` → **`search_context`** (context/memories, not
  files); `search_file` stays a shed-discoverable tool (not a struct field); prebuilt-graph search;
  `search_file` impl-based (wasm may use in-memory/VFS search).
- **F04/F05/F06:** delete `KvDocumentStore` (A2); single-writer detail to spell out (F05 — see below);
  CF runs migrations always (you pointed at `examples/cf-login-app`, `examples/cf-valtron-counter`);
  expand D1 binding to batch; **migrate wasm-bindgen CF code onto worker-rs types**; R2 threshold = a
  sqlite page size (small), above → R2; miniflare+wrangler for tests.

**F05 single-writer — you asked for depth (OD-05-6):** *"usually a Mutex enforces a single writer; if a
Mutex is used, an RwLock supports multiple readers + 1 writer — make it clear exactly what we implement
and how."* I'll spec it concretely: the store holds `Arc<RwLock<WriterState>>` where `WriterState` owns the
file high-water-mark + the in-flight batch; **writes take the write lock** (serialized → the "single
writer"), **reads take the read lock** (concurrent). The offset is computed under the write lock
(`file_hwm + batch_bytes`) and returned. This is the "RwLock = many readers + one writer" model you
described, made explicit in the feature.

---

## F. Proposed machinery-first re-numbering (for confirmation BEFORE I rename anything)

Your goal: nail the **core mechanics** first — agentic loop, tasks, memory, message API, tool calls + the
tool-call manager, execution — each with **in-memory implementations of the traits** and **full
`{crate}/tests` coverage** (logical sub-dirs per scope). Only after the core is proven do we add the
enhancements (real storage backends, vectors/RAG, embeddings, external/CF). **00-series stays.**

Renaming 35 feature dirs also means rewriting every `Fxx` cross-reference, so I want your nod on the
*ordering* before touching the filesystem. Proposed phases (numbers are the new prefixes):

**Phase 0 — Foundation fixes (unchanged):** `00` compact · `00b` llama-optional · `00c` wasm-providers ·
`00d` wasm-target-matrix.

**Phase 1 — Core machinery (in-memory impls + full tests):**
1. message model & types (was F01)
2. error handling / `AgenticError` (was F30 — pulled early so nothing stubs it)
3. agent stream contract (was F02)
4. token accounting & budget (was F03)
5. serialization JSON/Arrow (was F17 — needed to persist records)
6. DocumentStore trait + **in-memory** backend (subset of F04; SQL stays but in-memory is the Phase-1 impl)
7. MemoryStore (was F07, redesigned per A3 — in-memory/KV)
8. Message API (was F16)
9. ToolImpl + registry (was F20)
10. ToolShed + shed meta-tool (was F21)
11. ToolCall execution DAG / manager (was F23)
12. Model provider router (was F24)
13. steering queues / depends (was F25)
14. input/output processors (was F26)
15. memory hierarchy (was F19)
16. context provider & assembly (was F18)
17. loop detection (was F28)
18. access control & budget enforcement (was F29)
19. **agentic loop** (was F27)
20. agent session API (was F31)
21. testing strategy / coverage (was F32)

**Phase 2 — Real storage backends:** DocumentStore SQL hardening · VFS/Fjall (was F05) · Cloudflare D1+R2
(was F06, KV dropped).

**Phase 3 — Retrieval / RAG enhancements (research-gated, paired with `foundation_docs`):**
foundation_vectors core/IVF-HNSW/BM25/code-graph (was F08–F11) · VectorStore trait/native/CF (was F12–F14)
· embedding provider (was F15) · search tools / `search_context`+`search_file` (was F22).

**Phase 4 — Platform investments (new):** `foundation_http` fetch client (B1) · `foundation_wasmtime` (B2)
· `foundation_buildtools` (B3) · `foundation_docs` (B4).

**Open question for you:** are Phases 1–4 the right buckets, and is the Phase-1 *order* right (it's roughly
dependency order, ending at the loop that ties everything together)? Once you confirm, I'll renumber the
dirs and fix all cross-references in one pass.

---

## G. Suggested next steps
1. You react to **§A (cross-cutting)** and **§B (new crates)** — those unlock the most.
2. I fold all **§E (resolved)** answers into the features + update the affected decision docs (16, 18, 13,
   06, 07, 02, 03, 20, 21, 22, 15).
3. You confirm the **§F re-numbering**; I execute the renames + cross-ref fixes.
4. We work the **§C/§D research items** (web research + `foundation_docs` chapters) as their phase comes up.
