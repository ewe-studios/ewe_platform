//! WHY: Feature 13 — the owned wasm test-execution model. Cases must be plain Rust
//! functions that compile into discoverable, runnable exports WITHOUT wasm-bindgen.
//!
//! WHAT: The `#[wasm_test]` attribute body: keeps the original fn, emits a
//! `#[no_mangle] extern "C" fn __fwt_<name>() -> u32` wrapper, and contributes a
//! manifest line to the `__fwt_manifest` custom section. Entry points live in
//! `lib.rs` (proc-macro fns must sit at the crate root).
//!
//! HOW: read the walkthrough below — it covers the full lifecycle from attribute
//! to runner verdict, including the `__FWT_META_*` "map" trick the manifest is
//! built on.
//!
//! ---
//!
//! # The full lifecycle, end to end
//!
//! Writing this in a `cdylib` test crate:
//!
//! ```ignore
//! #[wasm_test]
//! fn adds_up() { assert_eq!(2 + 2, 4); }
//! ```
//!
//! expands to (simplified):
//!
//! ```ignore
//! // 1. The original function, untouched — still callable by other code.
//! fn adds_up() { assert_eq!(2 + 2, 4); }
//!
//! // 2. The RUNNABLE form: a C-ABI export the JS runner can call by name.
//! //    Only exists on wasm targets — native builds of the same crate get
//! //    neither the export nor the manifest entry.
//! #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
//! #[no_mangle]
//! pub extern "C" fn __fwt_adds_up() -> u32 {
//!     foundation_wasm::testing::enter_case("adds_up");  // who's running (for the hook)
//!     /* install the crate-wide panic hook, once */
//!     adds_up();                                        // the actual test body
//!     foundation_wasm::testing::pass("adds_up");        // verdict over host_report
//!     0                                                 // 0 = "already reported"
//! }
//!
//! // 3. The DISCOVERABLE form: one manifest line in a custom section.
//! #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
//! #[used]
//! #[link_section = "__fwt_manifest"]
//! static __FWT_META_ADDS_UP: [u8; 9] = *b"adds_up|\n";
//! ```
//!
//! Three artifacts per case, three consumers:
//!
//! | Artifact | Consumer | Purpose |
//! |---|---|---|
//! | the original `fn` | other Rust code | the case stays an ordinary function |
//! | `__fwt_<name>` export | the JS runner (`runner.mjs`) | callable entry point |
//! | `__FWT_META_<NAME>` static | testbed discovery (`fwt::discover_cases`) | name + flags WITHOUT running anything |
//!
//! # The `__FWT_META_*` manifest "map" — how it actually works
//!
//! There is no collection literal anywhere; the "map" is assembled by the
//! LINKER. Each case emits one `static` byte array holding a single line of
//! text, `name|flags\n`, tagged with two attributes:
//!
//! - **`#[link_section = "__fwt_manifest"]`** — on wasm targets, `rustc`/`lld`
//!   place the static's bytes into a *custom section* named `__fwt_manifest`
//!   inside the final `.wasm` binary. Custom sections are a first-class wasm
//!   concept: named, opaque byte blobs that the runtime ignores entirely but
//!   any tool can read (`WebAssembly.Module.customSections` in JS, our wasmbin
//!   port in Rust). When MANY statics across MANY object files share one
//!   section name, the linker CONCATENATES their bytes into a single section —
//!   so five `#[wasm_test]` cases produce ONE section whose payload is five
//!   newline-terminated lines:
//!
//!   ```text
//!   ignored_case|i\npasses_simple|\npanics_as_expected|p\n…
//!   ```
//!
//!   This is the same mechanism wasm-bindgen uses for its own metadata; we use
//!   it with our section name and a deliberately trivial line format.
//!
//! - **`#[used]`** — a static that no Rust code reads is dead code, and the
//!   linker would strip it (taking its section bytes along). `#[used]` forces
//!   the symbol to survive anyway. Without it the manifest silently vanishes
//!   under optimization.
//!
//! The line format is `name|flags\n`, where `flags` is a (possibly empty)
//! string of single-character markers:
//!
//! | flag | meaning | who acts on it |
//! |---|---|---|
//! | `a` | `async fn` — the export returns `1` ("report pending"); the verdict arrives later via `host_report` | the runner awaits `TestReports.next()` |
//! | `p` | `#[wasm_test(should_panic)]` — the expected outcome IS a panic | the runner INVERTS the verdict (see below) |
//! | `i` | `#[wasm_test(ignore)]` — report "ignored" without running the body | the runner counts it as skipped |
//!
//! Concatenation order is whatever the linker chooses, so consumers must treat
//! the section as an UNORDERED set of lines — discovery
//! (`foundation_wasm_testbed::fwt`) parses it into a name→flags map and sorts
//! cases by name. The EXPORT scan is authoritative for which cases exist; the
//! manifest only enriches them (a case missing a manifest line just gets empty
//! flags).
//!
//! # Why the wrapper is shaped the way it is
//!
//! **Panics abort on wasm32.** There is no unwinding, so `catch_unwind` cannot
//! turn a failed assertion into a `Result` — the instance traps and is dead.
//! The wrapper therefore installs (once per crate, `std::sync::Once`-guarded) a
//! `std::panic::set_hook` whose closure ships the panic text over the owned ABI
//! (`testing::fail_current` → `host_report`) BEFORE the trap. The hook lives in
//! the GENERATED code rather than in `foundation_wasm` because the hook API is
//! `std`-only and `foundation_wasm` is `no_std` — test crates are always `std`
//! cdylibs. Sequencing on a panic:
//!
//! ```text
//! assert fails → panic hook runs → host_report(FAIL, "panicked at …") → abort trap
//!                                  (JS receives the report)             (JS catches the RuntimeError)
//! ```
//!
//! The runner then THROWS AWAY the instance and creates a fresh one for the
//! next case — the only safe continuation after a trap.
//!
//! **`enter_case` runs before anything else.** The panic hook only receives the
//! panic info, not which test was executing; `testing::enter_case(name)`
//! records the current case in a static so `fail_current` can attribute the
//! failure to the right name.
//!
//! **The `u32` return is a tiny handshake.** `0` means "the verdict was already
//! delivered synchronously" (pass, fail, and ignored all report before the
//! export returns). `1` means "async — the verdict arrives via `host_report`
//! when the future resolves", telling the runner to `await` the next report
//! instead of reading a buffered one. Async bodies are driven by
//! `testing::run_async`, which polls with a noop waker and, on `Pending`,
//! re-arms itself through `schedule_timeout(0)` — the owned JS-yield loop.
//!
//! **`should_panic` cannot be checked module-side.** Under abort semantics the
//! module never regains control after its own panic, so the wrapper does
//! nothing special for it; the `p` manifest flag tells the RUNNER to invert:
//! trap + FAIL report = the case passed, a clean PASS report = an
//! "expected a panic" failure.
//!
//! # Path resolution
//!
//! Generated code reaches `foundation_wasm::testing::…` through
//! [`foundation_wasm_path`] (proc-macro-crate): inside `foundation_wasm` itself
//! that resolves to `crate`, in downstream crates to whatever name their
//! `Cargo.toml` gives the dependency — so the macro survives renames.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::crate_paths::foundation_wasm_path;

