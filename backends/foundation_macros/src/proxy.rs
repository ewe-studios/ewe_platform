//! `proxy!` — compile-time proxy configuration macro (Decision 19).
//!
//! Function-like proc macro that desugars a custom block syntax into a
//! `ProxyConfig` builder chain.  Unknown keys and missing required fields
//! are compile errors.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    Ident, LitInt, LitStr, Token,
};

// ── Top-level: proxy! { domain: "...", public_ip: "...", ssl: ..., services: { ... } } ──

struct ProxyInput {
    domain: LitStr,
    public_ip: LitStr,
    bind_addr: Option<LitStr>,
    ssl: Option<SslBlock>,
    services: Vec<ServiceBlock>,
}

impl Parse for ProxyInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut domain = None;
        let mut public_ip = None;
        let mut bind_addr = None;
        let mut ssl = None;
        let mut services = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "domain" => domain = Some(input.parse()?),
                "public_ip" => public_ip = Some(input.parse()?),
                "bind_addr" => bind_addr = Some(input.parse()?),
                "ssl" => ssl = Some(input.parse()?),
                "services" => {
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        services.push(content.parse()?);
                        // Optional trailing comma
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new(key.span(), format!("unknown key: {other}")));
                }
            }
            // Optional trailing comma between top-level fields
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            domain: domain.ok_or_else(|| input.error("missing required field: domain"))?,
            public_ip: public_ip.ok_or_else(|| input.error("missing required field: public_ip"))?,
            bind_addr,
            ssl,
            services,
        })
    }
}

// ── SSL block: ssl: lets_encrypt { email: "..." } or cloudflare { zone_id: "..." } or static_cert { cert: "...", key: "..." } or none ──

enum SslBlock {
    LetsEncrypt { email: LitStr },
    Cloudflare { zone_id: LitStr },
    StaticCert { cert: LitStr, key: LitStr },
    None,
}

impl Parse for SslBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let variant: Ident = input.parse()?;
        match variant.to_string().as_str() {
            "lets_encrypt" => {
                let content;
                braced!(content in input);
                let email = parse_field(&content, "email")?;
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
                Ok(SslBlock::LetsEncrypt { email })
            }
            "cloudflare" => {
                let content;
                braced!(content in input);
                let zone_id = parse_field(&content, "zone_id")?;
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
                Ok(SslBlock::Cloudflare { zone_id })
            }
            "static_cert" => {
                let content;
                braced!(content in input);
                let cert = parse_field(&content, "cert")?;
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
                let key = parse_field(&content, "key")?;
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
                Ok(SslBlock::StaticCert { cert, key })
            }
            "none" => Ok(SslBlock::None),
            other => Err(syn::Error::new(variant.span(), format!("unknown SSL provider: {other}"))),
        }
    }
}

// ── Service block: name { host: "...", backends: [...], path_prefix: "...", health_check: { ... } } ──

struct ServiceBlock {
    name: Ident,
    host: LitStr,
    path_prefix: Option<LitStr>,
    backends: Vec<LitStr>,
    health_check: Option<HealthCheckBlock>,
}

impl Parse for ServiceBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let content;
        braced!(content in input);

        let mut host = None;
        let mut path_prefix = None;
        let mut backends = Vec::new();
        let mut health_check = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "host" => host = Some(content.parse()?),
                "path_prefix" => path_prefix = Some(content.parse()?),
                "backends" => {
                    let inner;
                    syn::bracketed!(inner in content);
                    while !inner.is_empty() {
                        backends.push(inner.parse()?);
                        if inner.peek(Token![,]) {
                            inner.parse::<Token![,]>()?;
                        }
                    }
                }
                "health_check" => health_check = Some(content.parse()?),
                other => return Err(syn::Error::new(key.span(), format!("unknown service key: {other}"))),
            }
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            name,
            host: host.ok_or_else(|| content.error("missing required field: host"))?,
            path_prefix,
            backends,
            health_check,
        })
    }
}

// ── Health check block: health_check: { path: "/up", interval: 5, timeout: 2, healthy: 2, unhealthy: 3 } ──

struct HealthCheckBlock {
    path: LitStr,
    interval: LitInt,
    timeout: LitInt,
    healthy_threshold: Option<LitInt>,
    unhealthy_threshold: Option<LitInt>,
}

