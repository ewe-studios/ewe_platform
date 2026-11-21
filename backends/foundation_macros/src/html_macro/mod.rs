//! WHY: Components want to express DOM structure as HTML, but the framework is
//! WASM-first with zero runtime parsing (decision 001) — so the HTML must
//! become Rust code at compile time, with stable element ids (decision 005)
//! and `Part` descriptors marking every dynamic slot.
//!
//! WHAT: The `html!` proc-macro body. Two forms:
//!
//! ```ignore
//! // PURE — an Html value (snapshots, SSR, tests):
//! let tree = html! { <div class="card"><span>{count.get()}</span></div> };
//!
//! // REACTIVE — also queues the DOM build on `receiver` and creates one
//! // effect per dynamic slot/attr (they run immediately, decision 008):
//! let tree = html! { ctx, receiver,
//!     <div class="card">
//!         <span>{count.get()}</span>
//!         <input primal:onchange={set_name} value={name.get()} />
//!     </div>
//! };
//! ```
//!
//! No template directives — `@for`/`@if` don't exist; control flow is plain
//! Rust around (or inside) the macro. `primal:onX={h}` is the only namespaced
//! attribute; when `h` is a `SignalSetter` its `callback_id` lands in a
//! `primal:setter` attribute (two-way binding, decision 029 / G21).
//!
//! HOW: `parser` walks `TokenTree`s into a template tree; `codegen` numbers it
//! (element ids for `Part`s, runtime indexes for the reactive id block) and
//! emits typed `foundation_ui_traits::Html` construction plus, reactively, the
//! operation build + effects. See each submodule's docs for the details.

mod codegen;
mod parser;
pub(crate) mod scoped_css;

use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::{
    parse::{ParseStream, Parser},
    Expr, Token,
};

use crate::crate_paths::foundation_ui_runtime_path;
use parser::{ParseError, ParseResult, ParsedAttr, ParsedNode};

