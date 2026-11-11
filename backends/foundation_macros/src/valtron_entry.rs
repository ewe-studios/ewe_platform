//! WHY: Setting up the valtron execution engine by hand means three lines of
//! ceremony in every binary and test — initialize the pool, KEEP THE GUARD ALIVE
//! (dropping it shuts the threads down), remember to drop it last. Exactly the
//! problem `#[tokio::main]`/`#[tokio::test]` solve for tokio.
//!
//! WHAT: The bodies behind `#[valtron]` (entry points) and `#[valtron_test]`
//! (tests). Entry points live in `lib.rs` (proc-macro fns must sit at the crate
//! root); `foundation_core::valtron` re-exports both so users write
//! `use foundation_core::valtron::valtron_test;`.
//!
//! HOW: read the walkthrough below.
//!
//! ---
//!
//! # What the expansion looks like
//!
//! ```ignore
//! #[valtron]
//! fn main() { /* spawn tasks, run_until_complete, … */ }
//! ```
//!
//! becomes (simplified):
//!
//! ```ignore
//! fn main() {
//!     // The original body, MOVED into an inner fn with the SAME return type —
//!     // so `return` statements and `?` inside the body keep their meaning
//!     // (a closure would silently capture `return`).
//!     fn __valtron_body() { /* spawn tasks, … */ }
//!
//!     // Engine up inside a block scope. `initialize_pool` is the unified entry —
//!     // it picks the multi-threaded pool (feature "multi") or the
//!     // single-threaded/wasm pool automatically, and returns the matching
//!     // PoolGuard.
//!     {
//!         let __valtron_guard = ::foundation_core::valtron::initialize_pool(
//!             /* seed */    …,        // explicit `seed = N`, or a RandomState-derived u64
//!             /* threads */ None,     // explicit `threads = N`, or the engine default
//!         );
//!
//!         // Run the user's code WHILE the guard lives. The block ends, and so
//!         // drops the guard, AFTER the body has fully returned. PoolGuard::drop
//!         // joins/kills the pool threads under feature "multi".
//!         __valtron_body()
//!     }
//! }
//! ```
//!
//! `#[valtron_test]` is the same wrapper plus `#[test]` (like `#[tokio::test]`),
//! so the case is a plain `cargo test` target with the engine running around it.
//!
//! # CONTRACT: `#[valtron_test]` REPLACES `#[test]` — do not stack them
//!
//! Like `#[tokio::test]`, `#[valtron_test]` emits its OWN `#[test]`. Writing
//!
//! ```ignore
//! #[test]                       // ← WRONG: do not add this
//! #[serial]                     // ← WRONG: lifecycle lock already serializes
//! #[valtron_test(threads = 2)]
//! fn my_test() { … }
//! ```
//!
//! registers the test TWICE. Both copies share the process-global pool registry
//! (`REGISTRY`/`BG_REGISTRY` in the multi executor are singletons), so they race:
//! one acquires the pool lifecycle lock, the other blocks forever — a hang that
//! looks like "test running for over 60 seconds".
//!
//! Why you cannot rely on the macro to clean this up: attribute proc-macros
//! expand OUTERMOST-first. A `#[serial]` (or any proc-macro attr) sitting ABOVE
//! `#[valtron_test]` runs first and hoists the built-in `#[test]` into a wrapper
//! layer BEFORE `#[valtron_test]` ever sees it — so the defensive `#[test]`
//! filter below (which strips a directly-adjacent `#[test]`) can't catch it.
//!
//! Rules:
//! - Use `#[valtron_test]` ALONE (plus non-test attrs like `#[traced_test]`).
//! - Do NOT add `#[test]`.
//! - Do NOT add `#[serial]` — `initialize_pool` holds a process-wide lifecycle
//!   lock for the pool's entire lifetime, so all pool users are already
//!   serialized whether or not they use `#[valtron_test]`.
//!
//! Both macros take the SAME `seed`/`threads` rules: `threads = N` → `Some(N)`,
//! absent → `None` (engine default); `seed = N` → that seed, absent → a random
//! one (see "The default seed" below).
//!
//! Both macros also take `ctx = expr`, the root `Ctx` the engine starts with.
//! With it, the expansion calls `initialize_pool_with_ctx`; without it, the
//! expansion is the same as before this argument existed. A `timeout` body runs
//! on a new thread and receives the same root through
//! `initialize_current_thread_with_ctx`.
//!
//! Both macros take `slow_poll = ms`: warn when one task poll holds a worker
//! longer than `ms` milliseconds. The default is 100 ms, so a blocked test
//! reports itself. A warning names the task and the elapsed microseconds, at
//! most once per task per second.
//!
//! **A task poll must not block.** Do not call `join()`, `recv()` on a blocking
//! socket, or a blocking file read inside a task body or a future. The call
//! holds the worker that polls it, so a small pool starves and the effect looks
//! like a flaky test. Wait with a readiness signal, a timer, or an async
//! equivalent instead. `slow_poll` reports the next violation.
//!
//! # Why the guard handling is the whole point
//!
//! `initialize_pool` returns a `PoolGuard` whose `Drop` tears the engine down.
//! The classic footgun is `let _ = initialize_pool(…)` — the guard drops on THAT
//! LINE and the pool dies before any task runs. The macro makes the correct
//! lifetime structural: the guard is a named local in a block that encloses the
//! body, so it provably outlives the entire body and drops when the block ends,
//! after the body has returned. A block scope is used instead of an explicit
//! `drop(guard)` because the guard has a `Drop` impl only under feature
//! "multi"; in the single-threaded build `drop()` is a no-op that
//! `clippy::drop_non_drop` reports.
//!
//! # The default seed
//!
//! When no `seed = N` is given, the expansion derives one from
//! `std::collections::hash_map::RandomState` (each `RandomState::new()` carries
//! fresh process-level entropy; hashing a constant through it yields an
//! unpredictable `u64`). That keeps the macro dependency-free — no `rand` in
//! user crates — while still varying run-to-run. Pass `seed = 42` for
//! reproducible scheduling in tests.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::crate_paths::foundation_core_path;

