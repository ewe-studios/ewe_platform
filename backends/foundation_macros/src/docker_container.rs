//! `#[docker_container]` — start Docker containers for the duration of a function.
//!
//! Parses key=value attributes into a `ContainerConfig`, generates async setup
//! (start container, verify readiness) and teardown (stop + remove on Drop)
//! around the user's function body.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, ExprLit, ItemFn, Lit, Meta, Token,
};

use crate::crate_paths::{
    foundation_core_path, foundation_deployment_platform_path,
};

struct Attr {
    image: String,
    port: Vec<u16>,
    port_mapped: Vec<(u16, u16)>,
    env: Vec<(String, String)>,
    network: Option<String>,
    name: Option<String>,
    wait_stdout: Option<String>,
    wait_port: Option<u16>,
    stop_timeout: Option<u64>,
    memory: Option<String>,
    cpus: Option<u32>,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut image = None;
        let mut port = Vec::new();
        let port_mapped = Vec::new();
        let env = Vec::new();
        let mut network = None;
        let mut name = None;
        let mut wait_stdout = None;
        let mut wait_port = None;
        let mut stop_timeout = None;
        let mut memory = None;
        let mut cpus = None;

        let metas: Punctuated<Meta, Token![,]> = input.parse_terminated(Meta::parse, Token![,])?;
        for meta in metas {
            if let Meta::NameValue(nv) = meta {
                let key = nv.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                match key.as_str() {
                    "image" => image = Some(parse_string(&nv.value)?),
                    "port" => port.push(parse_u16(&nv.value)?),
                    "network" => network = Some(parse_string(&nv.value)?),
                    "name" => name = Some(parse_string(&nv.value)?),
                    "wait_stdout" => wait_stdout = Some(parse_string(&nv.value)?),
                    "wait_port" => wait_port = Some(parse_u16(&nv.value)?),
                    "stop_timeout" => stop_timeout = Some(parse_u64(&nv.value)?),
                    "memory" => memory = Some(parse_string(&nv.value)?),
                    "cpus" => cpus = Some(parse_u32(&nv.value)?),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &nv,
                            format!("unknown docker_container attribute: {key}"),
                        ));
                    }
                }
            }
        }

        let image = image.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "`image` is required")
        })?;

        Ok(Attr {
            image,
            port,
            port_mapped,
            env,
            network,
            name,
            wait_stdout,
            wait_port,
            stop_timeout,
            memory,
            cpus,
        })
    }
}

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

fn parse_u64(expr: &Expr) -> syn::Result<u64> {
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

pub fn docker_container(attr: TokenStream, item: TokenStream) -> TokenStream {
    docker_container_impl(attr.into(), item.into()).into()
}

fn docker_container_impl(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
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
    let is_async = input.sig.asyncness.is_some();
    let body = &input.block;
    let vis = &input.vis;
    let attrs = &input.attrs;

    let platform = foundation_deployment_platform_path();
    let core = foundation_core_path();

    // Build the ContainerConfig chain from parsed attributes
    let image = &args.image;
    let mut config_calls = Vec::new();

    let port = args.port;
    for p in &port {
        config_calls.push(quote! { .port(#p) });
    }
    for (c, h) in &args.port_mapped {
        config_calls.push(quote! { .port_mapped(#c, #h) });
    }
    for (k, v) in &args.env {
        config_calls.push(quote! { .env(#k, #v) });
    }
    if let Some(ref n) = args.network {
        config_calls.push(quote! { .network(#n) });
    }
    if let Some(ref n) = args.name {
        config_calls.push(quote! { .name(#n) });
    }
    if let Some(ref timeout) = args.stop_timeout {
        config_calls.push(quote! { .stop_timeout_secs(#timeout) });
    }
    if let Some(ref mem) = args.memory {
        config_calls.push(quote! { .memory(#mem) });
    }
    if let Some(ref c) = args.cpus {
        config_calls.push(quote! { .cpus(#c) });
    }

    // Wait strategy
    if let Some(ref msg) = args.wait_stdout {
        config_calls.push(quote! { .wait(#platform::docker::WaitFor::stdout(#msg)) });
    } else if let Some(p) = args.wait_port {
        config_calls.push(quote! { .wait(#platform::docker::WaitFor::port(#p)) });
    } else if let Some(first_port) = port.first() {
        config_calls.push(quote! { .wait(#platform::docker::WaitFor::port(#first_port)) });
    }

    // Generate the wrapper
    let expanded = if is_async {
        quote! {
            #(#attrs)*
            #vis #fn_name() #fn_output {
                let __cfg = #platform::docker::ContainerConfig::new(#image)
                    #(#config_calls)*;

                let __docker_guard = match #platform::docker::ContainerHandle::start_async(__cfg).await {
                    Ok(h) => h,
                    Err(e) if e.current_context().is_connection_error() => {
                        ::tracing::warn!("SKIP: Docker not available ({})", e);
                        return;
                    }
                    Err(e) => ::std::panic!("Failed to start Docker container: {}", e),
                };

                let __result = { #body }.await;
                ::core::mem::drop(__docker_guard);
                __result
            }
        }
    } else {
        let inner_fn = format_ident!("__docker_body_{fn_name}");
        quote! {
            #(#attrs)*
            #vis #fn_name() #fn_output {
                fn #inner_fn() #fn_output #body

                let __cfg = #platform::docker::ContainerConfig::new(#image)
                    #(#config_calls)*;

                let __docker_guard = #core::valtron::block_on_future(async move {
                    match #platform::docker::ContainerHandle::start_async(__cfg).await {
                        Ok(h) => h,
                        Err(e) if e.current_context().is_connection_error() => {
                            ::tracing::warn!("SKIP: Docker not available ({})", e);
                            return Default::default();
                        }
                        Err(e) => ::std::panic!("Failed to start Docker container: {}", e),
                    }
                });

                let __result = #inner_fn();
                ::core::mem::drop(__docker_guard);
                __result
            }
        }
    };

    expanded
}