/// Flags parsed from `#[wasm_test(...)]` arguments. Each maps 1:1 to a manifest
/// flag character (`p`, `i`); `a` (async) is detected from the fn signature, not
/// from an argument.
#[derive(Default)]
struct Flags {
    should_panic: bool,
    ignore: bool,
}

/// Comma-separated bare idents — the `#[wasm_test(...)]` argument grammar.
///
/// A newtype is required because `Punctuated` itself doesn't implement
/// [`syn::parse::Parse`]; `parse_terminated` consumes the whole stream, which
/// is exactly right for an attribute argument list.
struct FlagList(syn::punctuated::Punctuated<syn::Ident, syn::Token![,]>);

impl syn::parse::Parse for FlagList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(syn::punctuated::Punctuated::parse_terminated(input)?))
    }
}

/// Turn the attribute arguments (`should_panic`, `ignore`, or nothing) into
/// [`Flags`], rejecting anything unknown with a SPANNED error so the typo gets
/// underlined at the call site rather than producing a cryptic expansion error.
fn parse_flags(attr: TokenStream) -> Result<Flags, syn::Error> {
    let mut flags = Flags::default();
    if attr.is_empty() {
        return Ok(flags);
    }
    let FlagList(idents) = syn::parse2::<FlagList>(attr).map_err(|err| {
        syn::Error::new(
            err.span(),
            "#[wasm_test(...)] accepts `should_panic` and/or `ignore`",
        )
    })?;
    for ident in idents {
        match ident.to_string().as_str() {
            "should_panic" => flags.should_panic = true,
            "ignore" => flags.ignore = true,
            other => {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("unknown #[wasm_test] flag `{other}` (expected `should_panic` or `ignore`)"),
                ))
            }
        }
    }
    Ok(flags)
}

