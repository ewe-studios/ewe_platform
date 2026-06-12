//! WHY: The parser gives a template tree; something must turn it into Rust
//! that builds the TYPED `foundation_ui_traits::Html` (decision 005) and — in
//! the reactive form — wires one effect per dynamic slot so signal changes
//! re-render exactly that slot (decisions 005/008/029).
//!
//! WHAT: Code generation for both invocation forms:
//!
//! - `html! { <div>..</div> }` — PURE: an `Html` expression. Slots evaluate
//!   once via `IntoHtml`; `Part` descriptors record where the dynamic bits
//!   are. No context, no effects (snapshots, SSR, tests).
//! - `html! { ctx, receiver, <div>..</div> }` — REACTIVE: also returns `Html`,
//!   and additionally allocates a runtime id block, queues the full
//!   `DomOp` build of the tree on `receiver`, and creates one effect per
//!   slot/dynamic-attr. Effects run immediately (decision 008), so the
//!   INITIAL content also arrives through them — each slot expression
//!   therefore lives in EXACTLY ONE closure (no double-move of captured
//!   handles; handles used in several slots need per-slot clones, plain
//!   Rust rules).
//!
//! HOW: Two numberings walk the tree together (feature 03 §4 + F01 registry
//! semantics):
//! - `element id` — depth-first counter over ELEMENTS only; this is the
//!   spec-visible `primal-id` / `Part::node_id` numbering (div=0, span=1, …).
//! - `runtime index` — counter over ALL nodes (elements, static text, slots);
//!   in reactive mode node N's wire id is `__base + N` where `__base` comes
//!   from `ctx.allocate_id_block(total)`, so every instance occupies a
//!   disjoint `u32` range (the spec's `"42:0"` string prefixes don't fit the
//!   `u32` `DomOp`/`NodeRegistry` ids — block allocation is the same idea in
//!   wire-compatible form).
//!
//! Event handlers go through `MaybeCallback` (G21): `SignalSetter` resolves
//! `Some(callback_id)` → a `primal:setter` attribute; closures resolve `None`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::parser::{ParseError, ParsedAttr, ParsedNode};
use super::scoped_css::transform_scoped_css;
use crate::crate_paths::{foundation_ui_traits_path, foundation_wasm_ui_path};

/// A compile-time-transformed scoped style block (feature 09, decision 019):
/// extracted from `<style primal:style>` children, combined per parent (§4),
/// `:parent` resolved against the parent's identity.
struct ScopedStyle {
    css: String,
}

/// Extract + transform `<style primal:style>` children (recursively). The
/// style elements are REMOVED from the tree; their CSS — which must be quoted
/// string content, since raw CSS does not tokenize as Rust — is transformed
/// NOW and returned for the mode-specific emission (pure: an inline scoped
/// <style> child; reactive: head-injection ops).
fn extract_scoped_styles(node: &mut ParsedNode) -> Result<Vec<ScopedStyle>, ParseError> {
    let mut styles = Vec::new();
    extract_styles_walk(node, &mut styles)?;
    Ok(styles)
}

