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
}

/// Argument grammar: a comma-separated list of `name = expr` pairs, where name ∈
/// {`seed`, `threads`}. A newtype because `Punctuated` doesn't implement `Parse`.
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
            "expected `seed = <u64 expr>` and/or `threads = <usize expr>`",
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
            other => {
                return Err(syn::Error::new_spanned(
                    &pair.path,
                    format!("unknown argument `{other}` (expected `seed` or `threads`)"),
                ))
            }
        }
    }
    Ok(args)
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
    // The valtron engine is its own scheduler — an `async fn` here would imply a
    // futures executor we are not providing. Reject loudly instead of producing
    // a confusing type error inside the expansion.
    if let Some(asyncness) = &func.sig.asyncness {
        return syn::Error::new_spanned(
            asyncness,
            "#[valtron] functions are synchronous — the valtron engine schedules its own tasks \
             (spawn(...).schedule() inside the body); remove `async`",
        )
        .to_compile_error();
    }
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
    let sig = &func.sig;
    let block = &func.block;
    let output = &func.sig.output; // inner fn keeps the SAME return type
    let inner = format_ident!("__valtron_body_{}", func.sig.ident);

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

    quote! {
        #test_attr
        #(#attrs)*
        #vis #sig {
            // The original body as an inner fn: `return`/`?` keep their exact
            // meaning (they exit THIS fn), unlike a closure wrapper.
            #[allow(clippy::items_after_statements)]
            fn #inner() #output #block

            // Engine up — the guard is a NAMED local so it provably lives across
            // the whole body (a `let _ =` would drop it immediately and kill the
            // pool before anything ran).
            let __valtron_guard = #fc::valtron::initialize_pool(#seed, #threads);

            let __valtron_out = #inner();

            // Engine down — explicit, AFTER the body has fully returned, so the
            // shutdown ordering is visible rather than implied by scope.
            ::core::mem::drop(__valtron_guard);
            __valtron_out
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