/// Parsed `#[valtron(...)]` / `#[valtron_test(...)]` arguments.
#[derive(Default)]
struct Args {
    seed: Option<syn::Expr>,
    threads: Option<syn::Expr>,
    timeout: Option<syn::Expr>,
    /// `slow_poll = ms` — warn when one task poll blocks a worker longer than
    /// this. Default 100 ms.
    slow_poll: Option<syn::Expr>,
    /// `ctx = expr` — the `Ctx` the engine starts with. Absent → the engine
    /// uses `Ctx::with_seed` with the real clock and the pool seed.
    ctx: Option<syn::Expr>,
    /// `shutdown_grace = ms` — after a `timeout`, how long the pool gets to stop
    /// before stuck workers are detached. Default
    /// `foundation_core::valtron::DEFAULT_TEST_SHUTDOWN_GRACE`.
    shutdown_grace: Option<syn::Expr>,
    /// `tracing = "debug"` — override the default `RUST_LOG` fallback (`"info"`).
    tracing: Option<syn::Expr>,
    /// `tracing_targets = true` — show module path per event.
    tracing_targets: Option<syn::Expr>,
    /// `tracing_ids = false` — hide thread ids.
    tracing_ids: Option<syn::Expr>,
    /// `tracing_names = true` — show thread names.
    tracing_names: Option<syn::Expr>,
    /// `tracing_files = true` — show source file per event.
    tracing_files: Option<syn::Expr>,
    /// `tracing_lines = true` — show line numbers.
    tracing_lines: Option<syn::Expr>,
    /// `tracing_ansi = true` — enable ANSI escapes.
    tracing_ansi: Option<syn::Expr>,
    /// `tracing_writer = "console"` — send events to `console.log` (wasm) instead
    /// of the default standard output. `"stdout"` restores the default.
    tracing_writer: Option<bool>,
}

const KNOWN_ARGS: &[&str] = &[
    "seed",
    "threads",
    "timeout",
    "slow_poll",
    "ctx",
    "shutdown_grace",
    "tracing",
    "tracing_targets",
    "tracing_ids",
    "tracing_names",
    "tracing_files",
    "tracing_lines",
    "tracing_ansi",
    "tracing_writer",
];

/// Argument grammar: a comma-separated list of `name = expr` pairs. A newtype
/// because `Punctuated` doesn't implement `Parse`.
struct ArgList(syn::punctuated::Punctuated<syn::MetaNameValue, syn::Token![,]>);