fn extract_styles_walk(
    node: &mut ParsedNode,
    out: &mut Vec<ScopedStyle>,
) -> Result<(), ParseError> {
    let ParsedNode::Element {
        attrs,
        children,
        tag_span,
        ..
    } = node
    else {
        return Ok(());
    };
    let span = *tag_span;

    // Identity of THIS element, used when its style children scope to it.
    let identity = parent_identity(attrs);

    let mut kept = Vec::with_capacity(children.len());
    let mut combined = String::new();
    for child in children.drain(..) {
        if let ParsedNode::Element {
            tag,
            attrs: style_attrs,
            children: style_children,
            tag_span: style_span,
        } = &child
        {
            let is_scoped_style = tag == "style"
                && style_attrs.iter().any(|a| {
                    matches!(a, ParsedAttr::Static { name, .. } if name == "primal:style")
                });
            if is_scoped_style {
                // Guard only — the combined CSS uses the outer binding below.
                let Some(_identity) = &identity else {
                    return Err(ParseError {
                        span,
                        message: String::from(
                            "html!: scoped <style primal:style> requires the parent \
                             to have an id or class attribute",
                        ),
                    });
                };
                for piece in style_children {
                    match piece {
                        ParsedNode::Text(text) => combined.push_str(text),
                        // `{"css"}` slots: a single string literal, taken at
                        // compile time (raw CSS does not tokenize as Rust).
                        ParsedNode::Slot(tokens) => {
                            match syn::parse2::<syn::LitStr>(tokens.clone()) {
                                Ok(lit) => combined.push_str(&lit.value()),
                                Err(_) => {
                                    return Err(ParseError {
                                        span: *style_span,
                                        message: String::from(
                                            "html!: <style primal:style> content must be a \
                                             quoted string literal",
                                        ),
                                    })
                                }
                            }
                        }
                        ParsedNode::Element { .. } | ParsedNode::Fragment { .. } => {
                            return Err(ParseError {
                                span: *style_span,
                                message: String::from(
                                    "html!: <style primal:style> content must be a \
                                     quoted string literal",
                                ),
                            })
                        }
                    }
                    combined.push('\n');
                }
                continue; // style element removed from the tree
            }
        }
        kept.push(child);
    }
    if !combined.is_empty() {
        let identity = identity.as_deref().unwrap_or("#?");
        out.push(ScopedStyle {
            css: transform_scoped_css(&combined, identity),
        });
    }
    *children = kept;
    for child in children.iter_mut() {
        extract_styles_walk(child, out)?;
    }
    Ok(())
}

/// `#id`, else `.first-class`, else None (feature 09 §2 — id wins).
fn parent_identity(attrs: &[ParsedAttr]) -> Option<String> {
    for attr in attrs {
        if let ParsedAttr::Static { name, value } = attr {
            if name == "id" {
                return Some(format!("#{value}"));
            }
        }
    }
    for attr in attrs {
        if let ParsedAttr::Static { name, value } = attr {
            if name == "class" {
                let first = value.split_whitespace().next()?;
                return Some(format!(".{first}"));
            }
        }
    }
    None
}

/// What the expansion needs to know about one node, after numbering.
struct Numbered {
    node: ParsedNode,
    /// Element-only spec numbering (None for text/slots).
    element_id: Option<u32>,
    /// All-nodes numbering (reactive wire id = __base + this).
    runtime_index: u32,
    children: Vec<Numbered>,
}

/// One `Part` to record at the root, in document order.
enum PartSpec {
    Text { node_id: u32 },
    Attribute { node_id: u32, attr_name: String },
    Event { node_id: u32, event_name: String },
}

/// Numbering + collection state for one expansion.
#[derive(Default)]
struct Plan {
    next_element: u32,
    next_runtime: u32,
    parts: Vec<PartSpec>,
}

/// Number a tree and collect parts — slots need their PARENT's element id, so
/// the walk happens here rather than inside `Plan::number`.
fn build_plan(root: ParsedNode) -> (Plan, Numbered) {
    fn walk(plan: &mut Plan, node: ParsedNode, parent_element: Option<u32>) -> Numbered {
        // Fragments are directives, not nodes — they consume no ids.
        let runtime_index = plan.next_runtime;
        if !matches!(node, ParsedNode::Fragment { .. }) {
            plan.next_runtime += 1;
        }

        match node {
            ParsedNode::Element {
                tag,
                tag_span,
                attrs,
                children,
            } => {
                let element_id = plan.next_element;
                plan.next_element += 1;

                for attr in &attrs {
                    if let ParsedAttr::Dynamic { name, .. } = attr {
                        plan.parts.push(PartSpec::Attribute {
                            node_id: element_id,
                            attr_name: name.clone(),
                        });
                    }
                }
                for attr in &attrs {
                    if let ParsedAttr::Event { event_name, .. } = attr {
                        plan.parts.push(PartSpec::Event {
                            node_id: element_id,
                            event_name: event_name.clone(),
                        });
                    }
                }

                let children = children
                    .into_iter()
                    .map(|c| walk(plan, c, Some(element_id)))
                    .collect();
                Numbered {
                    node: ParsedNode::Element {
                        tag,
                        tag_span,
                        attrs,
                        children: Vec::new(),
                    },
                    element_id: Some(element_id),
                    runtime_index,
                    children,
                }
            }
            ParsedNode::Slot(tokens) => {
                plan.parts.push(PartSpec::Text {
                    node_id: parent_element.expect("slots always sit under an element root"),
                });
                Numbered {
                    node: ParsedNode::Slot(tokens),
                    element_id: None,
                    runtime_index,
                    children: Vec::new(),
                }
            }
            ParsedNode::Text(text) => Numbered {
                node: ParsedNode::Text(text),
                element_id: None,
                runtime_index,
                children: Vec::new(),
            },
            ParsedNode::Fragment { exprs } => Numbered {
                node: ParsedNode::Fragment { exprs },
                element_id: None,
                runtime_index,
                children: Vec::new(),
            },
        }
    }

    let mut plan = Plan::default();
    let numbered = walk(&mut plan, root, None);
    (plan, numbered)
}

