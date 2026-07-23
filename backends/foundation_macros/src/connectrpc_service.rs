//! Code-first `ConnectRPC` service generation (Feature 27, Decision 10 Mode 3).
//!
//! WHY: Proto is not the only source of truth. A service may be defined as a
//! plain Rust trait via `#[foundation_connectrpc::service(package = "…", codecs(…))]`, and
//! the macro generates the same artifacts the proto codegen path does: procedure
//! constants, registration fns, typed clients, and an `UnimplementedXxxHandler`.
//!
//! WHAT: Two proc-macro entry points:
//!   - `#[service]` (attribute) — transforms a trait into the full set of
//!     `ConnectRPC` service items, also emitting a `#[macro_export]` descriptor
//!     macro for cross-crate generation.
//!   - `generate!` (function-like) — syntactic sugar that expands a cross-crate
//!     descriptor macro inside a module.
//!
//! HOW: The attribute macro parses the trait, classifies each method's stream
//! shape by return type and argument pattern, then emits the service name
//! constant, procedure module, trait (with default unimplemented bodies),
//! registration fn, `UnimplementedHandler`, typed Client struct, `ClientTrait`,
//! and the descriptor macro.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse2, FnArg, Ident, ItemTrait, LitStr, PatType, PathArguments, ReturnType, Token, TraitItem,
    Type, TypeParamBound,
};

// ── Attribute argument parser ──────────────────────────────────────────────

/// Parsed arguments to `#[service(package = "…", codecs(json, arrow))]`.
struct ServiceAttr {
    /// The protobuf-style package name (e.g. `"acme.greet.v1"`).
    package: String,
    /// Requested codec families — defaults to `["json"]`.
    codecs: Vec<String>,
}

impl Parse for ServiceAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut package = String::new();
        let mut codecs = vec!["json".to_string()];

        while !input.is_empty() {
            let name: Ident = input.parse()?;
            if name == "package" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                package = lit.value();
            } else if name == "codecs" {
                let content;
                syn::parenthesized!(content in input);
                codecs.clear();
                while !content.is_empty() {
                    let c: Ident = content.parse()?;
                    codecs.push(c.to_string());
                    if !content.is_empty() {
                        content.parse::<Token![,]>()?;
                    }
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        if package.is_empty() {
            return Err(syn::Error::new(input.span(), "service: `package = \"…\"` is required"));
        }

        Ok(ServiceAttr { package, codecs })
    }
}

// ── RPC method shape ───────────────────────────────────────────────────────

/// The four `ConnectRPC` streaming shapes (Decision 04).
#[derive(Clone, Copy, PartialEq, Eq)]
enum MethodKind {
    Unary,
    ServerStream,
    ClientStream,
    BidiStream,
}

/// Information extracted from one trait method.
struct MethodInfo {
    /// `snake_case` variant (e.g. `"greet"`).
    snake: String,
    /// Token stream of the request inner type (e.g. `GreetRequest`).
    req_type: TokenStream,
    /// Token stream of the response inner type (e.g. `GreetResponse`).
    res_type: TokenStream,
    /// Which RPC shape the method represents.
    kind: MethodKind,
    /// Span for error reporting.
    span: proc_macro2::Span,
}

// ── Feature 27 / Decision 10 — stream classification ──────────────────────
// Each method's shape is recognised syntactically from the parameter and return
// type patterns shown in D10 §Mode 3. We never evaluate or resolve types.

