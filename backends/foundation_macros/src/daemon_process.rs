//! `#[daemon_process]`, `daemons!`, and `#[daemon_main]` (spec-58 F01).
//!
//! Three ways to declare a set of daemons, all lowering to the same
//! `foundation_nativeapis::daemon::DaemonDef` builder chain:
//!
//! - `#[daemon_process({ ... }, { ... })]` — attribute on a fn taking a
//!   `&DaemonGroup`; boots the daemons before the body and drops the group after.
//!   Assumes the valtron pool is already initialised (pairs with `#[valtron_test]`
//!   or `#[daemon_main]`), mirroring `#[docker_container]`.
//! - `daemons! { { ... }, { ... } }` — function-like macro producing a
//!   `Vec<DaemonDef>` inline anywhere. Implemented as a proc macro (a proc-macro
//!   crate cannot export `macro_rules!`).
//! - `#[daemon_main]` — entry point: initialises the valtron pool, loads
//!   `daemon.toml` (or `config = "path"`), boots, hands the body a `&DaemonGroup`.
//!
//! The parser is hand-rolled `key = value` (like `#[docker_container]`) because
//! keys such as `run` read best as bare idents and duplicate `name`s must be a
//! compile error.

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Expr, ExprLit, Ident, ItemFn, Lit, LitStr, Token,
};

use crate::crate_paths::{foundation_core_path, foundation_nativeapis_path};

/// One readiness strategy, parsed from a `readiness_*` key.
enum Readiness {
    Port(u16),
    Http(String),
    Delay(u64),
    Output(String),
    Cmd(Vec<String>),
}

/// A single daemon definition parsed from a `{ ... }` block.
struct DaemonDefAttr {
    name: Option<String>,
    run: Vec<String>,
    depends: Vec<String>,
    readiness: Option<Readiness>,
    env: Vec<(String, String)>,
    must_env: Vec<String>,
    cwd: Option<String>,
    watch: Vec<String>,
    memory_limit: Option<String>,
    cpu_limit: Option<u32>,
    restart: Option<bool>,
    max_restarts: Option<u32>,
    stop_timeout: Option<u64>,
    boot_start: bool,
}

impl Parse for DaemonDefAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut def = DaemonDefAttr {
            name: None,
            run: Vec::new(),
            depends: Vec::new(),
            readiness: None,
            env: Vec::new(),
            must_env: Vec::new(),
            cwd: None,
            watch: Vec::new(),
            memory_limit: None,
            cpu_limit: None,
            restart: None,
            max_restarts: None,
            stop_timeout: None,
            boot_start: false,
        };

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            let key = ident.to_string();
            let key_span = ident.span();
            input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;

            match key.as_str() {
                "name" => def.name = Some(parse_string(&value)?),
                "run" => def.run = parse_string_array(&value)?,
                "depends" => def.depends = parse_string_array(&value)?,
                "env" => def.env.extend(parse_env_list(&value)?),
                "must_env" => def.must_env = parse_string_array(&value)?,
                "cwd" => def.cwd = Some(parse_string(&value)?),
                "watch" => def.watch = parse_string_array(&value)?,
                "memory_limit" => def.memory_limit = Some(parse_string(&value)?),
                "cpu_limit" => def.cpu_limit = Some(parse_u32(&value)?),
                "restart" => def.restart = Some(parse_bool(&value)?),
                "max_restarts" => def.max_restarts = Some(parse_u32(&value)?),
                "stop_timeout" => def.stop_timeout = Some(parse_u64(&value)?),
                "boot_start" => def.boot_start = parse_bool(&value)?,
                "readiness_port" => {
                    set_readiness(&mut def, key_span, Readiness::Port(parse_u16(&value)?))?;
                }
                "readiness_http" => {
                    set_readiness(&mut def, key_span, Readiness::Http(parse_string(&value)?))?;
                }
                "readiness_delay" => {
                    set_readiness(&mut def, key_span, Readiness::Delay(parse_u64(&value)?))?;
                }
                "readiness_output" => {
                    set_readiness(&mut def, key_span, Readiness::Output(parse_string(&value)?))?;
                }
                "readiness_cmd" => {
                    set_readiness(&mut def, key_span, Readiness::Cmd(parse_string_array(&value)?))?;
                }
                other => {
                    return Err(syn::Error::new(
                        key_span,
                        format!("unknown daemon attribute: {other}"),
                    ));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        if def.name.is_none() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "daemon block requires `name = \"...\"`",
            ));
        }
        if def.run.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "daemon block requires a non-empty `run = [...]`",
            ));
        }
        Ok(def)
    }
}