impl syn::parse::Parse for ArgList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(syn::punctuated::Punctuated::parse_terminated(input)?))
    }
}

fn parse_args(attr: TokenStream) -> Result<Args, syn::Error> {
    let mut args = Args::default();
    if attr.is_empty() {
        return Ok(args);
    }
    let ArgList(pairs) = syn::parse2::<ArgList>(attr).map_err(|err| {
        syn::Error::new(
            err.span(),
            "expected `key = value` pairs (seed, threads, timeout, slow_poll, ctx, shutdown_grace, tracing, tracing_targets, tracing_ids, tracing_names, tracing_files, tracing_lines, tracing_ansi, tracing_writer)",
        )
    })?;
    for pair in pairs {
        let name = pair.path.get_ident().map_or_default(ToString::to_string);
        match name.as_str() {
            "seed" => args.seed = Some(pair.value),
            "threads" => args.threads = Some(pair.value),
            "timeout" => args.timeout = Some(pair.value),
            "slow_poll" => args.slow_poll = Some(pair.value),
            "ctx" => args.ctx = Some(pair.value),
            "shutdown_grace" => args.shutdown_grace = Some(pair.value),
            "tracing" => args.tracing = Some(pair.value),
            "tracing_targets" => args.tracing_targets = Some(pair.value),
            "tracing_ids" => args.tracing_ids = Some(pair.value),
            "tracing_names" => args.tracing_names = Some(pair.value),
            "tracing_files" => args.tracing_files = Some(pair.value),
            "tracing_lines" => args.tracing_lines = Some(pair.value),
            "tracing_ansi" => args.tracing_ansi = Some(pair.value),
            "tracing_writer" => {
                let lit =
                    match pair.value {
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) => s.value(),
                        other => return Err(syn::Error::new_spanned(
                            other,
                            "tracing_writer expects a string literal: \"console\" or \"stdout\"",
                        )),
                    };
                args.tracing_writer = Some(match lit.as_str() {
                    "console" => true,
                    "stdout" => false,
                    other => {
                        return Err(syn::Error::new_spanned(
                            &pair.path,
                            format!(
                                "tracing_writer = \"{other}\" — expected \"console\" or \"stdout\""
                            ),
                        ))
                    }
                });
            }
            other => {
                return Err(syn::Error::new_spanned(
                    &pair.path,
                    format!("unknown argument `{other}` (expected one of: {KNOWN_ARGS:?})"),
                ))
            }
        }
    }
    Ok(args)
}