/// Generate the PURE form: one `Html` expression.
pub(crate) fn generate_pure(mut root: ParsedNode) -> TokenStream {
    let styles = match extract_scoped_styles(&mut root) {
        Ok(styles) => styles,
        Err(err) => {
            let message = err.message;
            let span = err.span;
            return quote::quote_spanned! {span=> compile_error!(#message) };
        }
    };
    let (plan, numbered) = build_plan(root);
    let ui = foundation_ui_traits_path();

    let mut handler_index = 0usize;
    let mut prelude = TokenStream::new();
    collect_handler_bindings(&numbered, &mut prelude, &mut handler_index);

    let mut handler_cursor = 0usize;
    let html = gen_html_node(&numbered, &ui, Mode::Pure, &mut handler_cursor);
    let parts = gen_parts(&plan.parts, &ui);

    let style_children = styles.iter().map(|style| {
        let css = &style.css;
        quote! {
            __html.children.push(#ui::Html {
                runtime_id: ::core::option::Option::None,
                tag: ::core::option::Option::Some(#ui::HtmlTag::from_static("style")),
                attributes: #ui::__macro::vec![(
                    #ui::AttrName::from_static("data-primal-scoped"),
                    #ui::__macro::Cow::Borrowed("true"),
                )],
                children: #ui::__macro::vec![#ui::Html::text(#css)],
                text: ::core::option::Option::None,
                parts: #ui::__macro::Vec::new(),
            });
        }
    });

    quote! {{
        #prelude
        let mut __html = #html;
        __html.parts = #parts;
        #(#style_children)*
        __html
    }}
}