impl Parse for HealthCheckBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let mut path = None;
        let mut interval = None;
        let mut timeout = None;
        let mut healthy_threshold = None;
        let mut unhealthy_threshold = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "path" => path = Some(content.parse()?),
                "interval" => interval = Some(content.parse()?),
                "timeout" => timeout = Some(content.parse()?),
                "healthy" => healthy_threshold = Some(content.parse()?),
                "unhealthy" => unhealthy_threshold = Some(content.parse()?),
                other => return Err(syn::Error::new(key.span(), format!("unknown health_check key: {other}"))),
            }
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            path: path.ok_or_else(|| content.error("missing health_check.path"))?,
            interval: interval.ok_or_else(|| content.error("missing health_check.interval"))?,
            timeout: timeout.ok_or_else(|| content.error("missing health_check.timeout"))?,
            healthy_threshold,
            unhealthy_threshold,
        })
    }
}

// ── Helper: parse `key: value` inside a braced block ──

fn parse_field<T: Parse>(input: ParseStream, field_name: &str) -> syn::Result<T> {
    let key: Ident = input.parse()?;
    if key != field_name {
        return Err(syn::Error::new(key.span(), format!("expected {field_name}, got {key}")));
    }
    input.parse::<Token![:]>()?;
    input.parse()
}

// ── Codegen ──

pub fn proxy_impl(input: TokenStream) -> TokenStream {
    let config = match syn::parse::<ProxyInput>(input) {
        Ok(c) => c,
        Err(e) => return e.to_compile_error().into(),
    };

    let domain = &config.domain;
    let public_ip = &config.public_ip;

    // Bind addr
    let bind_call = match &config.bind_addr {
        Some(addr) => quote! { .bind(#addr) },
        None => quote! {},
    };

    // SSL
    let ssl_call = match &config.ssl {
        None => quote! {},
        Some(SslBlock::LetsEncrypt { email }) => {
            quote! { .ssl(foundation_proxy::SslConfig::lets_encrypt(#email)) }
        }
        Some(SslBlock::Cloudflare { zone_id }) => {
            quote! { .ssl(foundation_proxy::SslConfig::cloudflare(#zone_id)) }
        }
        Some(SslBlock::StaticCert { cert, key }) => {
            quote! { .ssl(foundation_proxy::SslConfig::static_cert(#cert, #key)) }
        }
        Some(SslBlock::None) => quote! {},
    };

    // Services
    let service_calls: Vec<_> = config.services.iter().map(|svc| {
        let name_str = svc.name.to_string();
        let name_lit = LitStr::new(&name_str, svc.name.span());
        let host = &svc.host;

        let prefix_call = match &svc.path_prefix {
            Some(p) => quote! { .path_prefix(#p) },
            None => quote! {},
        };

        let backend_calls: Vec<_> = svc.backends.iter().map(|b| {
            quote! { .backend(#b) }
        }).collect();

        let health_call = match &svc.health_check {
            Some(hc) => {
                let path = &hc.path;
                let interval = &hc.interval;
                let timeout = &hc.timeout;
                let healthy = &hc.healthy_threshold;
                let unhealthy = &hc.unhealthy_threshold;

                let healthy_set = match healthy {
                    Some(v) => quote! { healthy_threshold: #v as u32, },
                    None => quote! {},
                };
                let unhealthy_set = match unhealthy {
                    Some(v) => quote! { unhealthy_threshold: #v as u32, },
                    None => quote! {},
                };

                quote! {
                    .health_check_config(foundation_proxy::HealthCheckConfig {
                        path: #path.into(),
                        interval: std::time::Duration::from_secs(#interval as u64),
                        timeout: std::time::Duration::from_secs(#timeout as u64),
                        #healthy_set
                        #unhealthy_set
                        ..foundation_proxy::HealthCheckConfig::default()
                    })
                }
            }
            None => quote! {},
        };

        quote! {
            .service(
                foundation_proxy::ServiceConfig::new(#name_lit, #host)
                    #prefix_call
                    #(#backend_calls)*
                    #health_call
            )
        }
    }).collect();

    let expanded = quote! {
        {
            foundation_proxy::ProxyConfig::new(#domain, #public_ip)
                #bind_call
                #ssl_call
                #(#service_calls)*
                .build()
        }
    };

    expanded.into()
}