/// The `#[wasm_test]` expansion — see the module docs for the full walkthrough.
///
/// Steps, in the order they appear below:
/// 1. parse flags + the annotated `fn` (rejecting parameters — a test case takes
///    no input by definition);
/// 2. derive the generated names from the fn name (`__fwt_<name>` export,
///    `__FWT_META_<NAME>` manifest static);
/// 3. build the manifest line (`name|flags\n`) as a byte-string literal whose
///    LENGTH is computed right here at expansion time — a `static [u8; N]`
///    needs a concrete `N`, and the macro is the one place that knows it;
/// 4. pick the invocation body (sync: call + report `pass` + return 0;
///    async: hand the future to `testing::run_async` and forward its 0/1);
/// 5. emit the three artifacts: original fn + export wrapper + manifest static.
pub(crate) fn wasm_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let flags = match parse_flags(attr) {
        Ok(flags) => flags,
        Err(err) => return err.to_compile_error(),
    };
    let case = match syn::parse2::<syn::ItemFn>(item) {
        Ok(case) => case,
        Err(err) => return err.to_compile_error(),
    };
    if !case.sig.inputs.is_empty() {
        return syn::Error::new_spanned(&case.sig.inputs, "#[wasm_test] functions take no arguments")
            .to_compile_error();
    }

    let fw = foundation_wasm_path();
    let name = &case.sig.ident;
    let name_str = name.to_string();
    let is_async = case.sig.asyncness.is_some();
    // The export the runner calls: `my_case` → `__fwt_my_case`. The `__fwt_`
    // prefix is the discovery contract (our counterpart of bindgen's `__wbgt_`).
    let export = format_ident!("__fwt_{name}");
    // The manifest static: `my_case` → `__FWT_META_MY_CASE`. Uppercased purely
    // for static-naming convention; the IDENTIFIER never matters at runtime —
    // discovery reads the section BYTES, not the symbol name.
    let meta_ident = format_ident!("__FWT_META_{}", name_str.to_uppercase());

    // Manifest line: `name|flags\n` (see the flag table in the module docs).
    // The newline is both the line separator AND the terminator — the linker
    // butt-joins every case's bytes into one section, so without the trailing
    // `\n` adjacent lines would merge into garbage.
    let mut manifest_flags = String::new();
    if is_async {
        manifest_flags.push('a');
    }
    if flags.should_panic {
        manifest_flags.push('p');
    }
    if flags.ignore {
        manifest_flags.push('i');
    }
    let manifest_line = format!("{name_str}|{manifest_flags}\n");
    // A byte-string literal (e.g. `*b"my_case|a\n"`) plus its exact length —
    // both are known here at expansion time, which is what lets us declare a
    // fixed-size `[u8; N]` for the static below.
    let manifest_bytes = syn::LitByteStr::new(manifest_line.as_bytes(), name.span());
    let manifest_len = manifest_line.len();

    let ignore = flags.ignore;
    let invoke = if is_async {
        // Async: `#name()` CREATES the future (calling an async fn evaluates to
        // its future without running any of it); run_async owns the polling and
        // re-arms through schedule_timeout(0) while Pending. Its return value
        // IS the handshake: 0 = resolved (and reported) within the first poll,
        // 1 = the report is pending — the runner must await it.
        quote! { #fw::testing::run_async(#name_str, #name()) }
    } else {
        // Sync: if the body returns at all, it passed — a failed assertion
        // would have panicked → hook reported FAIL → abort trap, and execution
        // would never reach the `pass` call below.
        quote! {
            #name();
            #fw::testing::pass(#name_str);
            0
        }
    };

    quote! {
        // (1) The original function — kept verbatim so the case remains
        // ordinary Rust (callable by other code, compilable natively).
        #case

        // (2) The runnable export. wasm-only: native builds of the test crate
        // must not grow C exports or depend on the host ABI.
        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        #[no_mangle]
        pub extern "C" fn #export() -> u32 {
            // Record the running case FIRST — the panic hook reads it to
            // attribute a failure (the hook only receives the panic info).
            #fw::testing::enter_case(#name_str);
            {
                // One panic hook per crate, installed lazily by whichever case
                // runs first (`Once` makes later calls no-ops). The hook is
                // generated HERE — not inside foundation_wasm — because
                // std::panic::set_hook is a `std` API and foundation_wasm is
                // `no_std`; test crates are always std cdylibs.
                //
                // The hook is the ONLY chance to ship a failure message: wasm32
                // panics ABORT (no unwinding), so the moment the hook returns,
                // the instance traps and never executes again. The JS runner
                // catches that trap and re-instantiates for the next case.
                static __FWT_HOOK: ::std::sync::Once = ::std::sync::Once::new();
                __FWT_HOOK.call_once(|| {
                    ::std::panic::set_hook(::std::boxed::Box::new(|info| {
                        #fw::testing::fail_current(&::std::string::ToString::to_string(info));
                    }));
                });
            }
            if #ignore {
                // Ignored cases still REPORT (status 2) so the runner can count
                // them — they just never execute the body.
                #fw::testing::ignored(#name_str);
                return 0;
            }
            #invoke
        }

        // (3) The manifest entry. `link_section` routes the bytes into the
        // `__fwt_manifest` CUSTOM SECTION of the .wasm — the linker
        // concatenates every case's bytes into one section, and that
        // concatenation IS the manifest "map". `used` stops the
        // otherwise-unreferenced static from being stripped (which would
        // silently delete the manifest). wasm-only: ELF/Mach-O section
        // semantics differ and nothing reads the manifest natively.
        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        #[used]
        #[link_section = "__fwt_manifest"]
        static #meta_ident: [u8; #manifest_len] = *#manifest_bytes;
    }
}
