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
//! `DomOp` build + effects. See each submodule's docs for the details.

mod codegen;
mod parser;

use proc_macro2::{TokenStream, TokenTree};

/// Expand `html!`. See the module docs for both forms.
pub(crate) fn html(input: TokenStream) -> TokenStream {
    // Form detection: a template starts with '<'; the reactive form starts
    // with `ctx_expr, receiver_expr,` (arbitrary expressions up to top-level
    // commas) before the first '<'.
    let starts_with_tag = matches!(
        input.clone().into_iter().next(),
        Some(TokenTree::Punct(p)) if p.as_char() == '<'
    );

    let result = if starts_with_tag {
        parser::parse_template(input).map(codegen::generate_pure)
    } else {
        match split_reactive_header(input) {
            Ok((ctx, receiver, template)) => parser::parse_template(template)
                .map(|root| codegen::generate_reactive(&ctx, &receiver, root)),
            Err(err) => Err(err),
        }
    };

    match result {
        Ok(tokens) => tokens,
        Err(err) => {
            let message = err.message;
            let span = err.span;
            quote::quote_spanned! {span=> compile_error!(#message) }
        }
    }
}

/// Split `ctx_expr, receiver_expr, <template…>` at the two top-level commas.
fn split_reactive_header(
    input: TokenStream,
) -> Result<(TokenStream, TokenStream, TokenStream), parser::ParseError> {
    let mut sections: Vec<TokenStream> = vec![TokenStream::new()];
    let mut iter = input.into_iter();
    for token in iter.by_ref() {
        match &token {
            TokenTree::Punct(p) if p.as_char() == ',' && sections.len() < 3 => {
                sections.push(TokenStream::new());
                // After the second comma the rest is the template.
                if sections.len() == 3 {
                    break;
                }
            }
            _ => sections
                .last_mut()
                .expect("sections never empty")
                .extend([token]),
        }
    }
    let template: TokenStream = iter.collect();

    if sections.len() != 3 || sections[0].is_empty() || sections[1].is_empty() {
        return Err(parser::ParseError {
            span: proc_macro2::Span::call_site(),
            message: String::from(
                "html!: expected `html! { <template…> }` or `html! { ctx, receiver, <template…> }`",
            ),
        });
    }
    let ctx = sections.remove(0);
    let receiver = sections.remove(0);
    Ok((ctx, receiver, template))
}