/// Generate the REACTIVE form: build `Html`, queue the DOM build, wire effects.
pub(crate) fn generate_reactive(
    ctx: &TokenStream,
    receiver: &TokenStream,
    mut root: ParsedNode,
) -> TokenStream {
    let styles = match extract_scoped_styles(&mut root) {
        Ok(styles) => styles,
        Err(err) => {
            let message = err.message;
            let span = err.span;
            return quote::quote_spanned! {span=> compile_error!(#message) };
        }
    };
    let (plan, numbered) = build_plan(root);
    let ui = foundation_ui_traits_path();
    // Scoped styles take ids past the node block (feature 09 §8.3 — they ship
    // to <head>, reserved ambient id 0, as part of the same batch).
    let style_count = u32::try_from(styles.len()).expect("style count");
    let total = plan.next_runtime + style_count;

    let mut handler_index = 0usize;
    let mut prelude = TokenStream::new();
    collect_handler_bindings(&numbered, &mut prelude, &mut handler_index);

    let mut handler_cursor = 0usize;
    let html = gen_html_node(&numbered, &ui, Mode::Reactive, &mut handler_cursor);
    let parts = gen_parts(&plan.parts, &ui);

    let mut ops = TokenStream::new();
    let mut effects = TokenStream::new();
    let mut handler_cursor = 0usize;
    gen_mount_ops(
        &numbered,
        None,
        &ui,
        &mut ops,
        &mut effects,
        &mut handler_cursor,
    );

    let node_total = plan.next_runtime;
    let style_ops = styles.iter().enumerate().map(|(i, style)| {
        let css = &style.css;
        let idx = node_total + u32::try_from(i).expect("style index");
        quote! {
            __rcv.queue(#ui::DomOp::CreateElement {
                node_id: __base + #idx,
                tag: #ui::HtmlTag::from_static("style"),
                class: #ui::__macro::Cow::Borrowed(""),
            });
            __rcv.queue(#ui::DomOp::RegisterNode { node_id: __base + #idx });
            __rcv.queue(#ui::DomOp::SetText {
                node_id: __base + #idx,
                text: #ui::__macro::Cow::Borrowed(#css),
            });
            // Reserved ambient id 0 = <head> (JS NodeRegistry seed).
            __rcv.queue(#ui::DomOp::AppendChild {
                parent_id: 0,
                child_id: __base + #idx,
            });
        }
    });

    quote! {{
        let __ctx = &(#ctx);
        let __rcv = (#receiver).clone();
        let __base: u32 = __ctx.allocate_id_block(#total);
        #prelude
        let mut __html = #html;
        __html.parts = #parts;
        // The root's registered wire id — how mount_fragment recognizes an
        // already-mounted fragment (spec-42 feature 00).
        __html.runtime_id = ::core::option::Option::Some(__base);
        #ops
        #(#style_ops)*
        #effects
        __html
    }}
}

#[derive(Clone, Copy)]
enum Mode {
    Pure,
    Reactive,
}

/// Hoist every event handler ONCE: `let __hN = {tokens}; let __cbN = MaybeCallback...`.
/// The `MaybeCallback` trait lives in `foundation_wasm_ui` (G21) — its path is
/// only emitted when the template actually has handlers.
fn collect_handler_bindings(node: &Numbered, out: &mut TokenStream, index: &mut usize) {
    if let ParsedNode::Element { attrs, .. } = &node.node {
        for attr in attrs {
            if let ParsedAttr::Event { tokens, .. } = attr {
                let h = format_ident!("__h{index}");
                let cb = format_ident!("__cb{index}");
                let wui = foundation_wasm_ui_path();
                out.extend(quote! {
                    let #h = { #tokens };
                    let #cb: ::core::option::Option<u64> =
                        #wui::MaybeCallback::maybe_callback_id(&#h);
                });
                *index += 1;
            }
        }
    }
    for child in &node.children {
        collect_handler_bindings(child, out, index);
    }
}

/// The `Html` literal for one node.
// One arm per node kind × mode — splitting would obscure the generated-tree
// layout this function exists to define (same justification as gen_mount_ops).
#[allow(clippy::too_many_lines)]
fn gen_html_node(
    node: &Numbered,
    ui: &TokenStream,
    mode: Mode,
    handler_cursor: &mut usize,
) -> TokenStream {
    match &node.node {
        ParsedNode::Element { tag, attrs, .. } => {
            let element_id = node.element_id.expect("element");

            // primal-id: compile id (pure) or runtime block id (reactive).
            let pid_value = match mode {
                Mode::Pure => {
                    let s = element_id.to_string();
                    quote! { #ui::__macro::Cow::Borrowed(#s) }
                }
                Mode::Reactive => {
                    let ridx = node.runtime_index;
                    quote! { #ui::__macro::Cow::Owned(
                        #ui::__macro::ToString::to_string(&(__base + #ridx))
                    ) }
                }
            };

            let mut attr_stmts = vec![quote! {
                __attrs.push((
                    #ui::AttrName::from_static("primal-id"),
                    #pid_value,
                ));
            }];

            for attr in attrs {
                match attr {
                    ParsedAttr::Static { name, value } => {
                        attr_stmts.push(quote! {
                            __attrs.push((
                                #ui::AttrName::from_static(#name),
                                #ui::__macro::Cow::Borrowed(#value),
                            ));
                        });
                    }
                    ParsedAttr::Dynamic { name, tokens } => match mode {
                        // Pure: evaluate once, text via IntoHtml.
                        Mode::Pure => attr_stmts.push(quote! {
                            __attrs.push((
                                #ui::AttrName::from_static(#name),
                                #ui::__macro::Cow::Owned(
                                    #ui::IntoHtml::into_html((#tokens))
                                        .text
                                        .map(|__c| __c.into_owned())
                                        .unwrap_or_default(),
                                ),
                            ));
                        }),
                        // Reactive: the effect (which runs immediately) owns the
                        // expression; the tree carries a placeholder.
                        Mode::Reactive => attr_stmts.push(quote! {
                            __attrs.push((
                                #ui::AttrName::from_static(#name),
                                #ui::__macro::Cow::Borrowed(""),
                            ));
                        }),
                    },
                    ParsedAttr::Event { event_name, .. } => {
                        let marker = format!("primal:on{event_name}");
                        let cb = format_ident!("__cb{handler_cursor}");
                        *handler_cursor += 1;
                        attr_stmts.push(quote! {
                            __attrs.push((
                                #ui::AttrName::from_static(#marker),
                                #ui::__macro::Cow::Borrowed("true"),
                            ));
                            if let ::core::option::Option::Some(__id) = #cb {
                                __attrs.push((
                                    #ui::AttrName::from_static("primal:setter"),
                                    #ui::__macro::Cow::Owned(
                                        #ui::__macro::ToString::to_string(&__id)
                                    ),
                                ));
                            }
                        });
                    }
                }
            }

            let children: Vec<TokenStream> = node
                .children
                .iter()
                .map(|c| gen_html_node(c, ui, mode, handler_cursor))
                .collect();

            quote! {{
                let mut __attrs: #ui::__macro::Vec<(
                    #ui::AttrName,
                    #ui::__macro::Cow<'static, str>,
                )> = #ui::__macro::Vec::new();
                #(#attr_stmts)*
                #ui::Html {
                    tag: ::core::option::Option::Some(#ui::HtmlTag::from_static(#tag)),
                    attributes: __attrs,
                    children: #ui::__macro::vec![#(#children),*],
                    text: ::core::option::Option::None,
                    parts: #ui::__macro::Vec::new(),
                    runtime_id: ::core::option::Option::None,
                }
            }}
        }
        ParsedNode::Text(text) => quote! { #ui::Html::text(#text) },
        ParsedNode::Slot(tokens) => match mode {
            // Pure: text-shaped values wrap in the slot <span> (the morph
            // contract — spec-42 feature 00 §4: BOTH forms render text slots
            // as spans); element/fragment values inline as before.
            Mode::Pure => quote! {{
                let __v = #ui::IntoHtml::into_html((#tokens));
                if __v.tag.is_none() && __v.children.is_empty() {
                    #ui::Html {
                        tag: ::core::option::Option::Some(#ui::HtmlTag::from_static("span")),
                        attributes: #ui::__macro::Vec::new(),
                        children: #ui::__macro::vec![__v],
                        text: ::core::option::Option::None,
                        parts: #ui::__macro::Vec::new(),
                        runtime_id: ::core::option::Option::None,
                    }
                } else {
                    __v
                }
            }},
            // Reactive: span placeholder — the slot effect renders into it.
            Mode::Reactive => quote! { #ui::Html {
                tag: ::core::option::Option::Some(#ui::HtmlTag::from_static("span")),
                attributes: #ui::__macro::Vec::new(),
                children: #ui::__macro::Vec::new(),
                text: ::core::option::Option::None,
                parts: #ui::__macro::Vec::new(),
                runtime_id: ::core::option::Option::None,
            } },
        },
        ParsedNode::Fragment { exprs } => match mode {
            // Pure: inline — a tagless wrapper, same shape Vec<Html> slots
            // produce (to_markup flattens it transparently).
            Mode::Pure => quote! { #ui::Html {
                tag: ::core::option::Option::None,
                attributes: #ui::__macro::Vec::new(),
                children: #ui::__macro::vec![
                    #(#ui::IntoHtml::into_html((#exprs))),*
                ],
                text: ::core::option::Option::None,
                parts: #ui::__macro::Vec::new(),
                runtime_id: ::core::option::Option::None,
            } },
            // Reactive: the content mounts through ops; the VALUE side is an
            // empty grouping node.
            Mode::Reactive => quote! { #ui::Html::new() },
        },
    }
}