fn set_readiness(
    def: &mut DaemonDefAttr,
    span: proc_macro2::Span,
    readiness: Readiness,
) -> syn::Result<()> {
    if def.readiness.is_some() {
        return Err(syn::Error::new(
            span,
            "only one `readiness_*` strategy may be set per daemon",
        ));
    }
    def.readiness = Some(readiness);
    Ok(())
}

/// Every daemon declared in one invocation, in the order written.
struct Attr {
    defs: Vec<DaemonDefAttr>,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut defs = Vec::new();

        if input.peek(syn::token::Brace) {
            while !input.is_empty() {
                let content;
                syn::braced!(content in input);
                defs.push(content.parse::<DaemonDefAttr>()?);
                if input.is_empty() {
                    break;
                }
                input.parse::<Token![,]>()?;
            }
        } else {
            defs.push(input.parse::<DaemonDefAttr>()?);
        }

        if defs.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "expected at least one daemon definition",
            ));
        }

        // Duplicate `name` within one invocation is a compile error.
        let mut seen: HashMap<&str, ()> = HashMap::new();
        for def in &defs {
            if let Some(name) = &def.name {
                if seen.insert(name.as_str(), ()).is_some() {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        format!(
                            "duplicate daemon name {name:?}: each `name = \"...\"` must be \
                             unique within one invocation"
                        ),
                    ));
                }
            }
        }

        Ok(Attr { defs })
    }
}

// ---------------------------------------------------------------------------
// Value parsers (shared with the block parser)
// ---------------------------------------------------------------------------

fn parse_string(expr: &Expr) -> syn::Result<String> {
    if let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = expr {
        Ok(s.value())
    } else {
        Err(syn::Error::new_spanned(expr, "expected string literal"))
    }
}

fn parse_u16(expr: &Expr) -> syn::Result<u16> {
    if let Expr::Lit(ExprLit { lit: Lit::Int(n), .. }) = expr {
        n.base10_parse()
    } else {
        Err(syn::Error::new_spanned(expr, "expected integer literal"))
    }
}

fn parse_u32(expr: &Expr) -> syn::Result<u32> {
    if let Expr::Lit(ExprLit { lit: Lit::Int(n), .. }) = expr {
        n.base10_parse()
    } else {
        Err(syn::Error::new_spanned(expr, "expected integer literal"))
    }
}

fn parse_u64(expr: &Expr) -> syn::Result<u64> {
    if let Expr::Lit(ExprLit { lit: Lit::Int(n), .. }) = expr {
        n.base10_parse()
    } else {
        Err(syn::Error::new_spanned(expr, "expected integer literal"))
    }
}

fn parse_bool(expr: &Expr) -> syn::Result<bool> {
    if let Expr::Lit(ExprLit { lit: Lit::Bool(b), .. }) = expr {
        Ok(b.value())
    } else {
        Err(syn::Error::new_spanned(expr, "expected `true` or `false`"))
    }
}

fn parse_string_array(expr: &Expr) -> syn::Result<Vec<String>> {
    match expr {
        Expr::Array(arr) => arr.elems.iter().map(parse_string).collect(),
        _ => Err(syn::Error::new_spanned(
            expr,
            r#"expected an array of strings, e.g. `["a", "b"]`"#,
        )),
    }
}

fn parse_string_pair(expr: &Expr) -> syn::Result<(String, String)> {
    match expr {
        Expr::Tuple(t) if t.elems.len() == 2 => {
            Ok((parse_string(&t.elems[0])?, parse_string(&t.elems[1])?))
        }
        _ => Err(syn::Error::new_spanned(
            expr,
            r#"expected a 2-element tuple, e.g. `("KEY", "VALUE")`"#,
        )),
    }
}