fn output(result: ParseResult<TokenStream>) -> TokenStream {
    result.unwrap_or_else(|error| {
        let message = error.message;
        let span = error.span;
        quote::quote_spanned! {span=> compile_error!(#message) }
    })
}

fn starts_template(input: &TokenStream) -> bool {
    matches!(input.clone().into_iter().next(), Some(TokenTree::Punct(p)) if p.as_char() == '<')
}

/// Read complete Rust expressions before the template, including generic arguments.
fn fields(input: TokenStream) -> ParseResult<(Vec<Expr>, TokenStream)> {
    (|input: ParseStream<'_>| {
        let mut fields = Vec::new();
        while !input.is_empty() && !input.peek(Token![<]) {
            fields.push(input.parse::<Expr>()?);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok((fields, input.parse::<TokenStream>()?))
    })
    .parse2(input)
    .map_err(|error| ParseError::new(error.span(), error.to_string()))
}

fn bare_none(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(path) if path.path.is_ident("None"))
}

fn state_expr(state: Option<&Expr>) -> TokenStream {
    match state {
        Some(expr) if !bare_none(expr) => quote!(#expr),
        _ => quote!(::core::option::Option::None::<&()>),
    }
}

fn needs_view(node: &ParsedNode) -> bool {
    matches!(node, ParsedNode::Element { tag, attrs, children, .. }
        if parser::core_tag(tag) || attrs.iter().any(|attr| matches!(attr, ParsedAttr::StateMarker(_) | ParsedAttr::Action { .. })) || children.iter().any(needs_view))
}

fn state_marker(node: &ParsedNode) -> Option<proc_macro2::Span> {
    let ParsedNode::Element {
        attrs, children, ..
    } = node
    else {
        return None;
    };
    attrs
        .iter()
        .find_map(|attr| match attr {
            ParsedAttr::StateMarker(span) => Some(*span),
            _ => None,
        })
        .or_else(|| children.iter().find_map(state_marker))
}

pub(crate) fn html(input: TokenStream, action_view: bool) -> TokenStream {
    output(template(input, action_view))
}

fn template(input: TokenStream, action_view: bool) -> ParseResult<TokenStream> {
    if input.is_empty() || starts_template(&input) {
        let tree = parser::parse_template(input)?;
        reject_unbound_marker(&tree)?;
        return Ok(codegen::generate_pure(tree, action_view));
    }
    let (fields, input) = fields(input)?;
    let [ctx, out, state @ ..] = fields.as_slice() else {
        return Err(form_error());
    };
    if state.len() > 1 || input.is_empty() {
        return Err(form_error());
    }
    let tree = parser::parse_template(input)?;
    if state.first().is_none_or(bare_none) {
        reject_unbound_marker(&tree)?;
    }
    let legacy_receiver = !action_view
        && (proc_macro_crate::crate_name("foundation_wasm_ui").is_ok()
            || proc_macro_crate::crate_name("foundation_ui_runtime").is_err());
    if legacy_receiver && !needs_view(&tree) && state.first().is_none_or(bare_none) {
        return Ok(codegen::generate_reactive(
            &ctx.to_token_stream(),
            &out.to_token_stream(),
            tree,
        ));
    }
    let runtime = foundation_ui_runtime_path();
    let html = codegen::generate_pure(tree, true);
    let state = state_expr(state.first());
    Ok(quote! { #runtime::view::render(&(#ctx), &(#out), #html, #state)? })
}

fn reject_unbound_marker(tree: &ParsedNode) -> ParseResult<()> {
    if let Some(span) = state_marker(tree) {
        Err(ParseError::new(
            span,
            "html!: primal::state requires component state",
        ))
    } else {
        Ok(())
    }
}

fn form_error() -> ParseError {
    ParseError::new(
        proc_macro2::Span::call_site(),
        "html!: expected `ctx, receiver, <template>` or `ctx, receiver, Some(&state), <template>`",
    )
}

#[derive(Clone, Copy)]
pub(crate) enum Placement {
    Append,
    Before,
    After,
    Swap,
    Remove,
}

pub(crate) fn targeted(input: TokenStream, kind: Placement) -> TokenStream {
    output(placement(input, kind))
}

fn placement(input: TokenStream, kind: Placement) -> ParseResult<TokenStream> {
    let (fields, template) = fields(input)?;
    let runtime = foundation_ui_runtime_path();
    let (target, child, ctx, out, state) = match (&kind, fields.as_slice()) {
        (Placement::Before | Placement::After, [target, anchor, ctx, out, rest @ ..])
            if rest.len() <= 1 =>
        {
            let child = if bare_none(anchor) {
                quote!(None)
            } else {
                quote!((#anchor).map(#runtime::view::Target::from))
            };
            (target, child, ctx, out, rest.first())
        }
        (Placement::Remove, [target, ctx, out]) if template.is_empty() => {
            (target, quote!(None), ctx, out, None)
        }
        (Placement::Append | Placement::Swap, [target, ctx, out, rest @ ..]) if rest.len() <= 1 => {
            (target, quote!(None), ctx, out, rest.first())
        }
        _ => {
            return Err(ParseError::new(
                proc_macro2::Span::call_site(),
                "invalid arguments for targeted html macro",
            ))
        }
    };
    let tree = if matches!(kind, Placement::Remove) {
        quote!(None)
    } else {
        let tree = parser::parse_template(template)?;
        if state.is_none_or(bare_none) {
            reject_unbound_marker(&tree)?;
        }
        let tree = codegen::generate_pure(tree, true);
        quote!(Some(#tree))
    };
    let variant = match kind {
        Placement::Append => quote!(Append),
        Placement::Before => quote!(Before),
        Placement::After => quote!(After),
        Placement::Swap => quote!(Swap),
        Placement::Remove => quote!(Remove),
    };
    let state = state_expr(state);
    Ok(
        quote!(#runtime::view::place(&(#ctx), &(#out), #runtime::view::PlacementKind::#variant,
        #runtime::view::Target::from(#target), #child, #tree, #state)),
    )
}
