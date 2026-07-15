// 
// # Rust Valtron Usage
// 
// ## Core Philosophy
// 
// Valtron is a progress-driven execution engine. Operations produce `TaskIterator`s or `StreamIterator`s that yield incremental results. The execution engine drives progress — **callers decide when and where to synchronize**.
// 
// **The fundamental rule:** Do not turn async operations into sync operations at the leaf. Instead, schedule work via `execute()`, return the stream to the caller, and let them decide when to collect results. This preserves composability, parallelism, and progress observability.
// 
// Blocking should happen at **boundaries** — the outermost point where a concrete value is actually needed — not inside every individual operation.
// 
// ### When Blocking Internally IS Acceptable
// 
// **Exception to the fundamental rule:** For single-value operations where the result is required immediately for subsequent operations, blocking internally is acceptable:
// 
// - **Authentication checks** — `auth_check()`, `whoami()` — need to know before proceeding
// - **Single-item lookups** — getting user/org info needed for subsequent requests
// - **CRUD operations** — `create_repo()`, `delete_repo()` — caller needs the result immediately
// 
// **Pattern for single-value blocking:**
// 
// ```rust
// use foundation_core::valtron::{from_future, execute, collect_one};
// 
// pub fn whoami(&self) -> Result<User> {
//     let future = from_future(async move {
//         // ... async work ...
//         Ok::<_, Error>(user)
//     });
//     
//     let stream = execute(future, None)?;
//     collect_one(stream).ok_or_else(|| Error::NoResult)
// }
// ```
// 
// **Pattern for multi-value streaming:**
// 
// ```rust
// pub fn list_models(&self) -> Result<impl StreamIterator<D = Result<ModelInfo>, P = ()> + Send> {
//     let future = from_future(async move {
//         // ... async work ...
//         Ok::<_, Error>(vec_of_models)
//     });
//     
//     let stream = execute(future, None)?;
//     Ok(stream.flat_map_next(|result| {
//         // Expand Vec into stream items
//     }))
// }
// ```
// 
// **Decision flowchart:**
// 
// 1. **Does the operation return multiple values?** → Return a stream
// 2. **Is the result needed immediately for subsequent operations?** → Blocking internally is OK
// 3. **Could the caller benefit from composing this with other operations?** → Return a stream
// 4. **Is this a one-shot initialization or CLI tool?** → Blocking is acceptable
// 
// **Default:** Return streams. Block internally only when there's a clear justification.
// 
// ## Code Style: Clear, Simple, Succinct
// 
// **Async first then sync via valtron calling async** 
// 
// Where ever possible, implementation is always in the async functions and then sync call valtron to run async code and return result, unless due to technical issues or unnecessary complexity should we clone and duplicate code for sync but this ok but rare where the complexity of calling the async via valtron is not worth it and reimplementing the logic for sync when the api already support syncs makes the most sense. This will probably mean two traits one for async and one for sync with the async one having methods ending with `*_async` to avoid conflict with the sync ones. 
// 
// *NOT A RULE BUT SOMETHING TO CONSIDER*: If it makes sense to reduce duplication and one trait can have async methods that others implement with sync methods with default implementation great, or if duplication is causing bloat, we can just define a trait with both async and sync methods but i doubt rust like this. So use your judgement here.
// 
// **IMPORTANT**: In the desire to support sync and async versions, we may increase code size but i
// think its still worth it where sensible to ensure we can use these in sync and
// unsync context but if not we default to async versions and can provide wrappers that
// use valtron to call the async in a way that works for the users, meaning all things
// by default are async and just let the user know if its not possible and they can decide to call async code however they need in their different contexts.
// 
// **IMPORTANT**: Yes, remember there are caveats, this is why we need to be detailed, maybe sync cant wrap async due to issues like &mut self, we need to clearly articulate the design in the feature to clearly know its possible to do that.
// 
// **Prefer direct, minimal combinator chains.** Each combinator should have a clear purpose:
// 
// ```rust
// // GOOD: Only transform what needs transforming
// let task = task
//     .map_ready(|v| v * 2)      // transforming Ready
//     .filter_ready(|v| v > 10); // filtering Ready
// 
// // BAD: Unnecessary map_pending that does nothing
// let task = task
//     .map_pending(|p| p)        // useless - remove
//     .map_ready(|v| v * 2);
// 
// // GOOD: Use specific combinators for clarity
// let stream = stream.filter_done(|v| v.is_ok());
// 
// // BAD: Over-engineered with unnecessary maps
// let stream = stream
//     .map_done(|v| v)           // useless identity map
//     .map_pending(|p| p)        // useless identity map
//     .filter_done(|v| v.is_ok());
// ```
// 
// **Rules of thumb:**
// - Don't add `map_pending` unless you actually need to transform Pending values
// - Don't chain multiple maps when one would do
// - Use the most specific combinator (`filter_done` vs `map_done` + conditional)
// - If a combinator doesn't change behavior, remove it
// 
// ## Combinator Usage: Prefer StreamIteratorExt Over Raw Iterator
// 
// **Never manually match all `Stream` variants** — use `StreamIteratorExt` combinators that handle pass-through automatically.
// 
// ### Anti-Pattern: Manual Stream Matching
// 
// ```rust
// // BAD: Verbose, error-prone, manually handles all Stream variants
// let mapped = raw_stream.filter_map(|s| match s {
//     Stream::Next(Some(json_str)) => match serde_json::from_str::<V>(&json_str) {
//         Ok(v) => Some(Stream::Next(Some(v))),
//         Err(e) => {
//             tracing::error!("Deserialization error: {e}");
//             None
//         }
//     },
//     Stream::Next(None) => Some(Stream::Next(None)),
//     Stream::Pending(p) => Some(Stream::Pending(p)),      // boilerplate
//     Stream::Init => Some(Stream::Init),                   // boilerplate
//     Stream::Ignore => Some(Stream::Ignore),               // boilerplate
//     Stream::Delayed(d) => Some(Stream::Delayed(d)),       // boilerplate
// });
// ```
// 
// **Problems:**
// - 15+ lines of boilerplate just to transform `Next`
// - Must manually update if `Stream` adds new variants
// - Hard to read — logic buried in match arms
// 
// ### Correct: Use `map_done` Combinator
// 
// ```rust
// // GOOD: map_done automatically passes through Pending, Init, Ignore, Delayed
// let mapped = raw_stream.map_done(|opt_json| {
//     opt_json.and_then(|json_str| {
//         serde_json::from_str::<V>(&json_str)
//             .map_err(|e| tracing::error!("Deserialization error: {e}"))
//             .ok()
//     })
// });
// ```
// 
// **Benefits:**
// - 5 lines instead of 15+
// - Future-proof — new Stream variants handled automatically
// - Clear intent — "transform Next values, pass through rest"
// 
// ### When `map_pending` IS Necessary
// 
// Only use `map_pending` when you need to **change the Pending type**:
// 
// ```rust
// // GOOD: Converting Pending type for type compatibility
// let stream = raw_stream
//     .map_done(|v| v * 2)
//     .map_pending(|p| MyPending::from(p));  // Type conversion needed
// 
// // BAD: Unnecessary map_pending — Pending type doesn't matter
// let stream = raw_stream
//     .map_done(|v| v * 2)
//     .map_pending(|_| ());  // Useless unless type requires it
// ```
// 
// **Key insight:** At collection boundaries (`find_map`, `collect_result`), only `Stream::Next` values are extracted. Pending is progress information that gets discarded. Don't transform it unless the type signature requires it.
// 
// ### Common Transformations
// 
// | Goal | Use |
// |------|-----|
// | Transform `Next` values | `map_done(f)` |
// | Filter `Next` values | `filter_done(f)` |
// | Transform `Pending` values (type change) | `map_pending(f)` |
// | Transform both with one function | `map_pending_and_done(f)` |
// | Deserialize/parse `Next` | `map_done` + `and_then` |
// | Stop on error, preserve it | `map_circuit` |
// 
// ```rust
// // Deserialize JSON in Next values
// let stream = stream.map_done(|json_opt| {
//     json_opt.and_then(|s| serde_json::from_str(&s).ok())
// });
// 
// // Convert Result to Option, log errors
// let stream = stream.map_done(|result| {
//     result.map_err(|e| tracing::error!("Error: {e}")).ok()
// });
// 
// // Extract field from Next value
// let stream = stream.map_done(|user| user.name);
// ```
// 
// ## The Execution Pipeline
// 
// ```
// create future/task
//     → apply TaskIterator combinators (map_ready, map_pending, filter_ready, etc.)
//     → execute() — schedules on thread pool, returns StreamIterator
//     → apply StreamIterator combinators (map_done, filter_done, collect, etc.)
//     → return stream to caller
//     → caller composes multiple streams
//     → caller collects at boundary (Iterator::find_map, sync_one, sync_all, etc.)
// ```
// 
// Every step is optional. The key insight: **separating launch from collection** enables parallelism even with heterogeneous types.
// 
// ## Pattern 1: Methods Return Streams (The Default)
// 
// Methods that perform I/O should schedule the work and return the stream. The caller controls when to collect.
// 
// ```rust
// use foundation_core::valtron::{execute, from_future, Stream};
// 
// fn get<V: DeserializeOwned + Send + 'static>(
//     &self, key: &str,
// ) -> StorageResult<impl Iterator<Item = Stream<Option<V>, ()>>> {
//     let key = key.to_string();
//     let conn = Arc::clone(&self.conn);
// 
//     let task = from_future(async move {
//         let mut stmt = conn.prepare("SELECT value FROM kv_store WHERE key = ?").await?;
//         let mut rows = stmt.query([key]).await?;
//         match rows.next().await? {
//             Some(row) => {
//                 let value: String = row.get(0)?;
//                 let deserialized: V = serde_json::from_str(&value)?;
//                 Ok::<_, BackendError>(Some(deserialized))
//             }
//             None => Ok(None),
//         }
//     });
// 
//     // Schedule the work — returns immediately, work runs on pool
//     let stream = execute(task, None)
//         .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
//     Ok(stream)
// }
// ```
// 
// **Why `StorageResult<impl Iterator<...>>`:** `execute()` returns a `Result` indicating whether the task was successfully scheduled. The `StorageResult` wraps that scheduling result. The stream itself carries the async operation's outcome as `Stream::Next(value)`.
// 
// ### Caller Composes and Collects at Boundary
// 
// ```rust
// // Launch two independent lookups — both start executing immediately
// let user_stream = db.get::<User>("users:alice")?;
// let config_stream = db.get::<Config>("app:config")?;
// 
// // Both are running in parallel on the thread pool.
// // Collect at the boundary — second may already be done.
// let user_results = collect_result(user_stream);   // Vec<Option<User>>
// let config_results = collect_result(config_stream); // Vec<Option<Config>>
// ```
// 
// ## Pattern 2: TaskIterator Composition Before Execute (Parallel Homogeneous Tasks)
// 
// When multiple tasks have the same output type, compose them with `execute_collect_all` for parallel execution with synchronized collection.
// 
// **Reference implementation:** `bin/platform/src/gen_model_descriptors/mod.rs`
// 
// ```rust
// // Each task is a Box<dyn TaskIterator<Ready=Vec<ModelEntry>, Pending=FetchPending, ...>>
// let models_dev_task = create_fetch_task(&mut client, "models.dev", URL_A, parse_a)?;
// let openrouter_task = create_fetch_task(&mut client, "openrouter", URL_B, parse_b)?;
// let ai_gateway_task = create_fetch_task(&mut client, "ai-gateway", URL_C, parse_c)?;
// 
// // All three run in parallel — synchronize when all complete
// let result_stream = valtron::execute_collect_all(
//     vec![models_dev_task, openrouter_task, ai_gateway_task],
//     None,
// ).expect("scheduling succeeded");
// 
// // Collect at boundary
// for item in result_stream {
//     if let Stream::Next(models) = item {
//         all_models.extend(models.into_iter().flatten());
//     }
// }
// ```
// 
// ### Pre-Execute Combinators Shape the Task
// 
// Apply `TaskIteratorExt` combinators **before** `execute()` to transform the task's output:
// 
// ```rust
// let task = SendRequestTask::new(request, 5, pool, config)
//     // Transform Ready: HttpResponse → Vec<ModelEntry>
//     .map_ready(move |intro| match intro {
//         RequestIntro::Success { stream, .. } => {
//             let body = body_reader::collect_string(stream);
//             parser(&body, source)
//         }
//         RequestIntro::Failed(e) => Vec::new(),
//     })
//     // Transform Pending: HttpPending → FetchPending
//     .map_pending(move |p| FetchPending::from_http(p, source));
// ```
// 
// ## Pattern 3: Heterogeneous Parallel Execution
// 
// When tasks return different types, launch each individually — they still run in parallel:
// 
// ```rust
// // Launch all three — work begins immediately for each
// let user_stream = execute(user_task, None)?;
// let session_stream = execute(session_task, None)?;
// let oauth_stream = execute(oauth_task, None)?;
// 
// // Collect at boundary — by the time we finish the first,
// // the others may already be done
// let user = collect_result(user_stream);       // Vec<Option<User>>
// let session = collect_result(session_stream); // Vec<Option<Session>>
// let oauth = collect_result(oauth_stream);     // Vec<Option<OAuthState>>
// ```
// 
// ## Pattern 4: Rich Pending Types (Per-Method Judgment)
// 
// The `Pending` type in `Stream<D, P>` carries progress information. Whether to use a rich type or `()` is a **per-method judgment call** based on whether the caller can do something useful with the progress state.
// 
// **Simple get — no meaningful progress, use `()`:**
// ```rust
// fn get(&self, key: &str) -> StorageResult<impl Iterator<Item = Stream<Option<V>, ()>>>
// ```
// 
// **Migration — caller wants progress:**
// ```rust
// #[derive(Debug, Clone)]
// pub enum MigrationProgress {
//     Applying { current: usize, total: usize, name: String },
//     Verifying { migration: String },
// }
// 
// fn migrate(&self) -> StorageResult<impl Iterator<Item = Stream<MigrationResult, MigrationProgress>>>
// ```
// 
// **HTTP fetch — caller wants connection state:**
// ```rust
// #[derive(Debug, Clone)]
// pub enum FetchPending {
//     Connecting { source: &'static str },
//     AwaitingResponse { source: &'static str },
// }
// 
// fn fetch(&self, url: &str) -> Result<impl Iterator<Item = Stream<Response, FetchPending>>>
// ```
// 
// **Guideline:** Use `()` unless the operation is long-running or multi-step and the caller would benefit from observability. Don't force richness where it adds no value.
// 
// ## Sync Boundary Helpers (New Valtron Primitives)
// 
// These are the tools for collecting at boundaries. They are the **only** place where blocking should occur.
// 
// ### `collect_result` — Drain a Stream and Collect All Results
// 
// The primary boundary helper. Drains the entire stream, collecting every `Stream::Next` value. Blocks the calling thread until the stream is exhausted.
// 
// ```rust
// /// Blocks until stream is exhausted. Collects ALL `Stream::Next` values.
// /// Works for single-value streams (Vec will have one item) and multi-value streams alike.
// pub fn collect_result<D, P>(stream: impl Iterator<Item = Stream<D, P>>) -> Vec<D> {
//     stream
//         .filter_map(|s| match s {
//             Stream::Next(v) => Some(v),
//             _ => None,
//         })
//         .collect()
// }
// ```
// 
// For multi-value streams (like `list_keys` or `query`), the `Vec` contains all results.
// 
// ### `collect_one` — Extract First Result from a Stream
// 
// For single-value operations (like `get`) where you know the stream produces exactly one `Next`:
// 
// ```rust
// /// Blocks until first `Stream::Next(value)`, returns it. Returns None if stream exhausts.
// pub fn collect_one<D, P>(stream: impl Iterator<Item = Stream<D, P>>) -> Option<D> {
//     stream.find_map(|s| match s {
//         Stream::Next(v) => Some(v),
//         _ => None,
//     })
// }
// ```
// 
// ### `sync_collect_one` — Execute Task, Return Single Value
// 
// The single-value counterpart to `sync_one`. Returns `Result<T::Ready>`, not `Result<Vec<T::Ready>>`:
// 
// ```rust
// /// Execute a single task, block until first result. Returns the value directly.
// pub fn sync_collect_one<T>(task: T) -> GenericResult<T::Ready>
// where
//     T: TaskIterator + Send + 'static,
//     T::Ready: Send + 'static,
//     T::Pending: Send + 'static,
//     T::Spawner: ExecutionAction + Send + 'static,
// {
//     let stream = execute(task, None)?;
//     collect_one(stream).ok_or_else(|| /* error */)
// }
// ```
// 
// ### `sync_one` — Execute Task and Block for All Results
// 
// For tasks that produce multiple values:
// 
// ```rust
// /// Execute a single task and block until complete. Collects ALL results.
// pub fn sync_one<T>(task: T) -> GenericResult<Vec<T::Ready>>
// where
//     T: TaskIterator + Send + 'static,
//     T::Ready: Send + 'static,
//     T::Pending: Send + 'static,
//     T::Spawner: ExecutionAction + Send + 'static,
// {
//     let stream = execute(task, None)?;
//     Ok(collect_result(stream))
// }
// ```
// 
// ### `sync_all` — Execute Multiple Tasks, Block Until All Complete
// 
// ```rust
// /// Execute multiple homogeneous tasks in parallel, block until all complete.
// pub fn sync_all<T>(tasks: Vec<T>) -> GenericResult<Vec<T::Ready>>
// where
//     T: TaskIterator + Send + 'static,
//     T::Ready: Send + 'static,
//     T::Pending: Send + 'static,
//     T::Spawner: ExecutionAction + Send + 'static,
// {
//     let stream = execute_collect_all(tasks, None)?;
//     // execute_collect_all already buffers and collects all results internally.
//     // It yields Stream::Pending(count) while in flight, then a single
//     // Stream::Next(Vec<T::Ready>) when all complete.
//     stream
//         .find_map(|s| match s {
//             Stream::Next(v) => Some(v),
//             _ => None,
//         })
//         .ok_or_else(|| /* GenericError: no results produced */)
// }
// ```
// 
// ### Between vs. At Boundaries
// 
// **Between operations**, use `StreamIteratorExt` combinators — they preserve Valtron's execution model without blocking:
// 
// ```rust
// // BETWEEN operations — StreamIteratorExt (non-blocking, preserves Stream protocol)
// let transformed_stream = stream
//     .map_done(|user| user.name)
//     .filter_done(|name| !name.is_empty());
// ```
// 
// **At boundaries**, use standard `Iterator` methods — these block the thread and extract raw values:
// 
// ```rust
// // AT BOUNDARY — standard Iterator (blocking, extracts values)
// let names: Vec<String> = transformed_stream
//     .filter_map(|s| match s { Stream::Next(v) => Some(v), _ => None })
//     .collect();
// ```
// 
// ## The `!Send` Constraint
// 
// Many database crates (Turso, libsql) return row iterators that are `!Send`. These cannot cross the Valtron execution boundary. **Consume them fully inside the async block**, collecting into `Vec<T>` before the future returns.
// 
// ```rust
// // CORRECT: Collect inside async block — Vec<String> is Send
// let task = from_future(async move {
//     let mut rows = stmt.query([]).await?;
//     let mut keys = Vec::new();
//     while let Some(row) = rows.next().await? {
//         keys.push(row.get::<String>(0)?);
//     }
//     Ok::<_, BackendError>(keys)
// });
// let stream = execute(task, None)?;
// 
// // WRONG: turso::Rows is !Send — this won't compile
// let task = from_future(async move {
//     stmt.query([]).await  // Returns Rows which is !Send
// });
// ```
// 
// ### Using `run_future_iter` for Streaming !Send Iterators
// 
// When you need to stream rows (not collect to Vec), use `run_future_iter` to spawn a worker thread that owns the `!Send` iterator forever:
// 
// ```rust
// use foundation_core::valtron::{run_future_iter, Stream, ThreadedValue};
// 
// fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
//     let conn = Arc::clone(&self.conn);
// 
//     // run_future_iter spawns a worker thread that owns the !Send iterator
//     let iter = run_future_iter(
//         move || async move {
//             // !Send rows iterator is created and consumed inside this async block
//             let mut stmt = conn
//                 .prepare("SELECT id FROM deployment_resources ORDER BY id")
//                 .await
//                 .map_err(|e| StorageError::Backend(e.to_string()))?;
//             let rows = stmt
//                 .query([libsql::Value::Null; 0])
//                 .await
//                 .map_err(|e| StorageError::Backend(e.to_string()))?;
//             
//             // Wrap the !Send iterator with a transformation function
//             // The iterator stays on the worker thread forever
//             Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, |row| {
//                 row.get::<String>(0)
//                     .map_err(|e| StorageError::SqlConversion(e.to_string()))
//             }))
//         },
//         None,
//         None,
//     )
//     .map_err(|e| StorageError::Backend(e.to_string()))?;
// 
//     // Transform ThreadedValue to Stream protocol
//     let stream = iter.map(|threaded_value| match threaded_value {
//         ThreadedValue::Value(result) => Stream::Next(result),
//     });
// 
//     Ok(Box::new(stream))
// }
// ```
// 
// **Key pattern:**
// 
// | Component | Role |
// |-----------|------|
// | `run_future_iter(future_factory, None, None)` | Spawns worker thread, owns !Send iterator forever |
// | Generic iterator (`LibsqlRowsIterator<T, F>`) | Consumes !Send rows, transforms to `T: Send` via closure `F` |
// | Transformation closure | Inline per-use-site logic: `FnMut(&Row) -> Result<T, Error>` |
// | `ThreadedValue<T>` | Crosses thread boundary from worker to main |
// | `.map(\|tv\| match tv { ... })` | Converts `ThreadedValue` to `Stream` protocol |
// 
// **Why not `from_future` + collect?** For large result sets, collecting to `Vec` before streaming causes OOM. `run_future_iter` enables true streaming: rows are fetched, transformed, and yielded one at a time across the thread boundary.
// 
// ## The `Send + 'static` Requirement
// 
// All data captured by async blocks must be `Send + 'static` for Valtron scheduling:
// 
// ```rust
// fn query(&self, sql: &str) -> StorageResult<impl Iterator<Item = Stream<Vec<Row>, ()>>> {
//     let sql = sql.to_string();          // &str → owned String
//     let conn = Arc::clone(&self.conn);   // Arc clone, not borrow
// 
//     let task = from_future(async move {
//         // sql and conn are moved in — both Send + 'static
//         conn.prepare(&sql).await?.query([]).await
//     });
//     let stream = execute(task, None)?;
//     Ok(stream)
// }
// ```
// 
// ## Turbo-Fish for Async Block Error Types
// 
// When the compiler can't infer the error type in an async block, annotate explicitly:
// 
// ```rust
// exec_future(async move {
//     conn.execute_batch(&sql).await?;
//     Ok::<_, turso::Error>(true)  // Turbo-fish needed
// })?;
// ```
// 
// ## When Sync Blocking IS Acceptable
// 
// Some operations genuinely need to complete before anything else can proceed. Use `sync_one` or `exec_future` for:
// 
// - **One-shot initialization** — creating a DB connection, loading a model
// - **Migrations** — must complete before the application starts serving
// - **CLI tools** — where the entire program is sequential by nature
// 
// Even here, consider whether multiple initializations could run in parallel (e.g., connect to DB AND load config simultaneously).
// 
// ## Anti-Patterns
// 
// ### Anti-Pattern 1: Blocking at the Leaf (exec_future as Default)
// 
// ```rust
// // BAD: Every method blocks immediately — no parallelism possible
// fn get(&self, key: &str) -> StorageResult<Option<V>> {
//     exec_future(async move { /* ... */ })  // Blocks here!
// }
// 
// fn set(&self, key: &str, value: &V) -> StorageResult<()> {
//     exec_future(async move { /* ... */ })  // Blocks here!
// }
// 
// // Caller has no choice — two sequential blocking calls
// let user = db.get("user:1")?;      // blocks
// let config = db.get("config")?;    // blocks (could have been parallel)
// ```
// 
// ### Anti-Pattern 2: Blocking in the Middle of a Chain
// 
// ```rust
// // BAD: Blocking mid-pipeline kills composability
// let partial = collect_result(stream_a);  // blocks!
// let transformed = transform(partial);
// let final_stream = execute(make_task(transformed), None)?;
// // The first block prevented us from overlapping work
// ```
// 
// ### Anti-Pattern 3: Using loop {} in StreamIterator::next() or TaskIterator::next_status
// 
// ```rust
// // BAD: Blocks the executor thread
// impl StreamIterator for MyStream {
//     fn next(&mut self) -> Stream<D, P> {
//         loop {
//             if let Some(val) = self.try_get() {
//                 return Stream::Next(val);
//             }
//         }
//     }
// }
// 
// // GOOD: Return control to the executor
// impl StreamIterator for MyStream {
//     fn next(&mut self) -> Stream<D, P> {
//         match self.try_get() {
//             Some(val) => Stream::Next(val),
//             None if self.done => Stream::Init, // signal completion
//             None => Stream::Ignore,            // yield back to executor
//         }
//     }
// }
// ```
// 
// ## Spawner Type: Prefer `BoxedSendExecutionAction` over `NoAction`
// 
// When implementing `TaskIterator`, the `Spawner` associated type controls whether the task can spawn sub-tasks during execution. **`NoAction` should be rarely used** — it prevents the task from ever spawning work.
// 
// **Default choice:** Use `BoxedSendExecutionAction` for the `Spawner` type. This allows tasks to spawn sub-work if needed and is compatible with all executor functions.
// 
// ```rust
// use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
// 
// // PREFERRED: Allows spawning sub-tasks
// impl TaskIterator for MyTask {
//     type Ready = MyResult;
//     type Pending = MyProgress;
//     type Spawner = BoxedSendExecutionAction;
// 
//     fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
//         // ...
//     }
// }
// 
// // AVOID: Only use when you are certain the task will never need to spawn
// impl TaskIterator for SimplePureComputeTask {
//     type Ready = u32;
//     type Pending = ();
//     type Spawner = NoAction;  // Rarely appropriate
//     // ...
// }
// ```
// 
// **When `NoAction` is acceptable:** Trivial tasks that are pure computation with no possibility of needing to delegate work (e.g., test fixtures, simple wrappers around `from_future`).
// 
// ## Valtron Quick Reference
// 
// ### Core Types
// 
// | Type | Purpose |
// |------|---------|
// | `TaskStatus<D, P, S>` | Raw task state: Ready, Pending, Delayed, Init, Spawn, Ignore |
// | `Stream<D, P>` | Stream state: Next, Pending, Delayed, Init, Ignore |
// | `TaskIterator` | Trait for producing `TaskStatus` — input to `execute()` |
// | `StreamIterator` | Trait for producing `Stream` — output from `execute()` |
// 
// ### Executor Functions
// 
// | Function | Purpose | Returns |
// |----------|---------|---------|
// | `execute(task, wait)` | Schedule task, return stream | `GenericResult<DrivenStreamIterator<T>>` |
// | `execute_collect_all(tasks, wait)` | Parallel exec, collect all | `GenericResult<CollectAllStream<T>>` |
// | `execute_map_all(tasks, f, wait)` | Parallel exec, map when done | `GenericResult<MapAllDoneStream<T, F, O>>` |
// | `execute_as_task(task, wait)` | Schedule, return raw TaskStatus | `GenericResult<DrivenRecvIterator<T>>` |
// | `send(task)` | Fire and forget | `GenericResult<()>` |
// | `from_future(future)` | Wrap Future as TaskIterator | `FutureTask<F>` |
// 
// ### Pre-Execute Combinators (TaskIteratorExt)
// 
// Applied **before** `execute()` — transforms the task itself:
// 
// | Combinator | Purpose |
// |------------|---------|
// | `map_ready(f)` | Transform Ready values |
// | `map_pending(f)` | Transform Pending values |
// | `filter_ready(f)` | Filter Ready values (filtered → Ignore) |
// | `filter_state(f)` | Filter based on full `TaskStatus` |
// | `stream_collect()` | Collect all Ready values into Vec |
// | `flatten_ready()` | Flatten Ready values that are `IntoIterator` |
// | `flatten_pending()` | Flatten Pending values that are `IntoIterator` |
// | `flat_map_ready(f)` | Map + flatten Ready in one operation |
// | `flat_map_pending(f)` | Map + flatten Pending in one operation |
// | `map_state(f)` | Transform any `TaskStatus` variant |
// | `inspect_state(f)` | Side-effect on any `TaskStatus` |
// | `map_circuit(f)` | Short-circuit on condition — return error and stop, or continue |
// | `map_iter(f)` | Flatten Ready into inner iterator |
// | `split_collector(pred, size)` | Fork into observer + continuation |
// | `split_collect_one(pred)` | Fork on first match |
// | `split_collect_until(pred, size)` | Fork until predicate signals close |
// | `split_collect_until_map(f, size)` | Fork with transformation until close |
// | `split_collector_map(f, size)` | Fork with transformation |
// | `take(n)` / `take_state(n, f)` | Take first n items (Ready / any state) |
// | `take_all(n)` | Take first n items of any state |
// | `take_while(f)` / `take_while_state(f)` | Take while predicate holds |
// | `take_while_any(f)` | Take while predicate holds on any state |
// | `skip(n)` / `skip_state(n, f)` | Skip first n items (Ready / any state) |
// | `skip_all(n)` | Skip first n items of any state |
// | `skip_while(f)` / `skip_while_state(f)` | Skip while predicate holds |
// | `skip_while_any(f)` | Skip while predicate holds on any state |
// | `enumerate()` | Add index to each item |
// | `find(f)` | Find first item matching predicate |
// | `find_map(f)` | Find first item mapping to Some |
// | `fold(init, f)` | Fold/accumulate values |
// | `all(f)` | Check if all Ready items satisfy predicate |
// | `any(f)` | Check if any Ready item satisfies predicate |
// | `count()` | Count Ready items |
// | `count_all()` | Count all items (any state) |
// 
// **Quick examples:**
// 
// ```rust
// // Transform Ready values before execution
// let task = task.map_ready(|v| v * 2);
// 
// // Filter out unwanted results (filtered items become Ignore)
// let task = task.filter_ready(|v| v > 10);
// 
// // Stop immediately when seeing an error, returning it
// let task = task.map_circuit(|status| match status {
//     TaskStatus::Ready(Err(e)) => TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(Err(e))),
//     _ => TaskShortCircuit::Continue(status),
// });
// 
// // Flat map Ready values
// let task = task.flat_map_ready(|vec| vec.into_iter().map(TaskStatus::Ready));
// 
// // Split stream to observe first match while continuing
// let (observer, continuation) = task.split_collect_one(|item| item.is_success());
// ```
// 
// **Why `map_circuit` for error handling:**
// 
// When a task should fail immediately on error, you need to communicate that error to the caller while stopping iteration. Without `map_circuit`, you'd need wrapper enums or lose the error:
// 
// ```rust
// // WITHOUT map_circuit — verbose, creates wrapper states
// enum ResultWithDone<T> {
//     Continue(T),
//     Done(T),  // Need a separate variant just to signal "stop with this value"
// }
// 
// let task = task.map_ready(|result| match result {
//     Err(e) => ResultWithDone::Done(Err(e)),  // Custom enum needed
//     Ok(v) => ResultWithDone::Continue(Ok(v)),
// });
// 
// // WITH map_circuit — clear, succinct, no wrapper types
// let task = task.map_circuit(|status| match status {
//     TaskStatus::Ready(Err(e)) => TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(Err(e))),
//     _ => TaskShortCircuit::Continue(status),
// });
// ```
// 
// The key insight: `ReturnAndStop(value)` returns the value AND stops — the caller receives the error via `Stream::Next(error)` and knows iteration is done. This preserves the error without inventing wrapper types.
// 
// Use `map_circuit` **before `execute()`** when:
// - The task should terminate early on error
// - You want to propagate the error to the stream consumer
// - You need to stop immediately without creating wrapper enums
// 
// ### Post-Execute Combinators (StreamIteratorExt)
// 
// Applied **after** `execute()` — transforms the stream. Use these **between** operations, not at boundaries:
// 
// | Combinator | Purpose |
// |------------|---------|
// | `map_done(f)` | Transform Next values |
// | `map_pending(f)` | Transform Pending values |
// | `map_pending_and_done(f)` | Transform both Pending and Next with single function |
// | `map_delayed(f)` | Transform Delayed durations |
// | `filter_done(f)` | Filter Next values (filtered → Ignore) |
// | `filter_state(f)` | Filter based on full `Stream` state |
// | `collect()` | Accumulate all Next values, yield as single Vec |
// | `flatten_next()` | Flatten Next values that are `IntoIterator` |
// | `flatten_pending()` | Flatten Pending values that are `IntoIterator` |
// | `flat_map_next(f)` | Map + flatten Next in one operation |
// | `flat_map_pending(f)` | Map + flatten Pending in one operation |
// | `map_state(f)` | Transform any `Stream` variant |
// | `map_iter(f)` | Flatten Next into inner iterator |
// | `inspect_state(f)` | Side-effect on any `Stream` state |
// | `map_circuit(f)` | Short-circuit on condition — return value and stop, or continue |
// | `split_collector(pred, size)` | Fork into observer + continuation |
// | `split_collect_one(pred)` | Fork on first match |
// | `split_collect_until(pred, size)` | Fork until predicate signals close |
// | `split_collector_map(f, size)` | Fork with transformation |
// | `split_collect_one_map(f, size)` | Fork with transformation on first match |
// | `take(n)` / `take_state(n, f)` | Take first n items (Next / any state) |
// | `take_all(n)` | Take first n items of any state |
// | `take_while(f)` / `take_while_state(f)` | Take while predicate holds |
// | `take_while_any(f)` | Take while predicate holds on any state |
// | `skip(n)` / `skip_state(n, f)` | Skip first n items (Next / any state) |
// | `skip_all(n)` | Skip first n items of any state |
// | `skip_while(f)` / `skip_while_state(f)` | Skip while predicate holds |
// | `skip_while_any(f)` | Skip while predicate holds on any state |
// | `enumerate()` | Add index to each item |
// | `find(f)` | Find first item matching predicate |
// | `find_map(f)` | Find first item mapping to Some |
// | `fold(init, f)` | Fold/accumulate values |
// | `all(f)` | Check if all Next items satisfy predicate |
// | `any(f)` | Check if any Next item satisfies predicate |
// | `count()` | Count Next items |
// | `count_all()` | Count all items (any state) |
// 
// **Quick examples:**
// 
// ```rust
// // Transform Next values
// let stream = stream.map_done(|v| v.to_string());
// 
// // Filter Next values (filtered items become Ignore)
// let stream = stream.filter_done(|v| !v.is_empty());
// 
// // Stop immediately when seeing an error, preserving it for the caller
// let stream = stream.map_circuit(|item| match item {
//     Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
//     _ => ShortCircuit::Continue(item),
// });
// 
// // Chain combinators for clean pipelines
// let stream = stream
//     .filter_done(|v| v.is_ok())
//     .map_done(|v| v.unwrap());
// 
// // Flat map Next values
// let stream = stream.flat_map_next(|vec| vec.into_iter().map(Stream::Next));
// 
// // Split stream to observe first match while continuing
// let (observer, continuation) = stream.split_collect_one(|item| matches!(item, Stream::Next(v) if v > 10));
// ```
// 
// **Why `map_circuit` after `execute()`:**
// 
// When consuming a stream, you often want to stop on error while preserving the error for the caller. Without `map_circuit`, you'd need to return `None` and lose the error, or wrap results in custom enums:
// 
// ```rust
// // WITHOUT map_circuit — error is lost, caller can't distinguish
// let stream = stream.filter_done(|v| v.is_ok());  // Errors become Ignore, swallowed
// 
// // OR: verbose wrapper enum
// enum StreamValue<T> {
//     More(T),
//     Last(T),  // Extra variant just to say "stop with this"
// }
// 
// // WITH map_circuit — error preserved, iteration stops cleanly
// let stream = stream.map_circuit(|item| match item {
//     Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
//     Stream::Next(Ok(v)) => ShortCircuit::Continue(Stream::Next(Ok(v))),
//     _ => ShortCircuit::Stop,
// });
// ```
// 
// Use `map_circuit` **after `execute()`** when:
// - The stream should terminate early on error
// - You want to propagate errors to the final consumer
// - You need clean error handling without wrapper enums
// 
// ### Boundary Collection (Standard Iterator — Use at Sync Points Only)
// 
// | Method | Usage |
// |--------|-------|
// | `find_map(\|s\| match s { Stream::Next(v) => Some(v), _ => None })` | Extract first result |
// | `.filter_map(...).collect::<Vec<_>>()` | Collect all results |
// | `collect_result(stream)` | Drain stream, collect all `Next` values into `Vec<D>` |
// | `collect_one(stream)` | Drain stream until first `Next`, return `Option<D>` |
// | `sync_collect_one(task)` | Execute + return single value as `Result<T::Ready>` |
// | `sync_one(task)` | Execute + collect all results as `Result<Vec<T::Ready>>` |
// | `sync_all(tasks)` | Execute all in parallel + collect all results |
// 
// ## simple_http + Valtron Integration
// 
// When using `foundation_core::simple_http` with Valtron, follow the pattern from generated API clients:
// 
// ### Core Pattern: ClientRequestBuilder → SendRequestTask → StreamIterator
// 
// The correct pattern uses `ClientRequestBuilder`, `build_send_request()`, and `RequestIntro` handling:
// 
// ```rust
// use foundation_core::valtron::{execute, Stream};
// use foundation_core::simple_http::client::SimpleHttpClient;
// use foundation_core::simple_http::{RequestIntro, body_reader};
// 
// /// Single-value HTTP operation (blocking OK for auth/user info)
// pub fn whoami(client: &SimpleHttpClient, token: &str) -> Result<User> {
//     let client = client.clone();
//     let token = token.to_string();
//     let url = "https://huggingface.co/api/whoami-v2".to_string();
//     
//     // Start with ClientRequestBuilder
//     let builder = client.get(&url)?
//         .header("Authorization", format!("Bearer {}", token));
//     
//     // Build SendRequestTask and transform
//     let task = builder
//         .build_send_request()
//         .map_err(|e| Error::RequestBuildFailed(e.to_string()))?
//         .map_ready(|intro| match intro {
//             RequestIntro::Success { stream, status } => {
//                 let headers = status.headers().clone();
//                 if !status.is_success() {
//                     return Err(Error::HttpStatus {
//                         code: status.as_u16(),
//                         headers,
//                     });
//                 }
//                 // Read body using body_reader helper
//                 let body = body_reader::collect_string(stream);
//                 // Parse JSON inside map_ready
//                 let user: User = serde_json::from_str(&body)
//                     .map_err(|e| Error::Json(e.to_string()))?;
//                 Ok(user)
//             }
//             RequestIntro::Failed(e) => Err(Error::RequestSendFailed(e.to_string())),
//         })
//         .map_pending(|_| ());  // Discard progress info
//     
//     // Execute and collect single result
//     let stream = execute(task, None)
//         .map_err(|e| Error::Valtron(e.to_string()))?;
//     
//     // Use standard Iterator::find_map to extract first Next value
//     stream.find_map(|s| match s {
//         Stream::Next(result) => Some(result),
//         _ => None,
//     }).ok_or_else(|| Error::NoResult)
// }
// ```
// 
// ### Multi-Value HTTP Operations (Return Stream)
// 
// For listing endpoints, return a `StreamIterator` that yields individual items:
// 
// ```rust
// use foundation_core::valtron::{execute, Stream};
// use foundation_core::simple_http::{RequestIntro, body_reader};
// 
// pub type ApiStream<T> = Box<dyn Iterator<Item = Stream<Result<T, Error>, ()>> + Send>;
// 
// pub fn list_models(client: &SimpleHttpClient) -> Result<ApiStream<ModelInfo>> {
//     let client = client.clone();
//     let url = "https://huggingface.co/api/models".to_string();
//     
//     let builder = client.get(&url)?;
//     
//     let task = builder
//         .build_send_request()
//         .map_err(|e| Error::RequestBuildFailed(e.to_string()))?
//         .map_ready(|intro| match intro {
//             RequestIntro::Success { stream, status } => {
//                 let headers = status.headers().clone();
//                 if !status.is_success() {
//                     return Err(Error::HttpStatus {
//                         code: status.as_u16(),
//                         headers,
//                     });
//                 }
//                 let body = body_reader::collect_string(stream);
//                 // Parse Vec<ModelInfo> from response
//                 let models: Vec<ModelInfo> = serde_json::from_str(&body)
//                     .map_err(|e| Error::Json(e.to_string()))?;
//                 Ok(models)
//             }
//             RequestIntro::Failed(e) => Err(Error::RequestSendFailed(e.to_string())),
//         })
//         .map_pending(|_| ());
//     
//     let stream = execute(task, None)
//         .map_err(|e| Error::Valtron(e.to_string()))?;
//     
//     // Expand Vec into individual stream items using flat_map_next
//     Ok(Box::new(stream.flat_map_next(|result| {
//         match result {
//             Ok(models) => models.into_iter().map(|m| Stream::Next(Ok(m))).collect::<Vec<_>>().into_iter(),
//             Err(e) => vec![Stream::Next(Err(e))].into_iter(),
//         }
//     })))
// }
// ```
// 
// ### Reference Implementation Pattern
// 
// From `backends/foundation_deployment/src/providers/prisma_postgres/clients/mod.rs`:
// 
// ```rust
// pub fn get_v1_compute_services_execute(
//     builder: ClientRequestBuilder<SystemDnsResolver>,
// ) -> Result<
//     impl StreamIterator<D = Result<ApiResponse<T>, ApiError>, P = ApiPending> + Send + 'static,
//     ApiError,
// > {
//     let task = builder
//         .build_send_request()
//         .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?
//         .map_ready(|intro| match intro {
//             RequestIntro::Success { stream, intro, headers, .. } => {
//                 let status_code: usize = intro.0.into();
//                 if status_code < 200 || status_code >= 300 {
//                     let body = body_reader::collect_string(stream);
//                     return Err(ApiError::HttpStatus {
//                         code: status_code as u16,
//                         headers: headers.clone(),
//                         body: Some(body),
//                     });
//                 }
//                 let body = body_reader::collect_string(stream);
//                 let parsed: T = serde_json::from_str(&body)
//                     .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
//                 Ok(ApiResponse { status: status_code as u16, headers, body: parsed })
//             }
//             RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),
//         })
//         .map_pending(|_| ApiPending::Sending);
//     
//     execute(task, None).map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
// }
// ```
// 
// ### Key Points
// 
// 1. **Start with `ClientRequestBuilder`** — `client.get(url)?` or `client.post(url)?`
// 2. **Call `build_send_request()`** — Returns `SendRequestTask`
// 3. **Transform with `map_ready()`** — Handle `RequestIntro::Success { stream, status }` and `RequestIntro::Failed(e)`
// 4. **Check status code** — Return error if `!status.is_success()`
// 5. **Read body with `body_reader::collect_string(stream)`** — Helper to consume response stream
// 6. **Parse JSON inside `map_ready`** — Transform `String` body to your type
// 7. **Apply `map_pending()`** — Usually `map_pending(|_| ())` to discard progress
// 8. **Call `execute(task, None)`** — Returns `StreamIterator`
// 9. **For single-value ops** — Use `.find_map()` or `collect_one()` to extract result
// 10. **For multi-value ops** — Use `.flat_map_next()` to expand `Vec<T>` into stream items
// 
// ### Pattern Comparison
// 
// | Old (Incorrect) | New (Correct) |
// |-----------------|---------------|
// | `from_future(async { client.execute(req) })` | `client.get(url)?.build_send_request()` |
// | `response.body()?` | `RequestIntro::Success { stream, status }` |
// | Manual status check after send | Status check inside `map_ready` |
// | `SendSafeBody` parsing | `body_reader::collect_string(stream)` |
// 
// The new pattern aligns with how `SimpleHttpClient` is actually used in the codebase.
// 
// ## Decision Flowchart
// 
// **Is this operation I/O (database, HTTP, file download)?**
// - No → Keep it synchronous. No Valtron needed.
// - Yes → Continue:
// 
// **Does the caller need to compose this with other operations?**
// - Yes → Return a stream (`execute()` + return the iterator). Always the default for library/trait methods.
// - No → Is this a one-shot initialization or CLI tool?
//   - Yes → `sync_one()` or `exec_future()` is acceptable.
//   - No → Return a stream anyway. The caller might compose later.
// 
// **Does the caller benefit from progress reporting?**
// - Yes → Use a rich `Pending` type (e.g., `MigrationProgress`, `FetchPending`).
// - No → Use `Pending = ()`.
// 
// ## Valtron Streams in Async Contexts
// 
// When you need to use valtron streams inside `async` code (CF Workers, axum handlers, etc.), the **stream-to-future bridge** from feature 11 provides the conversion.
// 
// ### The Problem
// 
// Valtron `StreamIterator` is a synchronous `Iterator` — it yields `Stream<D, P>` states via `.next()`. Rust async code expects `Future<Output = T>` or `impl futures_core::Stream`. You can't `.await` a `StreamIterator` directly.
// 
// ### The Solution: Stream-to-Future Bridge Types
// 
// Four wrapper types in `foundation_core::valtron::stream_future` convert `StreamIterator` into standard async types:
// 
// | Type | Returns | When to use |
// |------|---------|-------------|
// | `StreamCollectFuture<SI>` | `Future<Output = Vec<D>>` | Collect all `Next` values — replacement for `collect_result()` |
// | `StreamReadyFuture<SI>` | `Future<Output = Option<(D, SI)>>` | Get the first `Next` value + remaining iterator — replacement for `collect_one()` |
// | `StreamPendingFuture<SI>` | `Future<Output = Option<(P, SI)>>` | Get the first `Pending` context — for progress-driven logic |
// | `StreamAsFutureStream<SI>` | `impl futures_core::Stream<Item = Stream<D, P>>` | Pass-through as async stream — for streaming consumers |
// 
// ### Pattern 1: Replace `collect_result()` in async code
// 
// ```rust
// // SYNC — blocks the thread
// let results: Vec<User> = collect_result(user_stream);
// 
// // ASYNC — returns a Future, use .await
// let results: Vec<User> = user_stream.into_collect_future().await;
// ```
// 
// ### Pattern 2: Replace `collect_one()` in async code
// 
// ```rust
// // SYNC — blocks until first Next
// let user = collect_one(user_stream).ok_or_else(|| Error::NoResult)?;
// 
// // ASYNC — resolves to first Next
// let result = user_stream.into_ready_future().await;
// let user = result
//     .map(|(value, _remaining)| value)
//     .ok_or_else(|| Error::NoResult)?;
// ```
// 
// ### Pattern 3: Full async pipeline from `from_future`
// 
// ```rust
// pub async fn get_user_async(conn: Arc<DbConn>, id: &str) -> Result<Option<User>> {
//     let conn = Arc::clone(&conn);
//     let id = id.to_string();
// 
//     // Step 1: Create future
//     let future = from_future(async move {
//         let mut stmt = conn.prepare("SELECT * FROM users WHERE id = ?").await?;
//         let row = stmt.query_row([&id]).await?;
//         let user: Option<User> = row.map(|r| from_row(&r)).transpose()?;
//         Ok::<_, DbError>(user)
//     });
// 
//     // Step 2: Execute → StreamIterator
//     let stream = execute(future, None)
//         .map_err(|e| Error::Scheduling(e.to_string()))?;
// 
//     // Step 3: Bridge to Future + await
//     stream.into_ready_future().await
//         .map(|(result, _)| result)
//         .ok_or_else(|| Error::NoResult)?
// }
// ```
// 
// ### Pattern 4: Using `schedule_future` helper (when available)
// 
// If `schedule_future` is available (e.g., in `foundation_db::async_utils`), it wraps the `from_future` + `execute` + `map` pipeline:
// 
// ```rust
// pub async fn get_async<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
//     let key = key.to_string();
//     let this = self.clone();
// 
//     let future = async move {
//         let stream = this.get::<V>(&key)?;
//         stream.flat_map(|s| match s {
//             Stream::Next(r) => vec![r],
//             _ => vec![],
//         }).next().ok_or_else(|| StorageError::NotFound(key.clone()))?
//     };
// 
//     schedule_future(future)?
//         .into_ready_future()
//         .await
//         .ok_or_else(|| StorageError::NotFound(key.to_string()))
// }
// ```
// 
// ### Pattern 5: Streaming consumption via `StreamAsFutureStream`
// 
// When you need to process items one at a time as an async stream:
// 
// ```rust
// use futures_core::Stream as FuturesStream;
// use futures_lite::stream::StreamExt; // for .next()
// 
// let mut stream = data_stream.into_future_stream();
// while let Some(item) = stream.next().await {
//     match item {
//         Stream::Next(Ok(value)) => process(value),
//         Stream::Next(Err(e)) => handle_error(e),
//         Stream::Pending(ctx) => log_progress(ctx),
//         _ => {}
//     }
// }
// ```
// 
// ### Critical: `wake_by_ref()` on `Poll::Pending`
// 
// All bridge `Future` impls call `cx.waker().wake_by_ref()` before returning `Poll::Pending`. This is required because:
// 
// 1. The wrapped `StreamIterator` is synchronous — data is always ready
// 2. Returning `Poll::Pending` contracts: "I'm not resolved, schedule me again"
// 3. Without `wake_by_ref()`, the runtime never re-polls → hangs forever
// 4. `wake_by_ref()` signals "re-poll me immediately" — the runtime coalesces the wake
// 
// **This is correct Future semantics, not a workaround.** It simulates the waker firing that would happen in a real async context when I/O completes.
// 
// ### `Unpin` Requirements
// 
// All bridge Future impls require `SI: Unpin` on the `StreamIterator`:
// 
// ```rust
// impl<SI> Future for StreamReadyFuture<SI>
// where
//     SI: StreamIterator + Unpin,  // Required for Pin::get_mut()
// {
//     // ...
// }
// ```
// 
// `StreamCollectFuture` additionally requires `SI::D: Unpin` because `Vec<T>` is only `Unpin` when `T: Unpin` (Rust stdlib quirk).
// 
// All valtron iterators (regular structs, Vec iterators, combinator wrappers) are `Unpin` by default — none contain self-referential data.
// 
// ### When NOT to Use the Bridge
// 
// - **You have native async APIs** (e.g., `D1WasmStorage` calling `JsFuture`) — call them directly, don't route through valtron.
// - **The operation is pure computation** — no need for valtron, just use a regular `async` block.
// - **You're already in a sync context** — use `collect_result()` / `collect_one()` directly.
// 
// The bridge is specifically for: "I have a valtron `StreamIterator` and I need to `.await` it in async code."
// 
// ### Decision Flowchart for Async
// 
// ```
// Do you need to use valtron streams in async code?
// ├─ No → Use sync collection (collect_result, collect_one)
// └─ Yes →
//     ├─ Do you have native async APIs (JsFuture, reqwest)?
//     │   ├─ Yes → Call them directly, don't bridge
//     │   └─ No → Use the stream-to-future bridge
//     │       ├─ Need all values? → .into_collect_future().await
//     │       ├─ Need first value? → .into_ready_future().await
//     │       ├─ Need pending context? → .into_pending_future().await
//     │       └─ Need async stream? → .into_future_stream()
// ```
// 
// ## Lessons from CF Workers wasm32 Deployment (2026-05-30)
// 
// ### TaskIterator::next_status() MUST Eventually Return `None`
// 
// **The most critical lesson:** Every `TaskIterator` implementation **must** return `None` at some point. If it always returns `Some(...)`, the task stays in the executor forever, the `NotifyQueue` never closes, and any `while let Some(item) = stream.next().await` loop hangs indefinitely.
// 
// ```rust
// // BAD: Always returns Some — task never terminates
// fn next_status(&mut self) -> Option<TaskStatus<...>> {
//     if self.current >= self.max {
//         return Some(TaskStatus::Ready(self.current)); // current never increments → infinite loop!
//     }
//     // ...
// }
// 
// // GOOD: Returns None after final Ready
// fn next_status(&mut self) -> Option<TaskStatus<...>> {
//     if self.current > self.max {
//         return None;  // Task completes, queue closes, stream terminates
//     }
//     if self.current == self.max {
//         self.current += 1; // Must advance past max so next call hits the None branch
//         return Some(TaskStatus::Ready(self.current));
//     }
//     // ...
// }
// ```
// 
// **Rule:** After returning `TaskStatus::Ready(final_value)`, the next `next_status()` call must return `None`. You must either increment the counter or track a `done` flag.
// 
// ### CondVar on wasm32 Requires `js-wasmbindgen` Feature Gate (When `std` Is Also On)
// 
// `std::sync::Condvar` internally calls `thread::sleep` on wasm32-unknown-unknown, which **panics** with "time not implemented on this platform". When `std` is off, the no_std spin-waiting CondVar is used automatically.
// 
// **The trap:** CF Workers builds often need `std` for `Duration`, `OnceLock`, `RefCell`, etc. — so you can't just disable `std`. In that case, the `js-wasmbindgen` feature gate overrides CondVar/Mutex routing to use the no_std spin-waiting implementation even with `std` enabled:
// 
// ```toml
// # cf-valtron-counter/Cargo.toml
// foundation_core = { default-features = false, features = ["js-wasmbindgen", "std"] }
// ```
// 
// The feature is threaded through: `foundation_nostd/Cargo.toml` adds `js-event-loop = []`, then `foundation_core/Cargo.toml`'s `js-wasmbindgen` feature includes `"foundation_nostd/js-event-loop"`. The cfg gates in `foundation_nostd` route to `nostd_impl` when `js-event-loop` is set, even if `std` is also set:
// 
// ```rust
// #[cfg(all(feature = "std", not(feature = "js-event-loop")))]
// mod std_impl;
// #[cfg(any(not(feature = "std"), feature = "js-event-loop"))]
// mod nostd_impl;
// ```
// 
// ### setTimeout in CF Workers Requires Reflect Fallback
// 
// CF Workers run in a custom global scope — neither `Window` nor `DedicatedWorkerGlobalScope`. After checking both, use `js_sys::Reflect::get` to get `setTimeout`:
// 
// ```rust
// } else {
//     let global = js_sys::global();
//     let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
//         .expect("global has no setTimeout");
//     let set_timeout = set_timeout.dyn_ref::<js_sys::Function>()
//         .expect("setTimeout is not a function");
//     let _ = set_timeout.call2(&global, closure.as_ref().unchecked_ref(), &JsValue::from_f64(dur.as_millis() as f64));
// }
// ```
// 
// ### tracing_subscriber::fmt() Panics on CF Workers — Use `.without_time()`
// 
// The default `tracing_subscriber::fmt()` calls `SystemTime::now()` which is unavailable in CF Workers. Always use `.without_time()`:
// 
// ```rust
// tracing_subscriber::fmt()
//     .without_time()          // Required — SystemTime not available
//     .with_max_level(tracing::Level::TRACE)
//     .with_env_filter(filter)
//     .try_init();
// ```
// 
// ### Pool Initialization: Prefer `#[valtron]`, Keep It Global Not Per-Request
// 
// For a normal entry point (a `fn main` or any function that owns the program's
// run), use the `#[valtron]` attribute macro. It calls `initialize_pool` for you,
// keeps the `PoolGuard` alive for the whole body, and drops it (tearing the pool
// down) afterwards — so you never hand-manage the guard or risk dropping it early:
// 
// ```rust
// use foundation_core::valtron::valtron;
// 
// #[valtron]                 // pool is live for the whole body, torn down after
// fn main() {
//     // spawn tasks, run_until_complete, etc.
// }
// 
// // With explicit seed / thread count (same args as #[valtron_test]):
// #[valtron(seed = 42, threads = 8)]
// fn main() { /* ... */ }
// ```
// 
// The pool is **global, singleton state** — it must be stood up once for the
// program's lifetime, never per-request. `#[valtron]` enforces that naturally
// because the guard lives for the wrapped function's whole duration.
// 
// **When you can't use `#[valtron]`** — e.g. a long-lived host (CF Worker) whose
// entry point is called per-request and must NOT re-init the pool each time — keep
// a single global guard and initialize once:
// 
// ```rust
// static POOL_GUARD: OnceLock<PoolGuard> = OnceLock::new();
// 
// #[wasm_bindgen]
// pub async fn fetch(req: web_sys::Request, _env: worker::Env) -> web_sys::Response {
//     POOL_GUARD.get_or_init(|| initialize_pool(42, None)); // once, not per-request
//     // ... handle request
// }
// ```
// 
// Calling `initialize_pool()` per-request (without the `OnceLock` guard) will
// block on the lifecycle gate and/or corrupt state — don't.
// 
// ### SingleExecutorSingleton (wasm32/wasm64, optional convenience API)
// 
// On wasm32/wasm64 targets (when `multi` feature is off), valtron provides `SingleExecutorSingleton` — a convenience wrapper that combines initialization + guard ownership into one call, similar to `CfHttpAppSingleton`:
// 
// ```rust
// use foundation_core::valtron::SingleExecutorSingleton;
// 
// // One call initializes the executor and gives you a guard
// let _guard = SingleExecutorSingleton::get_or_init(42, |_| {
//     // optional setup closure — e.g., register services
// });
// 
// // Later: get the guard without reinitializing
// let guard = SingleExecutorSingleton::guard();
// 
// // Check if initialized
// if SingleExecutorSingleton::is_initialized() { ... }
// ```
// 
// The existing free functions (`initialize_pool`, `spawn`, `run_until_complete`) continue to work independently — `SingleExecutorSingleton` is optional. Use it when you want explicit guard ownership similar to the CF/Web HTTP app singletons.
// 
// ### CF Workers Build Requires `worker-build`, Not Raw wasm-bindgen
// 
// Standard `wasm-bindgen --target web` or `--target bundler` produces output incompatible with CF Workers. Use `worker-build` (the worker-rs tool) which handles the correct JS shim generation:
// 
// ```bash
// worker-build --release   # Outputs to build/
// npx wrangler dev         # Uses build/index.js
// ```
// 
// ### `execute()` vs `spawn()` — Use `execute()` for Stream Results
// 
// `execute()` internally calls `spawn()` then `drive_stream()` on the resulting `NotifyQueueStreamIterator`. Don't manually call `spawn()` and then try to wrap it yourself — just use `execute()`:
// 
// ```rust
// // GOOD: One call does everything
// let driven_iter = execute(CounterTaskIterator::new(10), Some(Duration::from_millis(4)))?;
// 
// // BAD: Don't replicate what execute() does internally
// let iter = single::spawn().with_task(task).schedule_iter(...)?;
// let driven_iter = drive_stream(iter);
// ```
// 
// ### StreamAsFutureStream Must Call `wake_by_ref()` on `Poll::Pending`
// 
// When wrapping valtron's sync iterators as `futures_core::Stream`, returning `Poll::Pending` without calling `cx.waker().wake_by_ref()` causes the future to never be re-polled. `Stream::Wait` from the executor must still trigger a re-poll.
// 
// ## Error Handling: `foundation_errstacks` Required
// 
// **All types used with `VfsResult<T>` MUST implement `Debug`.**
// 
// - `VfsResult<T>` is `Result<T, ErrorTrace<VfsError>>` — never raw `Result<T, VfsError>`
// - All sync bridge wrappers (`SyncFile`, `SyncFs`, `SyncDirectory`, `SyncDynDirectory`, `LocalSeekableFile`, `SyncLibsqlDelta`, etc.) implement `Debug` using `finish_non_exhaustive()` for inner async types that may not be `Debug`
// - Tests use `err.current_context()` to access the typed `&VfsError` — **never** `downcast_ref`
// - Import `foundation_errstacks::ErrorTraceResultExt` when you need `change_context()` or `attach_printable()`
// 
// ```rust
// // CORRECT: current_context() returns &VfsError
// let err = sync.stat("/missing").unwrap_err();
// assert!(matches!(err.current_context(), VfsError::NotFound { .. }));
// 
// // WRONG: downcast_ref bypasses foundation_errstacks API
// let err = sync.stat("/missing").unwrap_err();
// assert!(err.downcast_ref::<VfsError>().is_some()); // DON'T DO THIS
// ```
// 
// **Debug impl pattern for sync bridge wrappers:**
// 
// ```rust
// impl<A: AsyncVfsFile + 'static> fmt::Debug for SyncFile<A> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_struct("SyncFile").finish_non_exhaustive()
//     }
// }
// ```
// 
// ## Test Annotations: Use `#[valtron_test]` — NOT `#[test]` + `#[serial]`
// 
// Valtron tests use the `#[valtron_test]` attribute macro (re-exported from
// `foundation_core::valtron`). It is to valtron what `#[tokio::test]` is to tokio:
// it **emits its own `#[test]`**, stands the pool up before the body, and tears it
// down after — and pool serialization is now handled process-wide by the engine
// (`initialize_pool` holds a fair lifecycle gate), so **`#[serial]` is no longer
// needed or wanted**.
// 
// ```rust
// use foundation_core::valtron::valtron_test;
// 
// #[valtron_test]                     // emits #[test]; brings the pool up/down
// #[tracing_test::traced_test]        // optional: log visibility
// fn test_name() {
//     // The pool is already live here — call get_pool()/spawn() directly.
//     // Do NOT call initialize_pool() yourself; the macro did it.
// }
// ```
// 
// ### DO NOT stack `#[test]` or `#[serial]`
// 
// ```rust
// // WRONG — registers the test TWICE (the macro adds its own #[test]); the two
// // copies run concurrently and race on the global pool → intermittent hangs.
// #[test]                  // ← remove
// #[serial]                // ← remove (the lifecycle gate serializes for you)
// #[valtron_test]
// fn bad() { ... }
// 
// // RIGHT
// #[valtron_test]
// fn good() { ... }
// ```
// 
// Why you can't rely on the macro to strip a stray `#[test]`: attribute macros
// expand outermost-first, so an outer `#[serial]` rewrites the function (hoisting
// `#[test]`) before `#[valtron_test]` ever sees it. The contract is: never write
// both. Non-test attributes like `#[tracing_test::traced_test]` are fine.
// 
// ### Arguments: `seed` and `threads`
// 
// Both `#[valtron_test]` and the entry-point `#[valtron]` take the same optional
// args:
// 
// ```rust
// #[valtron_test]                          // random seed, engine-default threads
// #[valtron_test(seed = 42)]               // fixed seed, default threads
// #[valtron_test(threads = 4)]             // random seed, 4 threads
// #[valtron_test(seed = 42, threads = 2)]  // both fixed
// ```
// 
// - `seed = N` → deterministic RNG seed; absent → a random seed.
// - `threads = N` → `Some(N)` worker count; absent → `None` (engine default).
//   The multi pool clamps very small counts up (it needs ≥ 2 task workers), so
//   `threads = 1` is accepted, not rejected.
// 
// ### `#[valtron]` for binaries/entry points
// 
// Same macro family for non-test entry points (e.g. `fn main`). It wraps the body
// so the pool is live for its whole duration and torn down after — same `seed` /
// `threads` args, no `#[test]`:
// 
// ```rust
// #[valtron(threads = 8)]
// fn main() {
//     // spawn tasks, run_until_complete, etc. — pool is live
// }
// ```
// 
// ### When you must manage the guard yourself (rare)
// 
// Tests that need to drop the `PoolGuard` mid-body to assert shutdown timing
// cannot use `#[valtron_test]` (it owns the guard). Those keep a plain `#[test]`
// and call `initialize_pool` directly — but still NO `#[serial]` (the lifecycle
// gate serializes them anyway):
// 
// ```rust
// #[test]
// fn shutdown_is_fast() {
//     let guard = initialize_pool(42, Some(4));
//     let start = std::time::Instant::now();   // time AFTER acquiring the pool —
//                                              // initialize_pool blocks on the
//                                              // fair gate; don't count that wait
//     // ... spawn long tasks ...
//     drop(guard);                             // explicit teardown under test
//     assert!(start.elapsed() < Duration::from_secs(2));
// }
// ```
// 
// ### `block_on`-based tests: kill INSIDE the closure with a cloned handle
// 
// `block_on(seed, threads, setup)` runs `setup`, then blocks in
// `waitgroup().wait()` until workers die. Anything that kills the pool must run
// and `join()` **before `setup` returns**, using the `pool` handle passed in
// (NOT `get_pool()`, which races the guard tearing down the global registry):
// 
// ```rust
// #[tracing_test::traced_test]             // NO #[test], NO #[serial]
// fn drives_to_completion() {
//     block_on(seed, None, |pool| {
//         let pool_clone = pool.clone();
//         let killer = std::thread::spawn(move || {
//             // ... wait for completion signal ...
//             pool_clone.kill();           // cloned handle, not get_pool()
//         });
//         pool.spawn().with_task(task).schedule().unwrap();
//         killer.join().unwrap();          // joined before setup returns
//     });
// }
// ```
// 
// See `backends/foundation_core/src/valtron/docs/debugging_multi_pool_test_hangs.md`
// for the full war-story on why these rules exist (duplicate registration, unfair
// lock starvation, `block_on` deadlock, WaitGroup lost-wakeup) and how to thread-
// dump a hung Rust test with gdb.
// 
// ---
// 
// _Created: 2026-03-28 · Updated: 2026-06-17 (migrate to #[valtron_test]; drop #[serial])_
// _Reference implementations: backends/foundation_core/tests/valtron/multi_workers.rs, shutdown_mechanism_tests.rs_
