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
    port_udp: Vec<u16>,
    env: Vec<(String, String)>,
    network: Option<String>,
    name: Option<String>,
    volume: Vec<(String, String)>,
    wait_stdout: Option<String>,
    wait_port: Option<u16>,
    wait_http: Option<String>,
    wait_timeout: Option<u64>,
    stop_timeout: Option<u64>,
    memory: Option<String>,
    cpus: Option<u32>,
    always_pull: bool,
    required: bool,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut image = None;
        let mut port = Vec::new();
        let mut port_mapped = Vec::new();
        let mut port_udp = Vec::new();
        let mut env = Vec::new();
        let mut network = None;
        let mut name = None;
        let mut volume = Vec::new();
        let mut wait_stdout = None;
        let mut wait_port = None;
        let mut wait_http = None;
        let mut wait_timeout = None;
        let mut stop_timeout = None;
        let mut memory = None;
        let mut cpus = None;
        let mut always_pull = false;
        let mut required = false;

        let metas: Punctuated<Meta, Token![,]> = input.parse_terminated(Meta::parse, Token![,])?;
        for meta in metas {
            if let Meta::NameValue(nv) = meta {
                let key = nv.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                match key.as_str() {
                    "image" => image = Some(parse_string(&nv.value)?),
                    "port" => port.push(parse_u16(&nv.value)?),
                    "port_mapped" => port_mapped.push(parse_u16_pair(&nv.value)?),
                    "port_udp" => port_udp.push(parse_u16(&nv.value)?),
                    "env" => env.extend(parse_env_list(&nv.value)?),
                    "network" => network = Some(parse_string(&nv.value)?),
                    "name" => name = Some(parse_string(&nv.value)?),
                    "volume" => volume.push(parse_string_pair(&nv.value)?),
                    "wait_stdout" => wait_stdout = Some(parse_string(&nv.value)?),
                    "wait_port" => wait_port = Some(parse_u16(&nv.value)?),
                    "wait_http" => wait_http = Some(parse_string(&nv.value)?),
                    "wait_timeout" => wait_timeout = Some(parse_u64(&nv.value)?),
                    "stop_timeout" => stop_timeout = Some(parse_u64(&nv.value)?),
                    "memory" => memory = Some(parse_string(&nv.value)?),
                    "cpus" => cpus = Some(parse_u32(&nv.value)?),
                    "always_pull" => always_pull = parse_bool(&nv.value)?,
                    "required" => required = parse_bool(&nv.value)?,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &nv,
                            format!("unknown docker_container attribute: {key}"),
                        ));
                    }
                }
            } else {
                return Err(syn::Error::new_spanned(
                    &meta,
                    "expected `key = value` in docker_container attribute",
                ));
            }
        }

        let image = image.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "`image` is required")
        })?;

        Ok(Attr {
            image,
            port,
            port_mapped,
            port_udp,
            env,
            network,
            name,
            volume,
            wait_stdout,
            wait_port,
            wait_http,
            wait_timeout,
            stop_timeout,
            memory,
            cpus,
            always_pull,
            required,
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

fn parse_bool(expr: &Expr) -> syn::Result<bool> {
    if let Expr::Lit(ExprLit { lit: Lit::Bool(b), .. }) = expr {
        Ok(b.value())
    } else {
        Err(syn::Error::new_spanned(expr, "expected `true` or `false`"))
    }
}

/// Splits a 2-element tuple expression, e.g. `(6379, 3030)` or `("/h", "/c")`.
fn tuple_parts(expr: &Expr) -> syn::Result<(&Expr, &Expr)> {
    match expr {
        Expr::Tuple(t) if t.elems.len() == 2 => Ok((&t.elems[0], &t.elems[1])),
        _ => Err(syn::Error::new_spanned(expr, "expected a 2-element tuple, e.g. `(6379, 3030)`")),
    }
}

/// `port_mapped = (container_port, host_port)`
fn parse_u16_pair(expr: &Expr) -> syn::Result<(u16, u16)> {
    let (a, b) = tuple_parts(expr)?;
    Ok((parse_u16(a)?, parse_u16(b)?))
}

/// `volume = ("/host/path", "/container/path")`
fn parse_string_pair(expr: &Expr) -> syn::Result<(String, String)> {
    let (a, b) = tuple_parts(expr)?;
    Ok((parse_string(a)?, parse_string(b)?))
}

/// `env = [("KEY", "VALUE"), ...]`, also accepting a bare `("KEY", "VALUE")`.
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
    for p in &args.port_udp {
        config_calls.push(quote! { .port_udp(#p) });
    }
    for (k, v) in &args.env {
        config_calls.push(quote! { .env(#k, #v) });
    }
    for (h, c) in &args.volume {
        config_calls.push(quote! { .volume(#h, #c) });
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
    if args.always_pull {
        config_calls.push(quote! { .always_pull() });
    }
    if args.required {
        config_calls.push(quote! { .required() });
    }

    // Wait strategies. Every explicit `wait_*` attribute contributes one
    // strategy; several of them compose into `WaitFor::all` with AND semantics,
    // applied in the order written. `wait_timeout` (seconds) overrides the
    // 30s per-strategy default, so the variants are built as struct literals
    // rather than via the fixed-timeout constructors.
    let wait_timeout = args.wait_timeout.unwrap_or(30);
    let timeout_expr = quote! { ::core::time::Duration::from_secs(#wait_timeout) };
    let mut waits = Vec::new();
    if let Some(p) = args.wait_port {
        waits.push(quote! {
            #platform::docker::WaitFor::Port { port: #p, timeout: #timeout_expr }
        });
    }
    if let Some(ref url) = args.wait_http {
        waits.push(quote! {
            #platform::docker::WaitFor::Http {
                url: ::std::string::String::from(#url),
                expected_status: 200,
                timeout: #timeout_expr,
            }
        });
    }
    if let Some(ref msg) = args.wait_stdout {
        waits.push(quote! {
            #platform::docker::WaitFor::Stdout {
                message: ::std::string::String::from(#msg),
                timeout: #timeout_expr,
            }
        });
    }

    // Default when no explicit wait was given: wait on the first exposed port
    // (a mapped port counts). With no ports at all there is nothing to probe.
    if waits.is_empty() {
        let default_port = port.first().copied().or_else(|| args.port_mapped.first().map(|(c, _)| *c));
        if let Some(p) = default_port {
            waits.push(quote! {
                #platform::docker::WaitFor::Port { port: #p, timeout: #timeout_expr }
            });
        }
    }

    match waits.len() {
        0 => {}
        1 => {
            let w = &waits[0];
            config_calls.push(quote! { .wait(#w) });
        }
        _ => {
            config_calls.push(quote! {
                .wait(#platform::docker::WaitFor::all(::std::vec![#(#waits),*]))
            });
        }
    }

    // `required = true` means a missing daemon is a hard failure, so the
    // graceful-skip arm is left out of the generated match entirely and any
    // start error falls through to the panic arm.
    let async_skip_arm = if args.required {
        quote! {}
    } else {
        quote! {
            Err(e) if e.current_context().is_connection_error() => {
                ::tracing::warn!("SKIP: Docker not available ({})", e);
                return;
            }
        }
    };
    let sync_skip_arm = if args.required {
        quote! {}
    } else {
        quote! {
            Err(e) if e.current_context().is_connection_error() => {
                ::tracing::warn!("SKIP: Docker not available ({})", e);
                None
            }
        }
    };

    // Generate the wrapper
    let expanded = if is_async {
        quote! {
            #(#attrs)*
            #vis async fn #fn_name() #fn_output {
                let __cfg = #platform::docker::ContainerConfig::new(#image)
                    #(#config_calls)*;

                let __docker_guard = match #platform::docker::ContainerHandle::start_async(__cfg).await {
                    Ok(h) => h,
                    #async_skip_arm
                    Err(e) => ::std::panic!("Failed to start Docker container: {}", e),
                };

                // The body is spliced in directly rather than wrapped in an
                // `async {}`: this fn is already `async`, so `.await` inside the
                // body compiles as-is, and `return` keeps meaning "return from
                // this fn" (an async block would swallow it). The guard's Drop
                // still stops the container on an early return.
                let __result = { #body };
                ::core::mem::drop(__docker_guard);
                __result
            }
        }
    } else {
        let inner_fn = format_ident!("__docker_body_{fn_name}");
        quote! {
            #(#attrs)*
            #vis fn #fn_name() #fn_output {
                fn #inner_fn() #fn_output #body

                let __cfg = #platform::docker::ContainerConfig::new(#image)
                    #(#config_calls)*;

                // `Option` so the Docker-absent skip needs no `Default` on the
                // handle; `None` means skip the body (as the async arm returns).
                let __docker_guard = #core::valtron::block_on_future(async move {
                    match #platform::docker::ContainerHandle::start_async(__cfg).await {
                        Ok(h) => Some(h),
                        #sync_skip_arm
                        Err(e) => ::std::panic!("Failed to start Docker container: {}", e),
                    }
                });

                let __docker_guard = match __docker_guard {
                    Some(h) => h,
                    None => return,
                };

                let __result = #inner_fn();
                ::core::mem::drop(__docker_guard);
                __result
            }
        }
    };

    expanded
}
