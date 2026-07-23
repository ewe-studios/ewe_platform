//! `wireguard!` — compile-time `WireGuard` mesh configuration macro (spec-55, feature 09).
//!
//! Function-like proc macro that desugars a custom block syntax into a
//! `WgConfig` builder chain. Unknown keys and missing required fields
//! are compile errors.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    Ident, LitBool, LitInt, LitStr, Token,
};

// ── Top-level: wireguard! { seed: "...", network: "...", ... } ──

struct WireguardInput {
    seed: Option<LitStr>,
    seed_env: Option<LitStr>,
    network_id: Option<LitStr>,
    udp_listen: Option<LitStr>,
    bootstrap_listen: Option<LitStr>,
    relay: Option<RelayBlock>,
    seed_endpoints: Vec<LitStr>,
    mtu: Option<LitInt>,
    keepalive: Option<LitInt>,
}

impl Parse for WireguardInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut seed = None;
        let mut seed_env = None;
        let mut network_id = None;
        let mut udp_listen = None;
        let mut bootstrap_listen = None;
        let mut relay = None;
        let mut seed_endpoints = Vec::new();
        let mut mtu = None;
        let mut keepalive = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "seed" => seed = Some(input.parse()?),
                "seed_env" => seed_env = Some(input.parse()?),
                "network_id" => network_id = Some(input.parse()?),
                "udp_listen" => udp_listen = Some(input.parse()?),
                "bootstrap_listen" => bootstrap_listen = Some(input.parse()?),
                "relay" => relay = Some(input.parse()?),
                "seed_endpoints" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        seed_endpoints.push(content.parse()?);
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "mtu" => mtu = Some(input.parse()?),
                "keepalive" => keepalive = Some(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key: {other}"),
                    ));
                }
            }
            // Optional trailing comma between fields.
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        if seed.is_none() && seed_env.is_none() {
            return Err(input.error(
                "missing required field: `seed` or `seed_env` (no silent default)",
            ));
        }

        Ok(Self {
            seed,
            seed_env,
            network_id,
            udp_listen,
            bootstrap_listen,
            relay,
            seed_endpoints,
            mtu,
            keepalive,
        })
    }
}

// ── Relay block: relay: { advertise: true, max_sessions: 4096 } ──

struct RelayBlock {
    advertise: Option<LitBool>,
    max_sessions: Option<LitInt>,
    rate_limit_pps: Option<LitInt>,
    idle_timeout_secs: Option<LitInt>,
}

impl Parse for RelayBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let mut advertise = None;
        let mut max_sessions = None;
        let mut rate_limit_pps = None;
        let mut idle_timeout_secs = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "advertise" => advertise = Some(content.parse()?),
                "max_sessions" => max_sessions = Some(content.parse()?),
                "rate_limit_pps" => rate_limit_pps = Some(content.parse()?),
                "idle_timeout_secs" => idle_timeout_secs = Some(content.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown relay key: {other}"),
                    ));
                }
            }
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            advertise,
            max_sessions,
            rate_limit_pps,
            idle_timeout_secs,
        })
    }
}

// ── Codegen ──