fn parse_env_list(expr: &Expr) -> syn::Result<Vec<(String, String)>> {
    match expr {
        Expr::Array(arr) => arr.elems.iter().map(parse_string_pair).collect(),
        Expr::Tuple(_) => Ok(vec![parse_string_pair(expr)?]),
        _ => Err(syn::Error::new_spanned(
            expr,
            r#"expected a list of pairs, e.g. `[("KEY", "VALUE")]`"#,
        )),
    }
}

// ---------------------------------------------------------------------------
// Codegen
// ---------------------------------------------------------------------------

/// Build the `DaemonDef` builder chain for one parsed block.
fn daemon_def_tokens(def: &DaemonDefAttr, napis: &TokenStream2) -> TokenStream2 {
    let name = def.name.as_deref().unwrap_or_default();
    let run = &def.run;
    let mut calls = Vec::new();

    if !def.depends.is_empty() {
        let depends = &def.depends;
        calls.push(quote! { .depends(&[#(#depends),*]) });
    }
    if let Some(readiness) = &def.readiness {
        let r = readiness_tokens(readiness, napis);
        calls.push(quote! { .readiness(#r) });
    }
    for (k, v) in &def.env {
        calls.push(quote! { .env(#k, #v) });
    }
    for key in &def.must_env {
        calls.push(quote! { .must_env(#key) });
    }
    if let Some(cwd) = &def.cwd {
        calls.push(quote! { .cwd(::std::path::PathBuf::from(#cwd)) });
    }
    if !def.watch.is_empty() {
        let watch = &def.watch;
        calls.push(quote! { .watch(&[#(#watch),*]) });
    }
    if let Some(mem) = &def.memory_limit {
        calls.push(quote! { .memory_limit(#mem) });
    }
    if let Some(cpu) = def.cpu_limit {
        calls.push(quote! { .cpu_limit(#cpu) });
    }
    if let Some(restart) = def.restart {
        calls.push(quote! { .restart(#restart) });
    }
    if let Some(max) = def.max_restarts {
        calls.push(quote! { .max_restarts(#max) });
    }
    if let Some(timeout) = def.stop_timeout {
        calls.push(quote! { .stop_timeout(#timeout) });
    }
    if def.boot_start {
        calls.push(quote! { .boot_start() });
    }

    quote! {
        #napis::daemon::DaemonDef::new(
            #name,
            ::std::vec![#(::std::string::String::from(#run)),*],
        )
        #(#calls)*
    }
}

fn readiness_tokens(readiness: &Readiness, napis: &TokenStream2) -> TokenStream2 {
    let rc = quote! { #napis::daemon::ReadinessConfig };
    match readiness {
        Readiness::Port(p) => quote! { #rc::Port(#p) },
        Readiness::Http(url) => quote! { #rc::Http(::std::string::String::from(#url)) },
        Readiness::Delay(secs) => quote! { #rc::delay_secs(#secs) },
        Readiness::Output(pattern) => quote! { #rc::output(#pattern) },
        Readiness::Cmd(args) => quote! {
            #rc::Cmd(::std::vec![#(::std::string::String::from(#args)),*])
        },
    }
}

/// `daemons! { ... }` — produce a `Vec<DaemonDef>`.
pub fn daemons(input: TokenStream) -> TokenStream {
    let attr: Attr = match syn::parse(input) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let napis = foundation_nativeapis_path();
    let defs: Vec<TokenStream2> = attr
        .defs
        .iter()
        .map(|d| daemon_def_tokens(d, &napis))
        .collect();
    quote! { ::std::vec![#(#defs),*] }.into()
}

/// `#[daemon_process(...)]` — boot the daemons around a fn body.
pub fn daemon_process(attr: TokenStream, item: TokenStream) -> TokenStream {
    daemon_process_impl(attr.into(), item.into()).into()
}

fn daemon_process_impl(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let args: Attr = match syn::parse2(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let input: ItemFn = match syn::parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let fn_name = &input.sig.ident;
    let fn_output = &input.sig.output;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let body = &input.block;
    let napis = foundation_nativeapis_path();

    // Stacking is rejected: one invocation owns the whole set.
    if attrs
        .iter()
        .any(|a| a.path().segments.last().is_some_and(|s| s.ident == "daemon_process"))
    {
        return syn::Error::new_spanned(
            &input.sig,
            "#[daemon_process] cannot be stacked: declare every daemon in one invocation, \
             one `{ ... }` block each",
        )
        .to_compile_error();
    }

    // Must take exactly one parameter to receive the group.
    if input.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input.sig,
            format!(
                "#[daemon_process] requires exactly one parameter to receive the booted \
                 daemons, but `{fn_name}` declares {}. Add `group: &DaemonGroup` \
                 (or `_: &DaemonGroup`).",
                input.sig.inputs.len()
            ),
        )
        .to_compile_error();
    }

    let inputs = &input.sig.inputs;
    let defs: Vec<TokenStream2> = args
        .defs
        .iter()
        .map(|d| daemon_def_tokens(d, &napis))
        .collect();

    let inner_fn = format_ident!("__daemon_body_{fn_name}");
    quote! {
        #(#attrs)*
        #vis fn #fn_name() #fn_output {
            fn #inner_fn(#inputs) #fn_output #body

            let __daemon_defs = ::std::vec![#(#defs),*];
            let __daemon_group = #napis::daemon::DaemonGroup::boot(__daemon_defs)
                .expect("failed to boot daemon group");

            #inner_fn(&__daemon_group)
        }
    }
}

/// `#[daemon_main]` — pool init + config load + boot + user body.
pub fn daemon_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    daemon_main_impl(attr.into(), item.into()).into()
}

fn daemon_main_impl(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let input: ItemFn = match syn::parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let config_path = match parse_config_arg(attr) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error(),
    };

    let fn_name = &input.sig.ident;
    let fn_output = &input.sig.output;
    let vis = &input.vis;
    let attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("test"))
        .collect();
    let body = &input.block;
    let inputs = &input.sig.inputs;
    let napis = foundation_nativeapis_path();
    let core = foundation_core_path();

    if input.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input.sig,
            "#[daemon_main] requires exactly one parameter: `group: &DaemonGroup`",
        )
        .to_compile_error();
    }

    let config_load = if let Some(path) = config_path {
        quote! {
            #napis::daemon::DaemonConfig::load_from_path(::std::path::Path::new(#path))
                .expect("failed to load daemon config")
        }
    } else {
        quote! {
            #napis::daemon::DaemonConfig::load_from_path(
                &::std::env::current_dir().expect("cwd unavailable"),
            )
            .expect("failed to load daemon config")
        }
    };

    let inner_fn = format_ident!("__daemon_main_body_{fn_name}");
    quote! {
        #(#attrs)*
        #vis fn #fn_name() #fn_output {
            fn #inner_fn(#inputs) #fn_output #body

            let __daemon_pool_guard = #core::valtron::initialize_pool(
                ::std::hash::BuildHasher::hash_one(
                    &::std::collections::hash_map::RandomState::new(),
                    0x0044_4d4e_u64,
                ),
                ::core::option::Option::None,
            );

            let __daemon_config = #config_load;
            let __daemon_defs = __daemon_config
                .into_daemon_defs()
                .expect("invalid daemon config");
            let __daemon_group = #napis::daemon::DaemonGroup::boot(__daemon_defs)
                .expect("failed to boot daemon group");

            let __daemon_result = #inner_fn(&__daemon_group);

            ::core::mem::drop(__daemon_group);
            ::core::mem::drop(__daemon_pool_guard);
            __daemon_result
        }
    }
}

/// Parse the optional `config = "path"` attribute argument.
fn parse_config_arg(attr: TokenStream2) -> syn::Result<Option<String>> {
    if attr.is_empty() {
        return Ok(None);
    }
    let parser = |input: ParseStream| -> syn::Result<Option<String>> {
        let key: Ident = input.parse()?;
        if key != "config" {
            return Err(syn::Error::new(key.span(), "expected `config = \"path\"`"));
        }
        input.parse::<Token![=]>()?;
        let value: LitStr = input.parse()?;
        Ok(Some(value.value()))
    };
    syn::parse::Parser::parse2(parser, attr)
}
