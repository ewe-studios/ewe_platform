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
//!     // Engine up. `initialize_pool` is the unified entry — it picks the
//!     // multi-threaded pool (feature "multi") or the single-threaded/wasm pool
//!     // automatically, and returns the matching PoolGuard.
//!     let __valtron_guard = ::foundation_core::valtron::initialize_pool(
//!         /* seed */    …,        // explicit `seed = N`, or a RandomState-derived u64
//!         /* threads */ None,     // explicit `threads = N`, or the engine default
//!     );
//!
//!     // Run the user's code WHILE the guard lives…
//!     let __valtron_out = __valtron_body();
//!
//!     // …and only shut the engine down (PoolGuard::drop joins/kills the pool
//!     // threads under feature "multi") AFTER the body has fully returned.
//!     ::core::mem::drop(__valtron_guard);
//!     __valtron_out
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
//! # Why the guard handling is the whole point
//!
//! `initialize_pool` returns a `PoolGuard` whose `Drop` tears the engine down.
//! The classic footgun is `let _ = initialize_pool(…)` — the guard drops on THAT
//! LINE and the pool dies before any task runs. The macro makes the correct
//! lifetime structural: the guard is a named local that provably outlives the
//! entire body, and is dropped explicitly afterwards (rather than relying on
//! scope order) so the sequencing is visible in the expansion.
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
}

const KNOWN_ARGS: &[&str] = &[
    "seed", "threads", "timeout",
    "tracing", "tracing_targets", "tracing_ids",
    "tracing_names", "tracing_files", "tracing_lines", "tracing_ansi",
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
            "expected `key = value` pairs (seed, threads, timeout, tracing, tracing_targets, tracing_ids, tracing_names, tracing_files, tracing_lines, tracing_ansi)",
        )
    })?;
    for pair in pairs {
        let name = pair
            .path
            .get_ident()
            .map(ToString::to_string)
            .unwrap_or_default();
        match name.as_str() {
            "seed" => args.seed = Some(pair.value),
            "threads" => args.threads = Some(pair.value),
            "timeout" => args.timeout = Some(pair.value),
            "tracing" => args.tracing = Some(pair.value),
            "tracing_targets" => args.tracing_targets = Some(pair.value),
            "tracing_ids" => args.tracing_ids = Some(pair.value),
            "tracing_names" => args.tracing_names = Some(pair.value),
            "tracing_files" => args.tracing_files = Some(pair.value),
            "tracing_lines" => args.tracing_lines = Some(pair.value),
            "tracing_ansi" => args.tracing_ansi = Some(pair.value),
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
/// For `#[valtron_test]` (is_test=true): tracing is always on (defaults to "info").
/// For `#[valtron]` (is_test=false): tracing is OFF unless explicitly requested
/// via `tracing = "debug"` or similar.
fn expand_tracing_init(args: &Args, fc: &proc_macro2::TokenStream, is_test: bool) -> proc_macro2::TokenStream {
    // For #[valtron] (non-test), skip tracing unless explicitly requested.
    // For #[valtron_test], always initialize with defaults.
    if !is_test && args.tracing.is_none()
        && args.tracing_targets.is_none()
        && args.tracing_ids.is_none()
        && args.tracing_names.is_none()
        && args.tracing_files.is_none()
        && args.tracing_lines.is_none()
        && args.tracing_ansi.is_none()
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

    quote! {
        #fc::valtron::trace::try_init_tracing_with(#fc::valtron::trace::TracingTestConfig {
            env_filter: #env_filter,
            targets: #targets,
            thread_ids: #thread_ids,
            thread_names: #thread_names,
            files: #files,
            line_numbers: #line_numbers,
            ansi: #ansi,
        })
    }
}

/// Shared expansion for both macros — see the module walkthrough.
///
/// `is_test` controls the two differences: emitting `#[test]`, and the
/// `Some(3)`-minimum thread default.
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
        return syn::Error::new_spanned(
            &func.sig.inputs,
            "#[valtron] functions take no arguments",
        )
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
            quote! { #fc::valtron::block_on_future(async move #block) },
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

    let test_attr = if is_test {
        quote! { #[test] }
    } else {
        quote! {}
    };

    // Timeout: only when explicitly set — spawn the inner fn in a thread and
    // wait with recv_timeout. Without timeout, direct call (no thread overhead).
    let body_with_timeout = if let Some(timeout_ms) = &args.timeout {
        quote! {
            // Sync: the original body as an inner fn (`return`/`?` keep their exact
            // meaning). Async: empty — the body is driven via `run_call` below.
            #inner_def

            // Before `initialize_pool` — workers pin the ambient dispatcher at
            // spawn time, so a later init leaves every worker thread silent.
            #tracing_init;
            let __valtron_guard = #fc::valtron::initialize_pool(#seed, #threads);
            let __timeout_start = std::time::Instant::now();
            type __PanicPayload = std::boxed::Box<dyn std::any::Any + std::marker::Send + 'static>;
            let (__sender, __receiver) = std::sync::mpsc::channel::<std::result::Result<_, __PanicPayload>>();
            std::thread::spawn(move || {
                let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #run_call
                }));
                let _ = __sender.send(panic_result);
            });
            match __receiver.recv_timeout(std::time::Duration::from_millis(#timeout_ms)) {
                std::result::Result::Ok(std::result::Result::Ok(t)) => {
                    ::core::mem::drop(__valtron_guard);
                    t
                },
                std::result::Result::Ok(std::result::Result::Err(payload)) => {
                    ::core::mem::drop(__valtron_guard);
                    std::panic::resume_unwind(payload);
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    panic!("timeout: the test took {} ms. Max {} ms", __timeout_start.elapsed().as_millis(), #timeout_ms);
                },
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("timeout: test thread disconnected unexpectedly");
                },
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

            // Engine up — the guard is a NAMED local so it provably lives across
            // the whole body (a `let _ =` would drop it immediately and kill the
            // pool before anything ran).
            let __valtron_guard = #fc::valtron::initialize_pool(#seed, #threads);

            // Sync: call the inner fn. Async: drive the body future to completion
            // (`.await` points park on the engine — Decision 00).
            let __valtron_out = #run_call;

            // Engine down — explicit, AFTER the body has fully returned, so the
            // shutdown ordering is visible rather than implied by scope.
            ::core::mem::drop(__valtron_guard);
            __valtron_out
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
