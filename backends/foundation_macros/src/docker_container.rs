//! `#[docker_container]` — start Docker containers for the duration of a function.
//!
//! Parses one or more container definitions into `ContainerConfig`s, generates
//! async setup (start each container, verify readiness) before the user's
//! function body, hands the body a `ContainerGroup` of the started containers,
//! and tears them all down after (the group's Drop, which also covers panics).
//!
//! One invocation declares every container, so this macro knows the whole set at
//! expansion time: it starts them in the order written, catches duplicate lookup
//! keys as compile errors, and needs no cooperation between attributes.

use std::collections::HashMap;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Expr, ExprLit, Ident, ItemFn, Lit, Token,
};

use crate::crate_paths::{
    foundation_core_path, foundation_deployment_platform_path,
};

/// Every container the macro invocation declares, in the order written.
struct Attr {
    defs: Vec<ContainerDef>,
}

impl Parse for Attr {
    /// Two accepted shapes:
    ///
    /// - a braced block per container, comma-separated —
    ///   `{ image = "redis:7", as = "cache" }, { image = "postgres:16", as = "db" }`
    /// - the bare `key = value` list, shorthand for a single container.
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut defs = Vec::new();

        if input.peek(syn::token::Brace) {
            while !input.is_empty() {
                let content;
                syn::braced!(content in input);
                defs.push(content.parse::<ContainerDef>()?);

                if input.is_empty() {
                    break;
                }
                input.parse::<Token![,]>()?;
            }
        } else {
            defs.push(input.parse::<ContainerDef>()?);
        }

        if defs.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[docker_container] needs at least one container definition",
            ));
        }

        // The whole set is visible here, so an ambiguous lookup key is a compile
        // error rather than a panic once the containers are already running.
        let mut seen: HashMap<&str, ()> = HashMap::new();
        for def in &defs {
            if let Some(ref key) = def.lookup_key {
                if seen.insert(key.as_str(), ()).is_some() {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        format!(
                            "duplicate container key {key:?}: each `as = \"...\"` must be unique \
                             within one #[docker_container]"
                        ),
                    ));
                }
            }
        }

        Ok(Attr { defs })
    }
}

/// A single container's configuration.
struct ContainerDef {
    image: String,
    /// Logical lookup key (`as = "cache"`) — never sent to the Docker API.
    lookup_key: Option<String>,
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

impl Parse for ContainerDef {
    /// Parses `key = value` pairs by hand rather than via
    /// `Punctuated<syn::Meta, Token![,]>`: `as` is a Rust keyword, so `Meta`'s
    /// path parser rejects `as = "cache"` with "expected identifier, found
    /// keyword `as`". Peeking for `Token![as]` accepts it, and spanning errors
    /// on the key ident gives better messages than `Meta` did.
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut image = None;
        let mut lookup_key = None;
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

        while !input.is_empty() {
            // `as` is a keyword, so it cannot be parsed as an `Ident`.
            let (key, key_span) = if input.peek(Token![as]) {
                let kw: Token![as] = input.parse()?;
                ("as".to_string(), kw.span)
            } else {
                let ident: Ident = input.parse()?;
                (ident.to_string(), ident.span())
            };

            input.parse::<Token![=]>()?;
            let value: Expr = input.parse()?;

            match key.as_str() {
                "image" => image = Some(parse_string(&value)?),
                "as" => lookup_key = Some(parse_string(&value)?),
                "port" => port.push(parse_u16(&value)?),
                "port_mapped" => port_mapped.push(parse_u16_pair(&value)?),
                "port_udp" => port_udp.push(parse_u16(&value)?),
                "env" => env.extend(parse_env_list(&value)?),
                "network" => network = Some(parse_string(&value)?),
                "name" => name = Some(parse_string(&value)?),
                "volume" => volume.push(parse_string_pair(&value)?),
                "wait_stdout" => wait_stdout = Some(parse_string(&value)?),
                "wait_port" => wait_port = Some(parse_u16(&value)?),
                "wait_http" => wait_http = Some(parse_string(&value)?),
                "wait_timeout" => wait_timeout = Some(parse_u64(&value)?),
                "stop_timeout" => stop_timeout = Some(parse_u64(&value)?),
                "memory" => memory = Some(parse_string(&value)?),
                "cpus" => cpus = Some(parse_u32(&value)?),
                "always_pull" => always_pull = parse_bool(&value)?,
                "required" => required = parse_bool(&value)?,
                _ => {
                    return Err(syn::Error::new(
                        key_span,
                        format!("unknown docker_container attribute: {key}"),
                    ));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let image = image.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "`image` is required")
        })?;

        Ok(ContainerDef {
            image,
            lookup_key,
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

    // The function must take the group, so a body can reach its containers —
    // a container it cannot address is of no use to it. Rejecting a zero-arg
    // fn at compile time is better than handing back handles nobody asked for.
    if input.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input.sig,
            format!(
                "#[docker_container] requires exactly one parameter to receive the started \
                 containers, but `{fn_name}` declares {}. Add `containers: ContainerGroup` \
                 (or `_: ContainerGroup` if the body does not need the handles).",
                input.sig.inputs.len()
            ),
        )
        .to_compile_error();
    }