pub fn wireguard_impl(input: TokenStream) -> TokenStream {
    let config = match syn::parse::<WireguardInput>(input) {
        Ok(c) => c,
        Err(e) => return e.to_compile_error().into(),
    };

    // Seed: either `seed("base64...")` or from env.
    let seed_call = if let Some(seed_lit) = &config.seed {
        quote! {
            .seed(foundation_wireguard::WgSeed::from_base64url(#seed_lit)?)
        }
    } else if let Some(env_lit) = &config.seed_env {
        quote! {
            .seed({
                let val = std::env::var(#env_lit).map_err(|_| {
                    foundation_wireguard::WgError::Config(
                        format!("env var {} not set", #env_lit))
                })?;
                foundation_wireguard::WgSeed::from_base64url(&val)?
            })
        }
    } else {
        quote! {}
    };

    // network_id
    let net_call = match &config.network_id {
        Some(net) => quote! { .network_id(foundation_wireguard::NetworkId::from_hex(#net)?) },
        None => quote! {}, // derived from seed
    };

    // udp_listen
    let udp_call = match &config.udp_listen {
        Some(addr) => quote! { .udp_listen(#addr.parse()?) },
        None => quote! {},
    };

    // bootstrap_listen
    let boot_call = match &config.bootstrap_listen {
        Some(addr) => quote! { .bootstrap_listen(#addr.parse()?) },
        None => quote! {},
    };

    // seed_endpoints
    let ep_calls: Vec<_> = config
        .seed_endpoints
        .iter()
        .map(|ep| quote! { .seed_endpoint(#ep.parse()?) })
        .collect();

    // relay
    let relay_call = match &config.relay {
        Some(relay) => {
            let advertise = if let Some(b) = &relay.advertise { quote! { .relay_advertise(#b) } } else { quote! { .relay_advertise(true) } };
            let sessions = match &relay.max_sessions {
                Some(n) => quote! { .relay_max_sessions(#n as u32) },
                None => quote! {},
            };
            let rate = match &relay.rate_limit_pps {
                Some(n) => quote! { .relay_rate_limit_pps(#n as u32) },
                None => quote! {},
            };
            let idle = match &relay.idle_timeout_secs {
                Some(n) => quote! { .relay_idle_timeout_secs(#n as u64) },
                None => quote! {},
            };
            quote! {
                #advertise
                #sessions
                #rate
                #idle
            }
        }
        None => quote! {},
    };

    // mtu / keepalive
    let mtu_call = match &config.mtu {
        Some(m) => quote! { .mtu(#m as u16) },
        None => quote! {},
    };
    let keep_call = match &config.keepalive {
        Some(k) => quote! { .keepalive_secs(#k as u16) },
        None => quote! {},
    };

    let expanded = quote! {
        {
            foundation_wireguard::WgConfig::builder()
                #seed_call
                #net_call
                #udp_call
                #boot_call
                #(#ep_calls)*
                #relay_call
                #mtu_call
                #keep_call
                .build()?
        }
    };

    expanded.into()
}

// ── #[wireguard_main] attribute macro ──

/// WHY: Like `#[valtron]` initialises the engine pool, `#[wireguard_main]` initialises
/// the pool AND joins the mesh — the single entry point for a WG service.
///
/// WHAT: Wraps a `fn(&WgHandle)` in a `fn main()` that loads the config, joins, and
/// passes the handle to the user body.
///
/// HOW: Follows the `valtron_entry` pattern: move the body into an inner fn, set up the
/// engine + mesh around it, drop them after the body returns.
///
/// Attribute args: `config = "path/to/wireguard.toml"` (optional; defaults to `from_env`).
pub fn wireguard_main_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = match syn::parse::<syn::ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };

    let func_name = &func.sig.ident;
    let func_vis = &func.vis;
    let func_sig = &func.sig;
    let func_body = &func.block;
    let func_attrs: Vec<_> = func
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("test"))
        .collect();

    // Parse the attribute for an optional `config = "path"` arg.
    let config_path = parse_config_arg(attr);

    // Build the inner fn that the generated main() calls.
    let inner = quote::format_ident!("__wireguard_body_{}", func_name);
    let inner_fn = quote! {
        #[allow(clippy::items_after_statements)]
        fn #inner(wg_handle: &foundation_wireguard::native::WgHandle) {
            #func_body
        }
    };

    let config_load = if let Some(path) = config_path {
        quote! {
            foundation_wireguard::WgConfig::load_file(#path)
                .expect("failed to load wireguard config")
        }
    } else {
        quote! {
            foundation_wireguard::WgConfig::from_env()
                .expect("failed to build wireguard config from env (set WG_SECRET)")
        }
    };

    let expanded = quote! {
        #(#func_attrs)*
        #func_vis #func_sig {
            // Initialise the valtron engine pool.
            let __wg_pool_guard = ::foundation_core::valtron::initialize_pool(
                ::std::hash::BuildHasher::hash_one(
                    &::std::collections::hash_map::RandomState::new(),
                    0x0057_4700_u64,
                ),
                ::core::option::Option::None,
            );

            // Load config + join the mesh.
            let __wg_config = #config_load;
            let __wg_node = foundation_wireguard::native::WgNode::from_config(__wg_config);
            let __wg_handle = __wg_node.join().expect("wireguard mesh join failed");

            // Run the user body.
            #inner_fn
            #inner(&__wg_handle);

            // Tear down.
            __wg_handle.shutdown();
            ::core::mem::drop(__wg_pool_guard);
        }
    };

    expanded.into()
}

/// Parse `config = "path"` from the attribute tokens, if present.
fn parse_config_arg(attr: TokenStream) -> Option<String> {
    if attr.is_empty() {
        return None;
    }
    // Try to parse as `config = "path"`.
    let attr_str = attr.to_string();
    if let Some(pos) = attr_str.find("config") {
        if let Some(eq) = attr_str[pos..].find('=') {
            let val = attr_str[pos + eq + 1..].trim();
            // Strip quotes
            let stripped = val.trim_matches('"').trim_matches('\'');
            return Some(stripped.to_string());
        }
    }
    None
}