/// The root `parts` vector (compile-time element ids, spec numbering).
fn gen_parts(parts: &[PartSpec], ui: &TokenStream) -> TokenStream {
    let entries = parts.iter().map(|part| match part {
        PartSpec::Text { node_id } => quote! {
            #ui::Part::Text(#ui::TextPart { node_id: #node_id })
        },
        PartSpec::Attribute { node_id, attr_name } => quote! {
            #ui::Part::Attribute(#ui::AttrPart {
                node_id: #node_id,
                attr_name: #ui::__macro::String::from(#attr_name),
            })
        },
        PartSpec::Event {
            node_id,
            event_name,
        } => quote! {
            #ui::Part::Event(#ui::EventPart {
                node_id: #node_id,
                event_name: #ui::__macro::String::from(#event_name),
            })
        },
    });
    quote! { #ui::__macro::vec![#(#entries),*] }
}

/// Reactive mode: queue the DOM build ops (document order, F01 registry
/// semantics — explicit `RegisterNode`, `AppendChild` after both ends exist) and
/// one effect per slot / dynamic attribute.
// One arm per node kind, ops in document order — splitting would obscure the
// generated-stream layout this function exists to define.
#[allow(clippy::too_many_lines)]
fn gen_mount_ops(
    node: &Numbered,
    parent_runtime: Option<u32>,
    ui: &TokenStream,
    ops: &mut TokenStream,
    effects: &mut TokenStream,
    handler_cursor: &mut usize,
) {
    let ridx = node.runtime_index;
    match &node.node {
        ParsedNode::Element { tag, attrs, .. } => {
            // Class rides CreateElement (op 0's value column).
            let class_value = attrs.iter().find_map(|a| match a {
                ParsedAttr::Static { name, value } if name == "class" => Some(value.clone()),
                _ => None,
            });
            let class = class_value.unwrap_or_default();

            ops.extend(quote! {
                __rcv.queue(#ui::DomOp::CreateElement {
                    node_id: __base + #ridx,
                    tag: #ui::HtmlTag::from_static(#tag),
                    class: #ui::__macro::Cow::Borrowed(#class),
                });
                __rcv.queue(#ui::DomOp::RegisterNode { node_id: __base + #ridx });
                __rcv.queue(#ui::DomOp::SetAttribute {
                    node_id: __base + #ridx,
                    name: #ui::AttrName::from_static("primal-id"),
                    value: #ui::__macro::Cow::Owned(
                        #ui::__macro::ToString::to_string(&(__base + #ridx))
                    ),
                });
            });

            for attr in attrs {
                match attr {
                    ParsedAttr::Static { name, value } if name != "class" => {
                        ops.extend(quote! {
                            __rcv.queue(#ui::DomOp::SetAttribute {
                                node_id: __base + #ridx,
                                name: #ui::AttrName::from_static(#name),
                                value: #ui::__macro::Cow::Borrowed(#value),
                            });
                        });
                    }
                    ParsedAttr::Static { .. } => {}
                    ParsedAttr::Dynamic { name, tokens } => {
                        // The effect owns the expression; immediate run queues
                        // the initial SetAttribute (decision 008).
                        effects.extend(quote! {{
                            let __r = __rcv.clone();
                            __ctx.effect(move || {
                                let __value = #ui::IntoHtml::into_html((#tokens))
                                    .text
                                    .map(|__c| __c.into_owned())
                                    .unwrap_or_default();
                                __r.queue(#ui::DomOp::SetAttribute {
                                    node_id: __base + #ridx,
                                    name: #ui::AttrName::from_static(#name),
                                    value: #ui::__macro::Cow::Owned(__value),
                                });
                            });
                        }});
                    }
                    ParsedAttr::Event { event_name, .. } => {
                        let cb = format_ident!("__cb{handler_cursor}");
                        *handler_cursor += 1;
                        ops.extend(quote! {
                            __rcv.queue(#ui::DomOp::AddEventListener {
                                node_id: __base + #ridx,
                                event_name: #ui::AttrName::from_static(#event_name),
                            });
                            if let ::core::option::Option::Some(__id) = #cb {
                                __rcv.queue(#ui::DomOp::SetAttribute {
                                    node_id: __base + #ridx,
                                    name: #ui::AttrName::from_static("primal:setter"),
                                    value: #ui::__macro::Cow::Owned(
                                        #ui::__macro::ToString::to_string(&__id)
                                    ),
                                });
                            }
                        });
                    }
                }
            }

            for child in &node.children {
                gen_mount_ops(child, Some(ridx), ui, ops, effects, handler_cursor);
            }

            if let Some(parent) = parent_runtime {
                ops.extend(quote! {
                    __rcv.queue(#ui::DomOp::AppendChild {
                        parent_id: __base + #parent,
                        child_id: __base + #ridx,
                    });
                });
            }
        }
        ParsedNode::Text(text) => {
            let parent = parent_runtime.expect("text under root element");
            ops.extend(quote! {
                __rcv.queue(#ui::DomOp::CreateTextNode {
                    node_id: __base + #ridx,
                    content: #ui::__macro::Cow::Borrowed(#text),
                });
                __rcv.queue(#ui::DomOp::RegisterNode { node_id: __base + #ridx });
                __rcv.queue(#ui::DomOp::AppendChild {
                    parent_id: __base + #parent,
                    child_id: __base + #ridx,
                });
            });
        }
        ParsedNode::Slot(tokens) => {
            let parent = parent_runtime.expect("slot under root element");
            // Dedicated <span primal-id> per slot (spec-42 feature 00 §4):
            // SetText targets IT, so siblings are never clobbered, and —
            // unlike a bare text node — the span carries an id, so MorphDom
            // preserves the slot's wiring across server patches.
            ops.extend(quote! {
                __rcv.queue(#ui::DomOp::CreateElement {
                    node_id: __base + #ridx,
                    tag: #ui::HtmlTag::from_static("span"),
                    class: #ui::__macro::Cow::Borrowed(""),
                });
                __rcv.queue(#ui::DomOp::RegisterNode { node_id: __base + #ridx });
                __rcv.queue(#ui::DomOp::SetAttribute {
                    node_id: __base + #ridx,
                    name: #ui::AttrName::from_static("primal-id"),
                    value: #ui::__macro::Cow::Owned(
                        #ui::__macro::ToString::to_string(&(__base + #ridx))
                    ),
                });
                __rcv.queue(#ui::DomOp::AppendChild {
                    parent_id: __base + #parent,
                    child_id: __base + #ridx,
                });
            });
            effects.extend(quote! {{
                let __r = __rcv.clone();
                __ctx.effect(move || {
                    let __html = #ui::IntoHtml::into_html((#tokens));
                    let __text = __html
                        .text
                        .map(|__c| __c.into_owned())
                        .unwrap_or_default();
                    __r.queue(#ui::DomOp::SetText {
                        node_id: __base + #ridx,
                        text: #ui::__macro::Cow::Owned(__text),
                    });
                });
            }});
        }
        ParsedNode::Fragment { exprs } => {
            let parent = parent_runtime.expect("Fragment under root element");
            // Once at mount, never in an effect: already-mounted fragments
            // (runtime_id set) splice by AppendChild; pure fragments are
            // built with a fresh id block (mount_fragment, feature 00 §6).
            let wui = foundation_wasm_ui_path();
            for tokens in exprs {
                ops.extend(quote! {{
                    let __fragment = #ui::IntoHtml::into_html((#tokens));
                    let _ = #wui::mount_fragment(
                        __ctx,
                        &__rcv,
                        __fragment,
                        __base + #parent,
                    );
                }});
            }
        }
    }
}