/// Does this type mention `Stream` as a path segment anywhere?
///
/// WHY: Simple string matching can't distinguish `Stream` (the trait) from
/// `StreamReq` (a type named after a stream). We must walk the AST and check
/// segment identifiers exactly.
fn type_mentions_stream(ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => {
            // Check all path segments (e.g. `futures::Stream` → segment `Stream`)
            for seg in &tp.path.segments {
                if seg.ident == "Stream" {
                    return true;
                }
            }
            // Also check generic arguments (e.g. `Request<StreamReq>` — but NOT
            // `StreamReq` itself, only `Stream` as a bare ident)
            for seg in &tp.path.segments {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner_ty) = arg {
                            if type_mentions_stream(inner_ty) {
                                return true;
                            }
                        }
                    }
                }
            }
            false
        }
        Type::ImplTrait(imp) => {
            for bound in &imp.bounds {
                if let TypeParamBound::Trait(tb) = bound {
                    for seg in &tb.path.segments {
                        if seg.ident == "Stream" {
                            return true;
                        }
                    }
                    // Check associated type bindings in the trait bound
                    for seg in &tb.path.segments {
                        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                            for arg in &args.args {
                                if let syn::GenericArgument::AssocType(at) = arg {
                                    if type_mentions_stream(&at.ty) {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// True when the method has a `Stream`-typed request parameter.
fn request_is_stream(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| match arg {
        FnArg::Typed(pt) => type_mentions_stream(&pt.ty),
        _ => false,
    })
}

/// True when the return type contains `Stream`.
fn response_is_stream(ret: &ReturnType) -> bool {
    match ret {
        ReturnType::Type(_, ty) => type_mentions_stream(ty),
        _ => false,
    }
}

/// Extract the inner type from `OuterName<T, …>`.
fn extract_type_arg<'a>(ty: &'a Type, outer_name: &str) -> Option<&'a Type> {
    if let Type::Path(tp) = ty {
        for seg in &tp.path.segments {
            if seg.ident == outer_name {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(t)) = args.args.first() {
                        return Some(t);
                    }
                }
            }
        }
    }
    None
}

/// Given `T = impl Stream<Item = ConnectResult<Inner>>`, extract `Inner`.
///
/// Walks: `impl Stream<Item = …>` → `Item` binding → first `ConnectResult<…>`
/// type argument, or falls back to the whole item type.
fn extract_stream_item_inner(stream_ty: &Type) -> TokenStream {
    let item_ts = extract_stream_item_type(stream_ty);

    if let Some(item) = &item_ts {
        // Try to unwrap one layer of `ConnectResult<T>` → T
        if let Ok(item_ty) = syn::parse2::<Type>(item.clone()) {
            if let Some(inner) = extract_type_arg(&item_ty, "ConnectResult") {
                return quote!(#inner);
            }
        }
        return item.clone();
    }

    // Fallback: emit the whole stream type
    quote!(#stream_ty)
}

/// Extract the `Item = …` type from `impl Stream<Item = T>`.
fn extract_stream_item_type(ty: &Type) -> Option<TokenStream> {
    // Helper: given an angle-bracketed arg list, find `Item = T` and return T.
    let find_item = |args: &syn::AngleBracketedGenericArguments| -> Option<TokenStream> {
        for arg in &args.args {
            if let syn::GenericArgument::AssocType(at) = arg {
                if at.ident == "Item" {
                    // `&at.ty` — NOT `quote!(#at.ty)`, which would emit the whole
                    // `Item = …` binding followed by literal `.ty` tokens.
                    let item_ty = &at.ty;
                    return Some(quote!(#item_ty));
                }
            }
        }
        None
    };

    match ty {
        Type::ImplTrait(imp) => {
            for bound in &imp.bounds {
                if let TypeParamBound::Trait(tb) = bound {
                    for seg in &tb.path.segments {
                        if seg.ident == "Stream" {
                            if let PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(result) = find_item(args) {
                                    return Some(result);
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        Type::Path(tp) => {
            for seg in &tp.path.segments {
                if seg.ident == "Stream" {
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(result) = find_item(args) {
                            return Some(result);
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the response type from `ConnectResult<Response<T>>`.
fn extract_response_from_connect_result(ty: &Type) -> Option<TokenStream> {
    // Walk: ConnectResult<T> → T, then check for Response<U> → U
    if let Some(inner) = extract_type_arg(ty, "ConnectResult") {
        // inner is Response<U> → extract U
        if let Some(res) = extract_type_arg(inner, "Response") {
            return Some(quote!(#res));
        }
        // Maybe inner itself is the response type (e.g. ConnectResult<MyRes>)
        return Some(quote!(#inner));
    }
    None
}

/// Extract the request type from the method signature.
///
/// The request parameter is the second non-self param (after `ctx: Ctx`).
/// It can be `Request<T>` (unary / server-stream) or
/// `impl Stream<Item = ConnectResult<T>>` (client-stream / bidi).
fn extract_req_type(sig: &syn::Signature) -> TokenStream {
    let non_self: Vec<&FnArg> = sig
        .inputs
        .iter()
        .filter(|a| !matches!(a, FnArg::Receiver(_)))
        .collect();

    // non_self[0] = ctx: Ctx, non_self[1] = request param
    if non_self.len() < 2 {
        return quote!(()); // Fallback
    }

    if let FnArg::Typed(PatType { ty, .. }) = non_self[1] {
        // Request<T> → T
        if let Some(t) = extract_type_arg(ty, "Request") {
            return quote!(#t);
        }

        // impl Stream<Item = ConnectResult<T>> → T
        if type_mentions_stream(ty) {
            return extract_stream_item_inner(ty);
        }

        // Fallback: emit the type as-is
        quote!(#ty)
    } else { quote!(()) }
}

/// Extract the response type from the return type.
///
/// Patterns handled:
/// - `ConnectResult<Response<T>>` → `T` (unary / client-stream)
/// - `ConnectResult<impl Stream<Item = ConnectResult<T>>>` → `T` (server / bidi)
fn extract_res_type(ret: &ReturnType) -> TokenStream {
    if let ReturnType::Type(_, ty) = ret {
        // Streaming FIRST: `ConnectResult<impl Stream<Item = ConnectResult<T>>>` → `T`.
        // This must precede the `Response` unwrap: `extract_response_from_connect_result`
        // falls back to returning the whole `ConnectResult` inner when it finds no
        // `Response<…>`, which for a streaming method is the `impl Stream<…>` type —
        // the wrong answer (and illegal nested `impl Trait` downstream).
        if let Some(inner) = extract_type_arg(ty, "ConnectResult") {
            if type_mentions_stream(inner) {
                return extract_stream_item_inner(inner);
            }
        }

        // Unary / client-stream: `ConnectResult<Response<T>>` → `T`.
        if let Some(r) = extract_response_from_connect_result(ty) {
            return r;
        }

        // Fallback
        quote!(#ty)
    } else { quote!(()) }
}

/// Extract method information from a trait method.
fn extract_method_info(method: &syn::TraitItemFn) -> MethodInfo {
    use syn::spanned::Spanned;

    let name = method.sig.ident.to_string();
    let snake = to_snake_case(&name);

    let req_stream = request_is_stream(&method.sig);
    let res_stream = response_is_stream(&method.sig.output);

    let kind = match (req_stream, res_stream) {
        (false, false) => MethodKind::Unary,
        (false, true) => MethodKind::ServerStream,
        (true, false) => MethodKind::ClientStream,
        (true, true) => MethodKind::BidiStream,
    };

    let req_type = extract_req_type(&method.sig);
    let res_type = extract_res_type(&method.sig.output);
    let span = method.sig.span();

    MethodInfo { snake, req_type, res_type, kind, span }
}

// ── Name conversion ────────────────────────────────────────────────────────

/// Convert `snake_case` to `PascalCase` (e.g. `greet_group` → `GreetGroup`).
fn to_pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().to_string() + chars.as_str()
                }
            }
        })
        .collect()
}
fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next = chars.get(i + 1).copied();
                let prev_lower = prev.is_lowercase() || prev.is_ascii_digit();
                let next_lower = next.is_some_and(char::is_lowercase);
                if (prev_lower || (next_lower && i + 1 < len))
                    && result.as_bytes().last() != Some(&b'_') {
                        result.push('_');
                    }
            }
            result.push(c.to_ascii_lowercase());
        } else if c == '-' {
            result.push('_');
        } else {
            result.push(c);
        }
    }
    result
}

// ── Supertrait bounds ──────────────────────────────────────────────────────

/// Ensure the trait has `Send + Sync + 'static` bounds, adding them if absent.
fn ensure_send_sync_static(trait_def: &mut ItemTrait) {
    let needs: Vec<&str> = {
        let has = |name: &str| -> bool {
            trait_def.supertraits.iter().any(|b| match b {
                TypeParamBound::Trait(tb) => tb.path.is_ident(name),
                _ => false,
            })
        };

        let mut v = Vec::new();
        if !has("Send") {
            v.push("Send");
        }
        if !has("Sync") {
            v.push("Sync");
        }
        if !has("static") {
            v.push("static");
        }
        v
    };

    for name in needs {
        if name == "static" {
            trait_def.supertraits.push(syn::parse_quote!('static));
        } else {
            let id: Ident = Ident::new(name, proc_macro2::Span::call_site());
            trait_def.supertraits.push(syn::parse_quote!(#id));
        }
    }
}

// ── Default method bodies ──────────────────────────────────────────────────

/// Build a default unimplemented body for the given method kind and type info.
///
/// The body is a bare `Err(...)` for every kind: `desugar_async_methods_to_send`
/// pins a concrete return type (`Response<Res>` for unary/client-stream, a boxed
/// `Pin<Box<dyn Stream + Send>>` for server/bidi-stream), so the `Err` Ok-type
/// infers with no annotation — no hidden `impl Stream` to pin.
fn make_default_body(
    kind: MethodKind,
    path_const: &Ident,
    _res_type: &TokenStream,
) -> syn::Block {
    let tokens = match kind {
        MethodKind::Unary | MethodKind::ClientStream => {
            quote! {{
                Err(foundation_connectrpc::ConnectError::unimplemented(procedure::#path_const).into())
            }}
        }
        MethodKind::ServerStream | MethodKind::BidiStream => {
            quote! {{
                Err(foundation_connectrpc::ConnectError::unimplemented(procedure::#path_const).into())
            }}
        }
    };
    syn::parse2(tokens).expect("connectrpc_service: failed to parse default body")
}

/// Desugar `async fn` trait methods to `fn … -> impl Future<Output = …> + Send`
/// (Decision 10 §S2 / "codegen emits the desugared `+ Send` form").
///
/// WHY: `Router::{unary,server_stream,client_stream,bidi_stream}` bound the
/// handler future as `Future<…> + Send + 'static`. A native `async fn` in a trait
/// desugars to a bare `-> impl Future` with **no `Send` guarantee**, so
/// `register_<svc>` (generic over `S: <Svc>`) cannot prove the bound and fails to
/// compile with "future cannot be sent between threads safely". Pinning `+ Send`
/// on the trait's return-position `impl Future` makes every implementation's
/// future `Send` by contract (each impl's `async fn` is checked against it).
///
/// **Streaming return shape:** a server/bidi-stream method is authored as
/// `-> ConnectResult<impl Stream<Item = …>>`, but `impl Trait` nested inside
/// `ConnectResult<…>` (and again inside the `impl Future<Output = …>` we add) is
/// illegal (`E0562`). So for streaming kinds we rewrite the `Output` to a **boxed
/// stream** — `ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<Res>> + Send>>>`
/// — a concrete type. Implementors return `Ok(Box::pin(stream))`; the box is
/// `Stream + Send + 'static`, satisfying the Router's streaming bound.
///
/// Runs AFTER `add_default_bodies`, reading each method's kind/response type from
/// its original signature before rewriting it. A generated/user body block `{ … }`
/// becomes `{ async move { … } }` so it still produces the future.
fn desugar_async_methods_to_send(trait_def: &mut ItemTrait) {
    for item in &mut trait_def.items {
        if let TraitItem::Fn(method) = item {
            // Only rewrite `async fn`; leave already-desugared / sync methods alone.
            if method.sig.asyncness.is_none() {
                continue;
            }
            // Read kind + response type from the *original* signature first.
            let info = extract_method_info(method);
            method.sig.asyncness = None;

            let out_ty: TokenStream = match info.kind {
                // Return type is concrete (`ConnectResult<Response<Res>>`) — keep verbatim.
                MethodKind::Unary | MethodKind::ClientStream => match &method.sig.output {
                    ReturnType::Default => quote!(()),
                    ReturnType::Type(_, ty) => quote!(#ty),
                },
                // Box the stream so nothing is `impl Trait` but the outer future.
                MethodKind::ServerStream | MethodKind::BidiStream => {
                    let res = &info.res_type;
                    quote! {
                        foundation_connectrpc::ConnectResult<
                            ::core::pin::Pin<Box<
                                dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<#res>> + Send
                            >>
                        >
                    }
                }
            };
            method.sig.output = syn::parse_quote!(
                -> impl ::core::future::Future<Output = #out_ty> + Send
            );
            if let Some(block) = method.default.take() {
                method.default = Some(syn::parse_quote!({ async move #block }));
            }
        }
    }
}

/// Add default bodies to trait methods that lack them.
fn add_default_bodies(trait_def: &mut ItemTrait) {
    let items = std::mem::take(&mut trait_def.items);
    for item in items {
        let mut item = item;
        if let TraitItem::Fn(method) = &mut item {
            if method.default.is_some() {
                // Method already has a default — preserve it.
                trait_def.items.push(TraitItem::Fn(method.clone()));
                continue;
            }
            if method.semi_token.is_none() {
                // Neither body nor semicolon — shouldn't happen, but skip.
                trait_def.items.push(TraitItem::Fn(method.clone()));
                continue;
            }

            let info = extract_method_info(method);
            let path_const = Ident::new(&info.snake.to_uppercase(), info.span);

            method.default = Some(make_default_body(info.kind, &path_const, &info.res_type));
            method.semi_token = None;
        }
        trait_def.items.push(item);
    }
}

// ── Codec table expression ─────────────────────────────────────────────────

/// Generate the `ProcedureCodecs::<Req, Res>::of(…)` expression for the
/// requested codec families.
fn codec_expr(codecs: &[String], req_type: &TokenStream, res_type: &TokenStream) -> TokenStream {
    let has_proto = codecs.iter().any(|c| c == "proto");
    let has_json = codecs.iter().any(|c| c == "json");
    let has_arrow = codecs.iter().any(|c| c == "arrow");

    if has_proto {
        // proto always pairs with json
        quote! { foundation_connectrpc::ProcedureCodecs::<#req_type, #res_type>::defaults() }
    } else if has_arrow && !has_json {
        // Arrow-only
        quote! { foundation_connectrpc::ProcedureCodecs::<#req_type, #res_type>::only(foundation_connectrpc::ArrowCodec) }
    } else if has_json && !has_arrow {
        // Json-only
        quote! { foundation_connectrpc::ProcedureCodecs::<#req_type, #res_type>::of((foundation_connectrpc::JsonCodec,)) }
    } else {
        // Both json + arrow (or default = json)
        let inner = if has_arrow {
            quote! { (foundation_connectrpc::JsonCodec, foundation_connectrpc::ArrowCodec) }
        } else {
            quote! { (foundation_connectrpc::JsonCodec,) }
        };
        quote! { foundation_connectrpc::ProcedureCodecs::<#req_type, #res_type>::of(#inner) }
    }
}

// ── Main expansion ─────────────────────────────────────────────────────────

/// Entry point for the `#[service]` attribute macro.
/// Accepts `proc_macro2::TokenStream` (converted from `proc_macro::TokenStream`
/// by the `#[proc_macro_attribute]` wrapper in `lib.rs`).
pub fn expand_service(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args: ServiceAttr = match parse2(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let mut input_trait: ItemTrait = match parse2(item) {
        Ok(t) => t,
        Err(e) => return e.to_compile_error(),
    };

    let svc_ident = input_trait.ident.clone();
    let svc_name = svc_ident.to_string();
    let package = &attr_args.package;
    let codecs = &attr_args.codecs;
    let crate_span = proc_macro2::Span::call_site();

    // Collect method infos
    let methods: Vec<MethodInfo> = input_trait
        .items
        .iter()
        .filter_map(|item| match item {
            TraitItem::Fn(m) => Some(extract_method_info(m)),
            _ => None,
        })
        .collect();

    // Ensure Send + Sync + 'static bounds
    ensure_send_sync_static(&mut input_trait);

    // Add default bodies
    add_default_bodies(&mut input_trait);

    // Desugar `async fn` → `-> impl Future + Send` so `register_<svc>` satisfies
    // the Router's `Fut: Send` bound (Decision 10 §S2).
    desugar_async_methods_to_send(&mut input_trait);

    // ── Identifiers for generated items ───────────────────────────────────
    let name_const_ident = Ident::new(&(svc_name.to_uppercase() + "_NAME"), crate_span);
    let svc_snake = to_snake_case(&svc_name);
    let register_fn_ident = format_ident!("register_{}", svc_snake);
    let unimplemented_ident = format_ident!("Unimplemented{}Handler", svc_name);
    let client_struct_ident = format_ident!("{}Client", svc_name);
    let client_trait_ident = format_ident!("{}ClientExt", svc_name);
    let descriptor_macro_ident = format_ident!("{}_rpc_definitions", svc_snake);

    let qualified_name = format!("{package}.{svc_name}");

    // ── Procedure constants ───────────────────────────────────────────────
    let procedure_consts: Vec<TokenStream> = methods
        .iter()
        .map(|m| {
            let const_name = Ident::new(&m.snake.to_uppercase(), m.span);
            let pascal_name = to_pascal_case(&m.snake);
            let path = format!("/{package}.{svc_name}/{pascal_name}");
            quote! {
                /// Procedure path for #path.
                pub const #const_name: &str = #path;
            }
        })
        .collect();

    // ── Registration function ─────────────────────────────────────────────
    let registration_arms: Vec<TokenStream> = methods
        .iter()
        .map(|m| {
            let path_const = Ident::new(&m.snake.to_uppercase(), m.span);
            let req_type = &m.req_type;
            let res_type = &m.res_type;
            let method_name = Ident::new(&m.snake, m.span);
            let codec = codec_expr(codecs, req_type, res_type);

            let register_call = match m.kind {
                MethodKind::Unary => quote! { router.unary },
                MethodKind::ServerStream => quote! { router.server_stream },
                MethodKind::ClientStream => quote! { router.client_stream },
                MethodKind::BidiStream => quote! { router.bidi_stream },
            };

            let handler_args = match m.kind {
                MethodKind::Unary | MethodKind::ServerStream => {
                    quote! { move |ctx, req| { let svc = svc.clone(); async move { svc.#method_name(ctx, req).await } } }
                }
                MethodKind::ClientStream | MethodKind::BidiStream => {
                    quote! { move |ctx, reqs| { let svc = svc.clone(); async move { svc.#method_name(ctx, reqs).await } } }
                }
            };

            quote! {
                {
                    let svc = service.clone();
                    #register_call(
                        procedure::#path_const,
                        #codec,
                        #handler_args,
                        foundation_connectrpc::HandlerOptions::new(),
                    );
                }
            }
        })
        .collect();

    // ── Client fields ─────────────────────────────────────────────────────
    let client_fields: Vec<TokenStream> = methods
        .iter()
        .map(|m| {
            let field = Ident::new(&m.snake, m.span);
            let rt = &m.req_type;
            let rst = &m.res_type;
            quote! {
                #field: foundation_connectrpc::Client<#rt, #rst>,
            }
        })
        .collect();

    // ── Client new() field initialisers ───────────────────────────────────
    let client_new_fields: Vec<TokenStream> = methods
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let field = Ident::new(&m.snake, m.span);
            let path_const = Ident::new(&m.snake.to_uppercase(), m.span);
            let rt = &m.req_type;
            let rst = &m.res_type;
            let codec = codec_expr(codecs, rt, rst);

            // The last field uses `options` without `.clone()`
            if i == methods.len() - 1 {
                quote! {
                    #field: foundation_connectrpc::Client::new(
                        transport.clone(),
                        &format!("{}{}", base_url, procedure::#path_const),
                        #codec,
                        options,
                    )?,
                }
            } else {
                quote! {
                    #field: foundation_connectrpc::Client::new(
                        transport.clone(),
                        &format!("{}{}", base_url, procedure::#path_const),
                        #codec,
                        options.clone(),
                    )?,
                }
            }
        })
        .collect();

    // ── Client per-method accessors ───────────────────────────────────────
    let client_methods: Vec<TokenStream> = methods
        .iter()
        .map(|m| {
            let method_name = Ident::new(&m.snake, m.span);
            let rt = &m.req_type;
            let rst = &m.res_type;

            match m.kind {
                MethodKind::Unary => {
                    quote! {
                        /// Unary RPC.
                        pub async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            request: foundation_connectrpc::Request<#rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<#rst>> {
                            self.#method_name.unary(ctx, request).await
                        }
                    }
                }
                MethodKind::ServerStream => {
                    quote! {
                        /// Server-streaming RPC.
                        pub async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            request: foundation_connectrpc::Request<#rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::ServerStream<#rst>> {
                            self.#method_name.server_stream(ctx, request).await
                        }
                    }
                }
                MethodKind::ClientStream => {
                    quote! {
                        /// Client-streaming RPC.
                        pub async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            reqs: impl futures::Stream<Item = #rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<#rst>> {
                            self.#method_name.client_stream(ctx, reqs).await
                        }
                    }
                }
                MethodKind::BidiStream => {
                    quote! {
                        /// Bidirectional streaming RPC.
                        pub async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            reqs: impl futures::Stream<Item = #rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::BidiStream<#rt, #rst>> {
                            self.#method_name.bidi_stream(ctx, reqs).await
                        }
                    }
                }
            }
        })
        .collect();

    // ── Client trait methods ──────────────────────────────────────────────
    let client_trait_methods: Vec<TokenStream> = methods
        .iter()
        .map(|m| {
            let method_name = Ident::new(&m.snake, m.span);
            let rt = &m.req_type;
            let rst = &m.res_type;

            match m.kind {
                MethodKind::Unary => {
                    quote! {
                        /// Unary RPC.
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            request: foundation_connectrpc::Request<#rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<#rst>>;
                    }
                }
                MethodKind::ServerStream => {
                    quote! {
                        /// Server-streaming RPC.
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            request: foundation_connectrpc::Request<#rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::ServerStream<#rst>>;
                    }
                }
                MethodKind::ClientStream => {
                    quote! {
                        /// Client-streaming RPC.
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            reqs: impl futures::Stream<Item = #rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<#rst>>;
                    }
                }
                MethodKind::BidiStream => {
                    quote! {
                        /// Bidirectional streaming RPC.
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            reqs: impl futures::Stream<Item = #rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::BidiStream<#rt, #rst>>;
                    }
                }
            }
        })
        .collect();

    // ── Blanket impl methods for client trait ─────────────────────────────
    let blanket_impl_methods: Vec<TokenStream> = methods
        .iter()
        .map(|m| {
            let method_name = Ident::new(&m.snake, m.span);
            let rt = &m.req_type;
            let rst = &m.res_type;

            match m.kind {
                MethodKind::Unary => {
                    quote! {
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            request: foundation_connectrpc::Request<#rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<#rst>> {
                            self.#method_name.unary(ctx, request).await
                        }
                    }
                }
                MethodKind::ServerStream => {
                    quote! {
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            request: foundation_connectrpc::Request<#rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::ServerStream<#rst>> {
                            self.#method_name.server_stream(ctx, request).await
                        }
                    }
                }
                MethodKind::ClientStream => {
                    quote! {
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            reqs: impl futures::Stream<Item = #rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<#rst>> {
                            self.#method_name.client_stream(ctx, reqs).await
                        }
                    }
                }
                MethodKind::BidiStream => {
                    quote! {
                        async fn #method_name(
                            &self,
                            ctx: foundation_connectrpc::Ctx,
                            reqs: impl futures::Stream<Item = #rt>,
                        ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::BidiStream<#rt, #rst>> {
                            self.#method_name.bidi_stream(ctx, reqs).await
                        }
                    }
                }
            }
        })
        .collect();

    // ── Assemble artefacts per group ─────────────────────────────────────
    //
    // The items are split into three groups so that the cross-crate
    // `generate!` macro can emit only what the consumer asked for:
    //
    //   common — always emitted (service name const + procedure paths)
    //   server — trait + registration fn + Unimplemented*Handler
    //   client — typed Client struct + ClientExt trait

    let generated_common = quote! {
        /// Fully-qualified service name.
        pub const #name_const_ident: &str = #qualified_name;

        /// Procedure path constants — leading slash included (R1).
        pub mod procedure {
            #(#procedure_consts)*
        }
    };

    let generated_server = quote! {
        #input_trait

        /// Register a #svc_name implementation with a ConnectRPC router.
        pub fn #register_fn_ident<S: #svc_ident>(
            router: &mut foundation_connectrpc::Router,
            service: std::sync::Arc<S>,
        ) {
            #(#registration_arms)*
        }

        /// An unimplemented #svc_name handler — all methods return
        /// `unimplemented`.
        pub struct #unimplemented_ident;
        impl #svc_ident for #unimplemented_ident {}
    };

    let generated_client = quote! {
        /// Typed client for #svc_name.
        pub struct #client_struct_ident {
            #(#client_fields)*
        }

        impl #client_struct_ident {
            /// Build a new typed client.
            pub fn new(
                transport: std::sync::Arc<dyn foundation_connectrpc::Transport>,
                base_url: &str,
                options: foundation_connectrpc::ClientOptions,
            ) -> foundation_connectrpc::ConnectResult<Self> {
                Ok(Self {
                    #(#client_new_fields)*
                })
            }

            #(#client_methods)*
        }

        /// Trait for #svc_name client behaviour (supports mocking/testing).
        pub trait #client_trait_ident: Send + Sync + 'static {
            #(#client_trait_methods)*
        }

        impl #client_trait_ident for #client_struct_ident {
            #(#blanket_impl_methods)*
        }
    };

    // ── Descriptor macro for cross-crate generation ───────────────────────
    //
    // Arms: (common), (server), (client) — each emits one artefact group.
    // The arm-less `()` form emits everything (backward compat when no
    // artifact filter is given). `generate!` always emits `(common)` first,
    // then conditionally emits `(server)` / `(client)` based on user input.
    quote! {
        #generated_common
        #generated_server
        #generated_client

        #[macro_export]
        macro_rules! #descriptor_macro_ident {
            (common) => { #generated_common };
            (server) => { #generated_server };
            (client) => { #generated_client };
            () => {
                #generated_common
                #generated_server
                #generated_client
            };
        }
    }
}

// ── generate! macro ────────────────────────────────────────────────────────

/// Parsed input for `generate!(path => mod name { server, client })`.
struct GenerateInput {
    /// The path to the descriptor macro (e.g. `my_api::greet_service_rpc_definitions`).
    path: syn::Path,
    /// The output module name.
    module_name: Ident,
    /// Artifact list (e.g. `server`, `client`) — which groups to emit.
    artifacts: Vec<Ident>,
}

impl Parse for GenerateInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let path = input.parse::<syn::Path>()?;
        input.parse::<Token![=>]>()?;
        input.parse::<Token![mod]>()?;
        let module_name = input.parse::<Ident>()?;
        let content;
        syn::braced!(content in input);
        let mut artifacts = Vec::new();
        while !content.is_empty() {
            artifacts.push(content.parse::<Ident>()?);
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        Ok(GenerateInput { path, module_name, artifacts })
    }
}

/// Entry point for the `foundation_connectrpc::generate!` function-like macro.
/// Accepts `proc_macro2::TokenStream` (converted by `lib.rs`).
pub fn expand_generate(input: TokenStream) -> TokenStream {
    let gi: GenerateInput = match parse2(input) {
        Ok(g) => g,
        Err(e) => return e.to_compile_error(),
    };
    let mod_name = &gi.module_name;
    let path = &gi.path;

    let has_server = gi.artifacts.iter().any(|a| a == "server");
    let has_client = gi.artifacts.iter().any(|a| a == "client");

    let mut invocations = TokenStream::new();

    if has_server || has_client {
        // Common artefacts (service name + procedure paths) are always needed
        // when either group is requested.
        invocations.extend(quote! { #path ! (common) ; });
    }
    if has_server {
        invocations.extend(quote! { #path ! (server) ; });
    }
    if has_client {
        invocations.extend(quote! { #path ! (client) ; });
    }
    // If neither was specified (empty `{}`), emit everything for backward compat.
    if !has_server && !has_client {
        invocations.extend(quote! { #path ! () ; });
    }

    quote! {
        pub mod #mod_name {
            #invocations
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    // ── Tests ───────────────────────────────────────────────────────────────

    /// WHY: First TDD test — verify a basic unary method produces the expected
    /// generated items: service name constant, procedure module, trait with
    /// default body, registration fn, UnimplementedHandler, client struct,
    /// and client trait.
    #[test]
    fn test_expand_basic_unary() {
        let attr = quote! { package = "test.v1" };
        let item = quote! {
            pub trait GreetService {
                async fn greet(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    req: foundation_connectrpc::Request<GreetRequest>,
                ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<GreetResponse>>;
            }
        };

        let output = expand_service(attr, item);
        let out = output.to_string();

        // Service name constant (R3)
        assert!(out.contains("test.v1.GreetService"), "missing service name constant");

        // Procedure constants (R1)
        assert!(out.contains("GREET"), "missing GREET procedure constant name");
        assert!(out.contains("/test.v1.GreetService/Greet"), "missing procedure path");

        // Procedure module
        assert!(out.contains("pub mod procedure"), "missing procedure module");

        // Service trait with Send + Sync + 'static
        assert!(
            out.contains("GreetService : Send + Sync + 'static"),
            "missing Send + Sync + 'static bounds"
        );

        // Unimplemented handler (R2)
        assert!(
            out.contains("UnimplementedGreetServiceHandler"),
            "missing UnimplementedHandler"
        );

        // Registration function
        assert!(out.contains("register_greet_service"), "missing register fn");

        // Client struct (R4)
        assert!(out.contains("GreetServiceClient"), "missing client struct");

        // Client trait
        assert!(out.contains("GreetServiceClientExt"), "missing client trait");

        // Default body: ConnectError::unimplemented(procedure::GREET)
        assert!(
            out.contains("ConnectError :: unimplemented (procedure :: GREET)"),
            "missing default unimplemented body"
        );

        // Descriptor macro export
        assert!(out.contains("greet_service_rpc_definitions"), "missing descriptor macro");
        assert!(out.contains("macro_export"), "missing macro_export on descriptor macro");
    }

    /// WHY: Verify all four RPC kinds are correctly classified and generate
    /// the right registration calls.
    #[test]
    fn test_all_four_rpc_kinds() {
        let attr = quote! { package = "test.v1" };
        let item = quote! {
            pub trait FullService {
                async fn unary_method(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    req: foundation_connectrpc::Request<UnaryReq>,
                ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<UnaryRes>>;

                async fn server_stream_method(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    req: foundation_connectrpc::Request<StreamReq>,
                ) -> foundation_connectrpc::ConnectResult<
                    impl futures::Stream<Item = foundation_connectrpc::ConnectResult<StreamRes>>,
                >;

                async fn client_stream_method(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    reqs: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<StreamReq>>,
                ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<StreamRes>>;

                async fn bidi_stream_method(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    reqs: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<StreamReq>>,
                ) -> foundation_connectrpc::ConnectResult<
                    impl futures::Stream<Item = foundation_connectrpc::ConnectResult<StreamRes>>,
                >;
            }
        };

        let output = expand_service(attr, item);
        let out_str = output.to_string();

        // All four procedure constant paths (string literals preserved verbatim)
        assert!(out_str.contains("/test.v1.FullService/UnaryMethod"));
        assert!(out_str.contains("/test.v1.FullService/ServerStreamMethod"));
        assert!(out_str.contains("/test.v1.FullService/ClientStreamMethod"));
        assert!(out_str.contains("/test.v1.FullService/BidiStreamMethod"));

        // All four router registration calls
        assert!(out_str.contains("router . unary ("));
        assert!(out_str.contains("router . server_stream ("));
        assert!(out_str.contains("router . client_stream ("));
        assert!(out_str.contains("router . bidi_stream ("));

        // Client uses the right receiver methods (field name = snake_case method)
        assert!(out_str.contains("self . unary_method . unary"));
        assert!(out_str.contains("self . server_stream_method . server_stream"));
        assert!(out_str.contains("self . client_stream_method . client_stream"));
        assert!(out_str.contains("self . bidi_stream_method . bidi_stream"));
    }

    /// WHY: Verify snake_case conversion is applied correctly.
    #[test]
    fn test_method_name_conversion() {
        let attr = quote! { package = "p.v1" };
        let item = quote! {
            pub trait ConvTest {
                async fn greet_group(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    req: foundation_connectrpc::Request<R>,
                ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<Res>>;
            }
        };

        let output = expand_service(attr, item);
        let out = output.to_string();

        // Procedure constant uses PascalCase path and SCREAMING_SNAKE_CASE name
        assert!(out.contains("GREET_GROUP"), "missing GREET_GROUP const name");
        assert!(out.contains("/p.v1.ConvTest/GreetGroup"), "missing procedure path");

        // Registration fn uses snake_case
        assert!(out.contains("register_conv_test"), "missing register fn");

        // Client field uses snake_case
        assert!(
            out.contains("greet_group : foundation_connectrpc :: Client < R , Res >"),
            "client field should use snake_case"
        );
    }

    /// WHY: Verify codecs attribute parsing generates the right codec expression.
    #[test]
    fn test_codec_json_expr() {
        let attr = quote! { package = "test.v1", codecs(json) };
        let item = quote! {
            pub trait JsonSvc {
                async fn call(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    req: foundation_connectrpc::Request<Req>,
                ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<Res>>;
            }
        };

        let output = expand_service(attr, item);
        let out = output.to_string();

        assert!(
            out.contains("ProcedureCodecs :: < Req , Res > :: of ((foundation_connectrpc :: JsonCodec ,))"),
            "json codecs should produce ProcedureCodecs::of((JsonCodec,))"
        );
    }

    /// WHY: Verify that protocol buffer codecs use defaults().
    #[test]
    fn test_codec_proto_expr() {
        let attr = quote! { package = "test.v1", codecs(proto) };
        let item = quote! {
            pub trait ProtoSvc {
                async fn call(
                    &self,
                    ctx: foundation_connectrpc::Ctx,
                    req: foundation_connectrpc::Request<PReq>,
                ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<PRes>>;
            }
        };

        let output = expand_service(attr, item);
        let out = output.to_string();

        assert!(
            out.contains("ProcedureCodecs :: < PReq , PRes > :: defaults ()"),
            "proto codecs should produce defaults()"
        );
    }

    /// WHY: Verify that `generate!` with `{ server, client }` emits
    /// (common), (server), and (client) arms.
    #[test]
    fn test_generate_expansion() {
        let input = quote! { foo::bar => mod my_mod { server, client } };
        let output = expand_generate(input);
        let out = output.to_string();

        assert!(out.contains("pub mod my_mod"));
        // Emits all three filtered arms, not the unfiltered `()` form.
        assert!(out.contains("foo :: bar ! (common)"));
        assert!(out.contains("foo :: bar ! (server)"));
        assert!(out.contains("foo :: bar ! (client)"));
    }

    /// WHY: Verify `generate!` with only `{ server }` emits common + server,
    /// and not client or the unfiltered form.
    #[test]
    fn test_generate_server_only() {
        let input = quote! { my_crate::my_mod::tokens => mod svc { server } };
        let output = expand_generate(input);
        let out = output.to_string();

        assert!(out.contains("pub mod svc"));
        assert!(out.contains("tokens ! (common)"));
        assert!(out.contains("tokens ! (server)"));
        assert!(!out.contains("tokens ! (client)"));
        assert!(!out.contains("tokens ! ()"));
    }

    /// WHY: Verify `generate!` with only `{ client }` emits common + client,
    /// not server.
    #[test]
    fn test_generate_client_only() {
        let input = quote! { api::svc_rpc_definitions => mod cli { client } };
        let output = expand_generate(input);
        let out = output.to_string();

        assert!(out.contains("pub mod cli"));
        assert!(out.contains("svc_rpc_definitions ! (common)"));
        assert!(!out.contains("svc_rpc_definitions ! (server)"));
        assert!(out.contains("svc_rpc_definitions ! (client)"));
    }

    /// WHY: Verify empty `{}` falls back to the unfiltered `()` form
    /// (backward compat).
    #[test]
    fn test_generate_empty_braces() {
        let input = quote! { api::tokens => mod m {} };
        let output = expand_generate(input);
        let out = output.to_string();

        assert!(out.contains("pub mod m"));
        assert!(out.contains("api :: tokens ! ()"));
        assert!(!out.contains("tokens ! (common)"));
        assert!(!out.contains("tokens ! (server)"));
        assert!(!out.contains("tokens ! (client)"));
    }

    /// WHY: Verify the `to_snake_case` conversion function.
    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Greet"), "greet");
        assert_eq!(to_snake_case("GreetGroup"), "greet_group");
        assert_eq!(to_snake_case("GetURL"), "get_url");
        assert_eq!(to_snake_case("ParseJSON"), "parse_json");
        assert_eq!(to_snake_case("Simple"), "simple");
        assert_eq!(to_snake_case("HTML"), "html");
    }

    /// WHY: Verify the `to_pascal_case` conversion function.
    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("greet"), "Greet");
        assert_eq!(to_pascal_case("greet_group"), "GreetGroup");
        assert_eq!(to_pascal_case("get_url"), "GetUrl");
        assert_eq!(to_pascal_case("parse_json"), "ParseJson");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case("a"), "A");
        assert_eq!(to_pascal_case(""), "");
    }
}