/// Build a `foundation_compact::trace::try_init_tracing_with(TracingTestConfig { ... })`
/// call from the parsed macro args. Called at macro expansion time (not inside
/// `quote!`), so the returned tokens are spliced directly into the test body.
///
/// For `#[valtron_test]` (`is_test = true`): tracing is always on (defaults to "info").
/// For `#[valtron]` (`is_test = false`): tracing is OFF unless explicitly requested
/// via `tracing = "debug"` or similar.
fn expand_tracing_init(
    args: &Args,
    fc: &proc_macro2::TokenStream,
    is_test: bool,
) -> proc_macro2::TokenStream {
    // For #[valtron] (non-test), skip tracing unless explicitly requested.
    // For #[valtron_test], always initialize with defaults.
    if !is_test
        && args.tracing.is_none()
        && args.tracing_targets.is_none()
        && args.tracing_ids.is_none()
        && args.tracing_names.is_none()
        && args.tracing_files.is_none()
        && args.tracing_lines.is_none()
        && args.tracing_ansi.is_none()
        && args.tracing_writer.is_none()
    {
        return quote! {};
    }

    // env_filter: None → None. Some(filter_str) → Some(filter_str.to_string()).
    let env_filter = match &args.tracing {
        None => quote! { ::core::option::Option::None },
        Some(expr) => quote! { ::core::option::Option::Some(#expr.to_string()) },
    };
    let field = |opt: &Option<syn::Expr>, default_val: bool| -> proc_macro2::TokenStream {
        match opt {
            None => quote! { #default_val },
            Some(e) => quote! { (#e) },
        }
    };
    let targets = field(&args.tracing_targets, false);
    let thread_ids = field(&args.tracing_ids, true);
    let thread_names = field(&args.tracing_names, false);
    let files = field(&args.tracing_files, false);
    let line_numbers = field(&args.tracing_lines, false);
    let ansi = field(&args.tracing_ansi, false);
    let writer = if let Some(true) = args.tracing_writer {
        quote! { #fc::valtron::trace::TraceWriter::Console }
    } else {
        quote! { #fc::valtron::trace::TraceWriter::Stdout }
    };

    quote! {
        #fc::valtron::trace::try_init_tracing_with(#fc::valtron::trace::TracingTestConfig {
            env_filter: #env_filter,
            targets: #targets,
            thread_ids: #thread_ids,
            thread_names: #thread_names,
            files: #files,
            line_numbers: #line_numbers,
            ansi: #ansi,
            writer: #writer,
        })
    }
}

/// Shared expansion for both macros — see the module walkthrough.
///
/// `is_test` controls the two differences: emitting `#[test]`, and the
/// `Some(3)`-minimum thread default.
#[expect(
    clippy::too_many_lines,
    reason = "one linear expansion that emits the runtime wrapper in order"
)]
fn expand(attr: TokenStream, item: TokenStream, is_test: bool) -> TokenStream {
    let args = match parse_args(attr) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };
    let func = match syn::parse2::<syn::ItemFn>(item) {
        Ok(func) => func,
        Err(err) => return err.to_compile_error(),
    };
    // An `async fn` body is accepted (feature 00-F3): it is driven to completion
    // via `block_on_future` (from_future + run-to-completion), so the body's
    // `.await` points park on the valtron engine (Decision 00 Level 1). A sync fn
    // keeps today's expansion byte-for-byte.
    let is_async = func.sig.asyncness.is_some();
    if !func.sig.inputs.is_empty() {
        return syn::Error::new_spanned(&func.sig.inputs, "#[valtron] functions take no arguments")
            .to_compile_error();
    }

    let fc = foundation_core_path();
    // Defensive: strip a DIRECTLY-adjacent caller-supplied `#[test]` — we emit
    // our own, and two `#[test]` attributes register the test twice (it then
    // runs concurrently and races on the global pool registry → hang).
    //
    // NOTE: this only catches `#[test]` when it sits immediately on the fn with
    // no other proc-macro attr between it and `#[valtron_test]`. Outer attrs
    // (e.g. `#[serial]`) expand first and hoist `#[test]` into a wrapper before
    // we run, so we never see it. The real guarantee is the usage contract in
    // the module docs: `#[valtron_test]` REPLACES `#[test]`; never stack them.
    let attrs: Vec<&syn::Attribute> = func
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("test"))
        .collect();
    let vis = &func.vis;
    // The emitted wrapper fn is ALWAYS synchronous — it owns the engine guard and
    // drives the (possibly async) body to completion. Strip `async` from the
    // signature we re-emit; the body handling below reintroduces the future.
    let mut sig = func.sig.clone();
    sig.asyncness = None;
    let block = &func.block;
    let output = &func.sig.output; // inner fn keeps the SAME return type
    let inner = format_ident!("__valtron_body_{}", func.sig.ident);

    // Sync body: the original block moved into an inner fn (so `return`/`?` keep
    // their exact meaning) which is called directly. Async body: no inner fn — the
    // block becomes a future run to completion by `block_on_future`, so `.await`
    // parks on the engine and `return`/`?` exit the future (whose output is the
    // fn's return value).
    let (inner_def, run_call) = if is_async {
        (
            quote! {},
            quote! { #fc::valtron::__private::block_on_future(async move #block) },
        )
    } else {
        (
            quote! {
                #[allow(clippy::items_after_statements)]
                fn #inner() #output #block
            },
            quote! { #inner() },
        )
    };

    // Pre-compute the tracing init call BEFORE any moves out of `args`
    // (seed/threads below consume those fields via map_or_else / match).
    let tracing_init = expand_tracing_init(&args, &fc, is_test);

    // Seed: explicit expression, or a RandomState-derived u64 (no rand dep —
    // see the module docs). `hash_one` is BuildHasher's one-shot hashing API.
    let seed = args.seed.map_or_else(
        || {
            quote! {
                ::std::hash::BuildHasher::hash_one(
                    &::std::collections::hash_map::RandomState::new(),
                    0x0045_5745_u64, // "EWE" in ASCII — any constant works; RandomState supplies the entropy
                )
            }
        },
        |expr| quote! { (#expr) },
    );

    // Threads: the engine wants an `Option<usize>` worker count. The rule is
    // simple and the SAME for both macros — SUPPLIED → `Some(n)`, ABSENT → `None`
    // (engine default).
    let threads = match args.threads {
        None => quote! { ::core::option::Option::None },
        Some(expr) => quote! { ::core::option::Option::Some((#expr) as usize) },
    };

    // Slow-poll watchdog: on by default in BOTH macros, because a blocking
    // call in a task body holds the worker and starves the pool with no trace.
    // 100 ms is ten times the executor's 10 ms readiness quantum, and well under
    // the 2 s peer bound of the 2026-11-06 failure. `slow_poll = ms` overrides.
    let slow_poll = match &args.slow_poll {
        None => quote! { ::core::option::Option::Some(std::time::Duration::from_millis(100)) },
        Some(expr) => quote! {
            ::core::option::Option::Some(std::time::Duration::from_millis((#expr) as u64))
        },
    };

    // `ctx = expr`: the engine root. Absent → the expansion is the same as
    // before this argument existed. With `timeout`, the body runs on a new
    // thread that must bring its own (single-threaded) engine up, so bind the
    // root once and clone it for the outer pool.
    let ctx_expr = args.ctx.as_ref();
    let ctx_binding = match ctx_expr {
        None => quote! {},
        Some(expr) => quote! { let __valtron_ctx: #fc::valtron::Ctx = (#expr); },
    };
    let init_pool_direct = match ctx_expr {
        // The root RNG takes the pool seed (`Ctx::with_seed`), so one seed
        // drives the whole run. Bind the seed once: the default seed
        // expression draws a new value each time it runs.
        None => quote! {
            {
                let __valtron_seed: u64 = #seed;
                #fc::valtron::initialize_pool_with_slow_poll_warning(
                    __valtron_seed,
                    #threads,
                    #slow_poll,
                    #fc::valtron::Ctx::with_seed(
                        ::std::sync::Arc::new(#fc::valtron::MonotonicClock::new()),
                        __valtron_seed,
                    ),
                )
            }
        },
        Some(expr) => quote! {
            #fc::valtron::initialize_pool_with_slow_poll_warning(
                #seed,
                #threads,
                #slow_poll,
                (#expr),
            )
        },
    };
    let init_pool_timeout = if ctx_expr.is_some() {
        quote! {
            #fc::valtron::initialize_pool_with_slow_poll_warning(
                __valtron_seed,
                #threads,
                #slow_poll,
                __valtron_ctx.clone(),
            )
        }
    } else {
        quote! {
            #fc::valtron::initialize_pool_with_slow_poll_warning(
                __valtron_seed,
                #threads,
                #slow_poll,
                #fc::valtron::Ctx::with_seed(
                    ::std::sync::Arc::new(#fc::valtron::MonotonicClock::new()),
                    __valtron_seed,
                ),
            )
        }
    };
    let init_current_thread_timeout = if ctx_expr.is_some() {
        quote! {
            #fc::valtron::initialize_current_thread_with_slow_poll_warning(
                __valtron_seed,
                #slow_poll,
                __valtron_ctx,
            )
        }
    } else {
        quote! {
            #fc::valtron::initialize_current_thread_with_slow_poll_warning(
                __valtron_seed,
                #slow_poll,
                #fc::valtron::Ctx::with_seed(
                    ::std::sync::Arc::new(#fc::valtron::MonotonicClock::new()),
                    __valtron_seed,
                ),
            )
        }
    };

    let test_attr = if is_test {
        quote! { #[test] }
    } else {
        quote! {}
    };

    // After a timeout the pool is torn down with a deadline (see the
    // `Timeout` arm below): an explicit `shutdown_grace = ms`, or the documented
    // default in foundation_core.
    let shutdown_grace = args.shutdown_grace.as_ref().map_or_else(
        || quote! { #fc::valtron::DEFAULT_TEST_SHUTDOWN_GRACE },
        |expr| quote! { std::time::Duration::from_millis(#expr) },
    );

    // Timeout: only when explicitly set — spawn the inner fn in a thread and
    // wait with recv_timeout. Without timeout, direct call (no thread overhead).
    let body_with_timeout = if let Some(timeout_ms) = &args.timeout {
        quote! {
            {
                // Sync: the original body as an inner fn (`return`/`?` keep their exact
                // meaning). Async: empty — the body is driven via `run_call` below.
                #inner_def

                // Before `initialize_pool` — workers pin the ambient dispatcher at
                // spawn time, so a later init leaves every worker thread silent.
                #tracing_init;
                let __valtron_seed: u64 = #seed;
                #ctx_binding
                let __valtron_guard = #init_pool_timeout;
                let __timeout_ms: u64 = #timeout_ms;
                type __PanicPayload = std::boxed::Box<dyn std::any::Any + std::marker::Send + 'static>;
                let (__sender, __receiver) = std::sync::mpsc::channel::<std::result::Result<_, __PanicPayload>>();
                std::thread::spawn(move || {
                    // The body runs on this thread. Without `multi` the engine is
                    // thread-local, so bring it up here too (a no-op under `multi`).
                    #init_current_thread_timeout;
                    let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #run_call
                    }));
                    let _ = __sender.send(panic_result);
                });
                // The guard lives in this block scope, so it drops after the match
                // has produced its value, or during the unwind of a panic arm. It
                // is never dropped with `mem::drop`: under the single-threaded
                // build that is a no-op that `clippy::drop_non_drop` reports.
                match __receiver.recv_timeout(std::time::Duration::from_millis(__timeout_ms)) {
                    std::result::Result::Ok(std::result::Result::Ok(t)) => t,
                    std::result::Result::Ok(std::result::Result::Err(payload)) => {
                        std::panic::resume_unwind(payload);
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // The body is hung, often because pool workers are blocked
                        // (a deadlock, a blocking syscall). Dropping the guard would
                        // wait for those workers forever and keep the process-wide
                        // pool gate shut, hanging every later test too. Tear the pool
                        // down with a deadline instead: stuck workers (and the body
                        // thread) are detached, and the gate is released on drop.
                        let __stopped = __valtron_guard.shutdown_within(#shutdown_grace);
                        panic!(
                            "timeout: the test did not finish within {} ms.{}",
                            __timeout_ms,
                            if __stopped { "" } else { " Pool workers were still blocked and were detached." },
                        );
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        panic!("timeout: test thread disconnected unexpectedly");
                    },
                }
            }
        }
    } else {
        quote! {
            // Sync: the original body as an inner fn (`return`/`?` keep their exact
            // meaning). Async: empty — the body is driven via `run_call` below.
            #inner_def

            // Wire up tracing so #[valtron_test] diagnostics land on stderr.
            // Idempotent — safe to call in every test. Macro args like
            // `tracing = "debug"` and `tracing_targets = true` feed the config.
            //
            // MUST precede `initialize_pool`: worker threads capture the ambient
            // dispatcher at spawn time and pin it thread-locally for their whole
            // life. Spawning first pins the NO-OP dispatcher, and a thread-local
            // default beats the global one — so every worker would go silent no
            // matter what subscriber is installed afterwards.
            #tracing_init;

            // Engine up inside a block scope: the guard is a NAMED local in the
            // block, so it provably lives across the whole body (a `let _ =` would
            // drop it immediately and kill the pool before anything ran) and drops
            // when the block ends, AFTER the body has returned. No `mem::drop`:
            // without feature "multi" the guard has no `Drop` impl, and that call
            // is the no-op `clippy::drop_non_drop` reports.
            {
                let __valtron_guard = #init_pool_direct;

                // Sync: call the inner fn. Async: drive the body future to
                // completion (`.await` points park on the engine — Decision 00).
                #run_call
            }
        }
    };

    quote! {
        #test_attr
        #(#attrs)*
        #vis #sig {
            #body_with_timeout
        }
    }
}

/// `#[valtron]` — see the module walkthrough.
pub(crate) fn valtron(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(attr, item, false)
}

/// `#[valtron_test]` — see the module walkthrough.
pub(crate) fn valtron_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(attr, item, true)
}
