//! WHY: Every wasm entry point (a Cloudflare Worker `fetch`, a `main`) had to
//! hand-write the same tracing subscriber + `console.log` writer — and the
//! `tracing-wasm` default freezes workerd (error 1101). `#[wasm_logs]` removes
//! that ceremony.
//!
//! WHAT: An attribute macro that injects one idempotent tracing-init call at the
//! top of the annotated function body. The `console` backend forwards events to
//! `console.log` (visible in `wrangler tail`); `stdout` writes to standard
//! output. It accepts the same `tracing_*` knobs as `#[valtron]`.
//!
//! HOW: `#[wasm_logs(console, tracing = "debug")]` expands to a call to
//! `foundation_core::valtron::trace::try_init_tracing_with(TracingTestConfig { … })`
//! prepended to the body. The init is `Once`-guarded inside `foundation_compact`,
//! so a worker can call it on every request without cost.
//!
//! # Examples
//!
//! ```ignore
//! use foundation_macros::wasm_logs;
//!
//! #[wasm_logs(console)]
//! pub async fn fetch(req: web_sys::Request) -> web_sys::Response {
//!     // tracing::info!(…) now lands in `console.log`.
//! }
//!
//! #[wasm_logs(stdout, tracing = "info", tracing_ansi = true)]
//! fn main() { /* … */ }
//! ```

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;

use crate::crate_paths::foundation_core_path;

/// Parsed `#[wasm_logs(...)]` arguments.
struct Args {
    /// The trace backend: `stdout` or `console`. Defaults to `console`.
    console: bool,
    tracing: Option<syn::Expr>,
    tracing_targets: Option<syn::Expr>,
    tracing_ids: Option<syn::Expr>,
    tracing_names: Option<syn::Expr>,
    tracing_files: Option<syn::Expr>,
    tracing_lines: Option<syn::Expr>,
    tracing_ansi: Option<syn::Expr>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            console: true,
            tracing: None,
            tracing_targets: None,
            tracing_ids: None,
            tracing_names: None,
            tracing_files: None,
            tracing_lines: None,
            tracing_ansi: None,
        }
    }
}

const KNOWN_ARGS: &[&str] = &[
    "tracing", "tracing_targets", "tracing_ids", "tracing_names",
    "tracing_files", "tracing_lines", "tracing_ansi",
];

fn parse_args(attr: TokenStream) -> Result<Args, syn::Error> {
    let mut args = Args::default();
    if attr.is_empty() {
        return Ok(args);
    }
    let metas =
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
            .parse2(attr)?;
    for meta in metas {
        match meta {
            // A bare `console` / `stdout` selects the backend.
            syn::Meta::Path(path) if path.is_ident("console") => args.console = true,
            syn::Meta::Path(path) if path.is_ident("stdout") => args.console = false,
            syn::Meta::Path(path) => {
                return Err(syn::Error::new_spanned(
                    &path,
                    "expected `stdout` or `console` as the backend, or a `tracing_*` key",
                ))
            }
            syn::Meta::NameValue(pair) => {
                let name = pair
                    .path
                    .get_ident()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                match name.as_str() {
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
            syn::Meta::List(list) => {
                return Err(syn::Error::new_spanned(
                    &list.path,
                    "expected `stdout`/`console` and `key = value` pairs, not a list",
                ))
            }
        }
    }
    Ok(args)
}

/// Build the `try_init_tracing_with(TracingTestConfig { … })` statement.
fn expand_init(args: &Args, fc: &TokenStream) -> TokenStream {
    let writer = if args.console {
        quote! { #fc::valtron::trace::TraceWriter::Console }
    } else {
        quote! { #fc::valtron::trace::TraceWriter::Stdout }
    };

    let env_filter = match &args.tracing {
        None => quote! { ::core::option::Option::None },
        Some(expr) => quote! { ::core::option::Option::Some(#expr.to_string()) },
    };
    let field = |opt: &Option<syn::Expr>, default_val: bool| -> TokenStream {
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
            writer: #writer,
        });
    }
}

pub(crate) fn wasm_logs(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match parse_args(attr) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };
    let mut func = match syn::parse2::<syn::ItemFn>(item) {
        Ok(func) => func,
        Err(err) => return err.to_compile_error(),
    };

    let fc = foundation_core_path();
    let init = expand_init(&args, &fc);
    let stmt: syn::Stmt =
        syn::parse2(init).expect("the generated tracing init is always a valid statement");
    func.block.stmts.insert(0, stmt);

    quote! { #func }
}

// Marker-validation tests live in-file (proc-macro crates can't export hooks).
#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn err(out: &TokenStream) -> String {
        let s = out.to_string();
        assert!(s.contains("compile_error"), "expected an error, got: {s}");
        s
    }

    /// Strip all whitespace so `::`, `:` and `()` spacing in `quote!` output does
    /// not leak into the assertions.
    fn tight(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }

    #[test]
    fn console_defaults_and_backend_parse() {
        let out = wasm_logs(quote! {}, quote! { fn f() {} });
        assert!(tight(&out.to_string()).contains("TraceWriter::Console"));
        assert!(!out.to_string().contains("compile_error"));

        let out = wasm_logs(quote! { stdout }, quote! { fn f() {} });
        assert!(tight(&out.to_string()).contains("TraceWriter::Stdout"));

        let out = wasm_logs(quote! { console }, quote! { fn f() {} });
        assert!(tight(&out.to_string()).contains("TraceWriter::Console"));
    }

    #[test]
    fn passes_tracing_knobs_through() {
        let out = wasm_logs(
            quote! { console, tracing = "debug", tracing_targets = true, tracing_ansi = true },
            quote! { async fn f() {} },
        );
        let s = tight(&out.to_string());
        assert!(s.contains("Some(\"debug\".to_string())"));
        assert!(s.contains("targets:(true)"));
        assert!(s.contains("ansi:(true)"));
        assert!(s.contains("asyncfnf"));
        assert!(!out.to_string().contains("compile_error"));
    }

    #[test]
    fn injects_init_into_body() {
        let out = wasm_logs(quote! { console }, quote! { fn f() { let x = 1; } });
        let s = out.to_string();
        let init = s.find("try_init_tracing_with").expect("init call present");
        let body = s.find("let x = 1").expect("body present");
        assert!(init < body, "init must precede the body statement");
    }

    #[test]
    fn bad_backend_rejected() {
        let out = err(&wasm_logs(quote! { file }, quote! { fn f() {} }));
        assert!(out.contains("stdout"));
    }

    #[test]
    fn bad_key_rejected() {
        let out = err(&wasm_logs(quote! { tracing_file = true }, quote! { fn f() {} }));
        assert!(out.contains("unknown argument"));
    }
}