    let inputs = &input.sig.inputs;
    // The parameter's pattern (`containers`), needed by the async arm to bind
    // the group to the name the body expects.
    let group_pat = match &input.sig.inputs[0] {
        syn::FnArg::Typed(pat_type) => pat_type.pat.clone(),
        syn::FnArg::Receiver(recv) => {
            return syn::Error::new_spanned(
                recv,
                "#[docker_container] cannot be applied to a method taking `self`",
            )
            .to_compile_error();
        }
    };

    // One invocation owns the whole set, so stacking is rejected rather than
    // silently half-working: a second attribute would receive an already-expanded
    // zero-arg fn and fail with an unrelated "requires exactly one parameter".
    if attrs.iter().any(|a| {
        a.path()
            .segments
            .last()
            .is_some_and(|s| s.ident == "docker_container")
    }) {
        return syn::Error::new_spanned(
            &input.sig,
            "#[docker_container] cannot be stacked: declare every container in one invocation, \
             one `{ ... }` block each — \
             #[docker_container({ image = \"redis:7\", as = \"cache\" }, { image = \"postgres:16\", as = \"db\" })]",
        )
        .to_compile_error();
    }

    // Per-container startup, in the order written. Each block starts one
    // container and adds it to the group under its lookup key; the group is the
    // single owner, so an early return drops it and cleans up whatever already
    // started.
    let starts: Vec<TokenStream2> = args
        .defs
        .iter()
        .map(|def| container_start(def, &platform))
        .collect();

    // Starting the containers is the same async job in both arms: fill a group
    // in declaration order, yielding `None` if Docker is absent. `Option` (not
    // `return`) carries the skip out, since a `return` inside an async block
    // only leaves the block. On `None` the partially filled group drops here,
    // stopping whatever had already started.
    let startup = quote! {
        async move {
            let mut __docker_group = #platform::docker::ContainerGroup::empty();
            #(#starts)*
            ::core::option::Option::Some(__docker_group)
        }
    };

    // The arms differ only in what drives that job: the caller's executor for an
    // `async fn`, or valtron's pool for a sync one (the Docker client is
    // valtron-based, hence the documented pairing with `#[valtron_test]`).
    let expanded = if is_async {
        quote! {
            #(#attrs)*
            #vis async fn #fn_name() #fn_output {
                let __docker_group = match #startup.await {
                    ::core::option::Option::Some(g) => g,
                    ::core::option::Option::None => return,
                };

                // Bind the group to the name the body declared.
                let #group_pat = __docker_group;

                // The body is spliced in directly rather than wrapped in an
                // `async {}`: this fn is already `async`, so `.await` inside the
                // body compiles as-is, and `return` keeps meaning "return from
                // this fn" (an async block would swallow it). The group's Drop
                // still stops the containers on an early return.
                #body
            }
        }
    } else {
        let inner_fn = format_ident!("__docker_body_{fn_name}");
        quote! {
            #(#attrs)*
            #vis fn #fn_name() #fn_output {
                fn #inner_fn(#inputs) #fn_output #body

                let __docker_group = match #core::valtron::block_on_future(#startup) {
                    ::core::option::Option::Some(g) => g,
                    ::core::option::Option::None => return,
                };

                #inner_fn(__docker_group)
            }
        }
    };

    expanded
}

/// Generates the statements that start one container and add it to
/// `__docker_group`. Written to be valid in both arms: the enclosing scope is
/// always an async context, and a skip is a bare `return` — which returns from
/// the `async fn` in the async arm, and from the `block_on_future` async block
/// (yielding `None`) in the sync arm.
fn container_start(def: &ContainerDef, platform: &TokenStream2) -> TokenStream2 {
    let config = container_config(def, platform);

    let lookup_key = if let Some(k) = &def.lookup_key { quote! { ::core::option::Option::Some(::std::string::String::from(#k)) } } else { quote! { ::core::option::Option::None } };

    // `required = true` means a missing daemon is a hard failure, so the
    // graceful-skip arm is left out of the generated match entirely and any
    // start error falls through to the panic arm.
    let skip_arm = if def.required {
        quote! {}
    } else {
        quote! {
            Err(e) if e.current_context().is_connection_error() => {
                ::tracing::warn!("SKIP: Docker not available ({})", e);
                return ::core::option::Option::None;
            }
        }
    };

    quote! {
        {
            let __cfg = #config;
            match #platform::docker::ContainerHandle::start_async(__cfg).await {
                Ok(h) => __docker_group.insert(#lookup_key, h),
                #skip_arm
                Err(e) => ::std::panic!("Failed to start Docker container: {}", e),
            }
        }
    }
}

/// Builds one container's `ContainerConfig` builder chain.
fn container_config(args: &ContainerDef, platform: &TokenStream2) -> TokenStream2 {
    let image = &args.image;
    let mut config_calls = Vec::new();

    let port = &args.port;
    for p in port {
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

    quote! {
        #platform::docker::ContainerConfig::new(#image)
            #(#config_calls)*
    }
}
