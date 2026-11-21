//! WHY: `html!` must understand HTML-ish syntax at COMPILE time with zero
//! runtime parsing (decision 001). Rust's tokenizer already split the input —
//! this parser only walks `TokenTree`s; it never sees raw characters.
//!
//! WHAT: The token walker producing [`ParsedNode`]/[`ParsedAttr`] trees, void
//! element handling, and every compile-error message from the feature-03 spec
//! table (with spans, so IDEs underline the right token).
//!
//! HOW: A peekable cursor over `proc_macro2::TokenTree`. `<` opens a tag, a
//! brace group is a dynamic slot captured VERBATIM (the expression is never
//! interpreted here — codegen re-emits it), everything else accumulates into
//! text with punct-aware spacing.

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};

/// Tags that never take children or a closing tag (feature 03 §3.4).
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub(crate) fn is_void_element(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

/// A parse failure: the offending span plus the spec-table message.
#[derive(Debug)]
pub(crate) struct ParseError {
    pub span: Span,
    pub message: String,
}

impl ParseError {
    pub(super) fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

pub(crate) type ParseResult<T> = Result<T, ParseError>;

/// One parsed template node.
pub(crate) enum ParsedNode {
    Element {
        tag: String,
        tag_span: Span,
        attrs: Vec<ParsedAttr>,
        children: Vec<ParsedNode>,
    },
    /// Selects one child sequence when Rust evaluates the template.
    Either {
        condition: TokenStream,
        left: Vec<ParsedNode>,
        right: Vec<ParsedNode>,
    },
    /// Holds a generated expression for the selected children after numbering.
    BranchValue(TokenStream),
    /// Literal text content (quoted strings AND bare token runs).
    Text(String),
    /// A `{ ... }` slot — the expression tokens, untouched.
    Slot(TokenStream),
    /// `<Fragment>{expr}…</Fragment>` (spec-42 feature 00 §6): a compile-time
    /// directive, no DOM node of its own. Each expression evaluates ONCE at
    /// mount and splices into the PARENT element (pure form: inline, like
    /// today's `{expr}`; reactive form: `mount_fragment`).
    Fragment { exprs: Vec<TokenStream> },
    /// `<Show when={expr}>{render}</Show>` (spec-42 feature 01): mounts the
    /// content (an `impl Render`) while the condition holds. Reactive-only.
    Show {
        span: Span,
        when: TokenStream,
        content: TokenStream,
    },
    /// `<For each={expr} key={fn} render={fn} />` (spec-42 feature 01): a
    /// keyed reactive list. Reactive-only.
    For {
        span: Span,
        each: TokenStream,
        key: TokenStream,
        render: TokenStream,
    },
}

pub(super) fn core_tag(tag: &str) -> bool {
    matches!(
        tag,
        "Stack"
            | "Text"
            | "Image"
            | "Button"
            | "TextInput"
            | "Toggle"
            | "List"
            | "Scroll"
            | "Spacer"
            | "Overlay"
            | "Modal"
            | "BottomSheet"
    )
}

pub(super) enum ActionTarget {
    Rust(syn::Path),
    Host(String),
}

/// One parsed attribute.
pub(crate) enum ParsedAttr {
    Action {
        event: String,
        target: ActionTarget,
    },
    StateMarker(Span),
    /// `name="value"` or bare boolean `name` (value "true").
    Static {
        name: String,
        value: String,
    },
    /// `name={expr}` — REACTIVE: re-evaluated in an effect on signal change.
    Dynamic {
        name: String,
        tokens: TokenStream,
    },
    /// `name=[expr]` — STATIC-once: evaluated a single time at build (no
    /// effect). For runtime-but-non-reactive values (config `Cow`s, computed
    /// classes) — avoids the `FnMut` move that forces `{expr}` callers to
    /// `.clone()`, and emits no per-render effect.
    StaticDynamic {
        name: String,
        tokens: TokenStream,
    },
    /// `primal:onX={handler}` — `event_name` is X with the prefix stripped.
    Event {
        event_name: String,
        tokens: TokenStream,
    },
}

/// Peekable cursor over a token stream.
pub(crate) struct Cursor {
    tokens: Vec<TokenTree>,
    at: usize,
    /// Span of the last consumed token — error anchor at end-of-input.
    last_span: Span,
}

impl Cursor {
    pub(crate) fn new(stream: TokenStream) -> Self {
        Self {
            tokens: stream.into_iter().collect(),
            at: 0,
            last_span: Span::call_site(),
        }
    }

    pub(crate) fn peek(&self) -> Option<&TokenTree> {
        self.tokens.get(self.at)
    }

    fn peek2(&self) -> Option<&TokenTree> {
        self.tokens.get(self.at + 1)
    }

    pub(crate) fn next(&mut self) -> Option<TokenTree> {
        let token = self.tokens.get(self.at).cloned();
        if let Some(token) = &token {
            self.last_span = token.span();
            self.at += 1;
        }
        token
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.at >= self.tokens.len()
    }

    fn span(&self) -> Span {
        self.peek().map_or(self.last_span, TokenTree::span)
    }

    fn is_punct(&self, ch: char) -> bool {
        matches!(self.peek(), Some(TokenTree::Punct(p)) if p.as_char() == ch)
    }

    fn eat_punct(&mut self, ch: char) -> bool {
        if self.is_punct(ch) {
            self.next();
            true
        } else {
            false
        }
    }

    /// `</` lookahead (without consuming).
    fn at_closing_tag(&self) -> bool {
        self.is_punct('<')
            && matches!(self.peek2(), Some(TokenTree::Punct(p)) if p.as_char() == '/')
    }
}

/// Parse a full template: exactly one root element.
pub(crate) fn parse_template(stream: TokenStream) -> ParseResult<ParsedNode> {
    let mut cursor = Cursor::new(stream);
    if cursor.is_empty() {
        return Err(ParseError::new(Span::call_site(), "html!: empty input"));
    }
    let nodes = parse_nodes(&mut cursor)?;
    if !cursor.is_empty() {
        return Err(ParseError::new(
            cursor.span(),
            "html!: multiple root elements — wrap in a parent",
        ));
    }
    let mut roots = nodes.into_iter();
    let Some(root) = roots.next() else {
        return Err(ParseError::new(Span::call_site(), "html!: empty input"));
    };
    if roots.next().is_some() {
        return Err(ParseError::new(
            Span::call_site(),
            "html!: multiple root elements — wrap in a parent",
        ));
    }
    Ok(root)
}

/// Parse sibling nodes until end-of-stream or a closing tag (`</`), which the
/// CALLER consumes.
fn parse_nodes(cursor: &mut Cursor) -> ParseResult<Vec<ParsedNode>> {
    let mut nodes = Vec::new();
    let mut text = TextRun::new();

    loop {
        if cursor.is_empty() || cursor.at_closing_tag() {
            text.flush(&mut nodes);
            return Ok(nodes);
        }
        match cursor.peek() {
            Some(TokenTree::Punct(p)) if p.as_char() == '<' => {
                text.flush(&mut nodes);
                nodes.push(parse_element(cursor)?);
            }
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                text.flush(&mut nodes);
                let TokenTree::Group(group) = cursor.next().expect("peeked") else {
                    unreachable!()
                };
                nodes.push(ParsedNode::Slot(group.stream()));
            }
            Some(TokenTree::Literal(_)) => {
                let TokenTree::Literal(lit) = cursor.next().expect("peeked") else {
                    unreachable!()
                };
                // A quoted string contributes its CONTENT; other literals
                // (numbers) their token text.
                match syn::parse2::<syn::LitStr>(quote::quote!(#lit)) {
                    Ok(s) => text.push_word(&s.value()),
                    Err(_) => text.push_word(&lit.to_string()),
                }
            }
            Some(_) => {
                // Bare text run: idents and puncts up to the next structural
                // token, joined with punct-aware spacing ("Count" ':' -> "Count:").
                let token = cursor.next().expect("peeked");
                match &token {
                    TokenTree::Punct(p) => text.push_punct(p.as_char()),
                    other => text.push_word(&other.to_string()),
                }
            }
            None => unreachable!("guarded by is_empty"),
        }
    }
}

/// Accumulates a bare-text run between structural tokens.
struct TextRun {
    buf: String,
    /// Suppress the space before the next word (set after a punct).
    glue_next: bool,
}

impl TextRun {
    fn new() -> Self {
        Self {
            buf: String::new(),
            glue_next: false,
        }
    }

    fn push_word(&mut self, word: &str) {
        if !self.buf.is_empty() && !self.glue_next {
            self.buf.push(' ');
        }
        self.buf.push_str(word);
        self.glue_next = false;
    }

    fn push_punct(&mut self, ch: char) {
        self.buf.push(ch);
        self.glue_next = true;
    }

    fn flush(&mut self, nodes: &mut Vec<ParsedNode>) {
        if !self.buf.is_empty() {
            nodes.push(ParsedNode::Text(std::mem::take(&mut self.buf)));
            self.glue_next = false;
        }
    }
}

/// Parse one element starting at its `<`.
fn parse_element(cursor: &mut Cursor) -> ParseResult<ParsedNode> {
    let open_span = cursor.span();
    assert!(cursor.eat_punct('<'), "caller guaranteed '<'");

    let (tag, tag_span) = parse_tag_name(cursor, open_span)?;

    // Capitalized tags are RESERVED compile-time built-ins (spec-42
    // feature 00 §6) — never DOM elements (real tags are lowercase; custom
    // elements require a dash).
    if !core_tag(&tag) && tag.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        if tag == "Either" {
            return parse_either(cursor, tag_span);
        }
        if tag == "Left" || tag == "Right" {
            return Err(ParseError::new(
                tag_span,
                format!("html!: <{tag}> requires an <Either> parent"),
            ));
        }
        if tag == "Fragment" {
            return parse_fragment(cursor, tag_span);
        }
        if tag == "Show" {
            return parse_show(cursor, tag_span);
        }
        if tag == "For" {
            return parse_for(cursor, tag_span);
        }
        return Err(ParseError::new(
            tag_span,
            format!(
                "html!: unknown built-in '<{tag}>' — capitalized tags are reserved \
                 for shared elements, Either, Fragment, Show, and For; custom elements need a dash (e.g. <my-{}>)",
                tag.to_lowercase()
            ),
        ));
    }

    let attrs = parse_attributes(cursor, &tag, tag_span)?;

    // `/>` or `>`.
    let self_closing = if cursor.eat_punct('/') {
        if !cursor.eat_punct('>') {
            return Err(ParseError::new(
                cursor.span(),
                format!("html!: missing '>' after attributes for '<{tag}>'"),
            ));
        }
        true
    } else if cursor.eat_punct('>') {
        false
    } else {
        return Err(ParseError::new(
            cursor.span(),
            format!("html!: missing '>' after attributes for '<{tag}>'"),
        ));
    };

    if self_closing || is_void_element(&tag) {
        return Ok(ParsedNode::Element {
            tag,
            tag_span,
            attrs,
            children: Vec::new(),
        });
    }

    let children = parse_nodes(cursor)?;
    consume_closing_tag(cursor, &tag, tag_span)?;
    Ok(ParsedNode::Element {
        tag,
        tag_span,
        attrs,
        children,
    })
}

fn parse_either(cursor: &mut Cursor, span: Span) -> ParseResult<ParsedNode> {
    let mut attrs = parse_builtin_attrs(cursor, "Either", span, &["condition"])?;
    let condition = take_builtin_attr(&mut attrs, "condition", "Either", span)?;
    if !attrs.is_empty() {
        return Err(ParseError::new(span, "html!: <Either> takes one condition"));
    }
    if !cursor.eat_punct('>') {
        return Err(ParseError::new(
            span,
            "html!: <Either> requires <Left> and <Right> branches",
        ));
    }

    let mut left = None;
    let mut right = None;
    while !cursor.is_empty() && !cursor.at_closing_tag() {
        let branch_span = cursor.span();
        if !cursor.eat_punct('<') {
            return Err(ParseError::new(
                branch_span,
                "html!: expected <Left> or <Right> in <Either>",
            ));
        }
        let (name, name_span) = parse_tag_name(cursor, branch_span)?;
        let branch = match name.as_str() {
            "Left" => &mut left,
            "Right" => &mut right,
            _ => {
                return Err(ParseError::new(
                    name_span,
                    "html!: expected <Left> or <Right> in <Either>",
                ))
            }
        };
        if branch.is_some() {
            return Err(ParseError::new(
                name_span,
                format!("html!: duplicate <{name}> branch"),
            ));
        }
        let empty = cursor.eat_punct('/');
        if !cursor.eat_punct('>') {
            return Err(ParseError::new(
                name_span,
                format!("html!: <{name}> takes no attributes"),
            ));
        }
        *branch = Some(if empty {
            Vec::new()
        } else {
            let nodes = parse_nodes(cursor)?;
            consume_closing_tag(cursor, &name, name_span)?;
            nodes
        });
    }
    consume_closing_tag(cursor, "Either", span)?;
    Ok(ParsedNode::Either {
        condition,
        left: left
            .ok_or_else(|| ParseError::new(span, "html!: <Either> requires a <Left> branch"))?,
        right: right
            .ok_or_else(|| ParseError::new(span, "html!: <Either> requires a <Right> branch"))?,
    })
}

/// Parse `<Fragment> {expr}… </Fragment>` after its tag name. Fragments take
/// no attributes; their children must be `{ }` expressions (static content
/// belongs directly in the parent — the directive only marks once-at-mount
/// splice points).
fn parse_fragment(cursor: &mut Cursor, tag_span: Span) -> ParseResult<ParsedNode> {
    // `<Fragment/>` (pointless but harmless) or `<Fragment>`.
    if cursor.eat_punct('/') {
        if !cursor.eat_punct('>') {
            return Err(ParseError::new(
                cursor.span(),
                "html!: missing '>' after '<Fragment/'",
            ));
        }
        return Ok(ParsedNode::Fragment { exprs: Vec::new() });
    }
    if !cursor.eat_punct('>') {
        return Err(ParseError::new(
            tag_span,
            "html!: <Fragment> takes no attributes — expected '>'",
        ));
    }

    let mut exprs = Vec::new();
    loop {
        if cursor.at_closing_tag() {
            break;
        }
        match cursor.peek() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                let Some(TokenTree::Group(group)) = cursor.next() else {
                    unreachable!("peeked")
                };
                exprs.push(group.stream());
            }
            Some(other) => {
                return Err(ParseError::new(
                    other.span(),
                    "html!: <Fragment> children must be { } expressions \
                     (static content goes directly in the parent element)",
                ));
            }
            None => {
                return Err(ParseError::new(tag_span, "html!: missing '</Fragment>'"));
            }
        }
    }
    consume_closing_tag(cursor, "Fragment", tag_span)?;
    Ok(ParsedNode::Fragment { exprs })
}

/// Collect the dynamic (`name={expr}`) attributes of a built-in, rejecting
/// everything else.
fn parse_builtin_attrs(
    cursor: &mut Cursor,
    tag: &str,
    tag_span: Span,
    allowed: &[&str],
) -> ParseResult<Vec<(String, TokenStream)>> {
    let attrs = parse_attributes(cursor, tag, tag_span)?;
    let mut out = Vec::new();
    for attr in attrs {
        match attr {
            ParsedAttr::Dynamic { name, tokens } if allowed.contains(&name.as_str()) => {
                out.push((name, tokens));
            }
            ParsedAttr::StateMarker(span) => {
                return Err(ParseError::new(
                    span,
                    "html!: primal::state must mark an element",
                ))
            }
            ParsedAttr::Action { event: name, .. }
            | ParsedAttr::Dynamic { name, .. }
            | ParsedAttr::StaticDynamic { name, .. }
            | ParsedAttr::Static { name, .. }
            | ParsedAttr::Event {
                event_name: name, ..
            } => {
                return Err(ParseError::new(
                    tag_span,
                    format!(
                        "html!: <{tag}> does not take '{name}' — expected {{}}-valued: {}",
                        allowed.join(", ")
                    ),
                ));
            }
        }
    }
    Ok(out)
}

fn take_builtin_attr(
    attrs: &mut Vec<(String, TokenStream)>,
    name: &str,
    tag: &str,
    tag_span: Span,
) -> ParseResult<TokenStream> {
    match attrs.iter().position(|(n, _)| n == name) {
        Some(at) => Ok(attrs.remove(at).1),
        None => Err(ParseError::new(
            tag_span,
            format!("html!: <{tag}> requires the '{name}={{…}}' attribute"),
        )),
    }
}

/// `<Show when={expr}>{render}</Show>` — exactly one `when`, exactly one
/// `{ }` child.
fn parse_show(cursor: &mut Cursor, tag_span: Span) -> ParseResult<ParsedNode> {
    let mut attrs = parse_builtin_attrs(cursor, "Show", tag_span, &["when"])?;
    let when = take_builtin_attr(&mut attrs, "when", "Show", tag_span)?;
    if cursor.eat_punct('/') {
        return Err(ParseError::new(
            tag_span,
            "html!: <Show> needs a {content} child (it cannot be self-closing)",
        ));
    }
    if !cursor.eat_punct('>') {
        return Err(ParseError::new(
            tag_span,
            "html!: missing '>' after attributes for '<Show>'",
        ));
    }
    let mut content = None;
    loop {
        if cursor.at_closing_tag() {
            break;
        }
        match cursor.peek() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace && content.is_none() => {
                let Some(TokenTree::Group(group)) = cursor.next() else {
                    unreachable!("peeked")
                };
                content = Some(group.stream());
            }
            Some(other) => {
                return Err(ParseError::new(
                    other.span(),
                    "html!: <Show> takes exactly ONE { } child (an impl Render)",
                ));
            }
            None => return Err(ParseError::new(tag_span, "html!: missing '</Show>'")),
        }
    }
    consume_closing_tag(cursor, "Show", tag_span)?;
    let Some(content) = content else {
        return Err(ParseError::new(
            tag_span,
            "html!: <Show> takes exactly ONE { } child (an impl Render)",
        ));
    };
    Ok(ParsedNode::Show {
        span: tag_span,
        when,
        content,
    })
}

/// `<For each={expr} key={fn} render={fn} />` — self-closing only.
fn parse_for(cursor: &mut Cursor, tag_span: Span) -> ParseResult<ParsedNode> {
    let mut attrs = parse_builtin_attrs(cursor, "For", tag_span, &["each", "key", "render"])?;
    let each = take_builtin_attr(&mut attrs, "each", "For", tag_span)?;
    let key = take_builtin_attr(&mut attrs, "key", "For", tag_span)?;
    let render = take_builtin_attr(&mut attrs, "render", "For", tag_span)?;
    if !(cursor.eat_punct('/') && cursor.eat_punct('>')) {
        return Err(ParseError::new(
            tag_span,
            "html!: <For …/> is self-closing (items render through the `render` closure)",
        ));
    }
    Ok(ParsedNode::For {
        span: tag_span,
        each,
        key,
        render,
    })
}

/// Tag and attribute names: `ident(('-'|':')ident)*` — covers `div`,
/// `my-component`, `primal:onclick`, `data-id`.
fn parse_compound_name(cursor: &mut Cursor) -> Option<(String, Span)> {
    let TokenTree::Ident(first) = cursor.peek()? else {
        return None;
    };
    let span = first.span();
    let mut name = first.to_string();
    cursor.next();
    loop {
        let joiner = match cursor.peek() {
            Some(TokenTree::Punct(p)) if matches!(p.as_char(), '-' | ':') => p.as_char(),
            _ => break,
        };
        // Only join when an ident immediately follows (so `disabled >` stops).
        let Some(TokenTree::Ident(next)) = cursor.peek2() else {
            break;
        };
        name.push(joiner);
        name.push_str(&next.to_string());
        cursor.next(); // joiner
        cursor.next(); // ident
    }
    Some((name, span))
}

fn parse_tag_name(cursor: &mut Cursor, open_span: Span) -> ParseResult<(String, Span)> {
    parse_compound_name(cursor)
        .ok_or_else(|| ParseError::new(open_span, "html!: expected a tag name after '<'"))
}

/// Parse attributes up to (but not consuming) `>` or `/>`.
fn parse_attributes(
    cursor: &mut Cursor,
    tag: &str,
    tag_span: Span,
) -> ParseResult<Vec<ParsedAttr>> {
    let mut attrs = Vec::new();
    loop {
        if cursor.is_punct('>') || cursor.is_punct('/') {
            return Ok(attrs);
        }
        if cursor.is_empty() {
            return Err(ParseError::new(
                tag_span,
                format!("html!: missing '>' after attributes for '<{tag}>'"),
            ));
        }
        let Some((name, name_span)) = parse_compound_name(cursor) else {
            return Err(ParseError::new(
                cursor.span(),
                format!("html!: missing '>' after attributes for '<{tag}>'"),
            ));
        };

        if name == "primal" && cursor.eat_punct(':') {
            if !cursor.eat_punct(':')
                || !matches!(cursor.next(), Some(TokenTree::Ident(id)) if id == "state")
            {
                return Err(ParseError::new(name_span, "html!: unknown primal marker"));
            }
            if cursor.is_punct('=') {
                return Err(ParseError::new(
                    name_span,
                    "html!: primal::state is a bare marker; remove its value",
                ));
            }
            attrs.push(ParsedAttr::StateMarker(name_span));
            continue;
        }
        if let Some(event) = name.strip_prefix("on:") {
            attrs.push(parse_action(cursor, event, name_span)?);
            continue;
        }
        if core_tag(tag) {
            check_core_property(cursor, tag, &name, name_span)?;
        }

        if name.starts_with("primal:") {
            attrs.push(parse_primal_attribute(cursor, name, name_span)?);
            continue;
        }

        if cursor.eat_punct('=') {
            match cursor.peek() {
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                    let TokenTree::Group(group) = cursor.next().expect("peeked") else {
                        unreachable!()
                    };
                    attrs.push(ParsedAttr::Dynamic {
                        name,
                        tokens: group.stream(),
                    });
                }
                // `name=[expr]` — static-once interpolation (no effect).
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket => {
                    let TokenTree::Group(group) = cursor.next().expect("peeked") else {
                        unreachable!()
                    };
                    attrs.push(ParsedAttr::StaticDynamic {
                        name,
                        tokens: group.stream(),
                    });
                }
                Some(TokenTree::Literal(_)) => {
                    let TokenTree::Literal(lit) = cursor.next().expect("peeked") else {
                        unreachable!()
                    };
                    let value = syn::parse2::<syn::LitStr>(quote::quote!(#lit))
                        .map_err(|_| {
                            ParseError::new(
                                lit.span(),
                                format!("html!: unterminated string in attr '{name}' of '<{tag}>'"),
                            )
                        })?
                        .value();
                    attrs.push(ParsedAttr::Static { name, value });
                }
                _ => {
                    return Err(ParseError::new(
                        name_span,
                        format!("html!: attribute '{name}' has '=' but no value"),
                    ));
                }
            }
        } else {
            // Bare boolean attribute.
            attrs.push(ParsedAttr::Static {
                name,
                value: String::from("true"),
            });
        }
    }
}

fn parse_primal_attribute(
    cursor: &mut Cursor,
    name: String,
    span: Span,
) -> ParseResult<ParsedAttr> {
    if let Some(event) = name.strip_prefix("primal:on") {
        if !cursor.eat_punct('=') {
            return Err(ParseError::new(
                span,
                format!("html!: attribute '{name}' has '=' but no value"),
            ));
        }
        return Ok(ParsedAttr::Event {
            event_name: event.to_owned(),
            tokens: parse_brace_value(cursor, &name, span)?,
        });
    }
    if matches!(name.as_str(), "primal:style" | "primal:script") {
        return Ok(ParsedAttr::Static {
            name,
            value: String::from("true"),
        });
    }
    Err(ParseError::new(
        span,
        format!("html!: unknown primal attribute '{name}'"),
    ))
}

/// `={ ... }` value for event handlers (always brace-delimited).
fn parse_brace_value(cursor: &mut Cursor, name: &str, span: Span) -> ParseResult<TokenStream> {
    match cursor.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
            let TokenTree::Group(group) = cursor.next().expect("peeked") else {
                unreachable!()
            };
            Ok(group.stream())
        }
        _ => Err(ParseError::new(
            span,
            format!("html!: attribute '{name}' has '=' but no value"),
        )),
    }
}

/// `</tag>` with name verification.
fn consume_closing_tag(cursor: &mut Cursor, expected: &str, open_span: Span) -> ParseResult<()> {
    if !cursor.eat_punct('<') {
        return Err(ParseError::new(
            open_span,
            format!("html!: unclosed tag '<{expected}>'"),
        ));
    }
    if !cursor.eat_punct('/') {
        return Err(ParseError::new(
            cursor.span(),
            format!("html!: unclosed tag '<{expected}>'"),
        ));
    }
    let Some((found, found_span)) = parse_compound_name(cursor) else {
        return Err(ParseError::new(
            cursor.span(),
            format!("html!: unclosed tag '<{expected}>'"),
        ));
    };
    if found != expected {
        return Err(ParseError::new(
            found_span,
            format!("html!: expected closing '</{expected}>', found '</{found}>'"),
        ));
    }
    if !cursor.eat_punct('>') {
        return Err(ParseError::new(
            cursor.span(),
            format!("html!: missing '>' after attributes for '<{expected}>'"),
        ));
    }
    Ok(())
}

// Error-message tests live HERE rather than in `tests/` because a
// `proc-macro = true` crate may only export procedural macros — the parser
// cannot be reached from an integration test, and asserting exact messages
// from `tests/` would require a compile-fail harness (new dependency).

fn parse_action(cursor: &mut Cursor, event: &str, span: Span) -> ParseResult<ParsedAttr> {
    if !matches!(event, "tap" | "change" | "submit" | "focus" | "blur") {
        return Err(ParseError::new(
            span,
            format!("html!: unknown event `{event}`; use tap, change, submit, focus or blur"),
        ));
    }
    if !cursor.eat_punct('=') {
        return Err(ParseError::new(
            span,
            "html!: an action binding requires a value",
        ));
    }
    let value_span = cursor.span();
    let target = match cursor.next() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
            let path = syn::parse2(group.stream()).map_err(|_| {
                ParseError::new(
                    value_span,
                    "html!: a brace action is a Rust action path, for example on:tap={add_todo}",
                )
            })?;
            ActionTarget::Rust(path)
        }
        Some(TokenTree::Literal(lit)) => {
            let value = syn::parse2::<syn::LitStr>(quote::quote!(#lit))
                .ok()
                .map(|s| s.value());
            let valid = value
                .as_deref()
                .and_then(|s| s.strip_prefix("host:"))
                .is_some_and(|s| {
                    !s.is_empty()
                        && s.len() <= 128
                        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                });
            if !valid {
                return Err(ParseError::new(value_span, "html!: a string action reference must start with `host:` and contain a valid name"));
            }
            ActionTarget::Host(value.unwrap())
        }
        _ => {
            return Err(ParseError::new(
                value_span,
                "html!: an action binding requires a Rust path in braces or a host string",
            ))
        }
    };
    Ok(ParsedAttr::Action {
        event: event.to_owned(),
        target,
    })
}

fn check_core_property(cursor: &Cursor, tag: &str, name: &str, span: Span) -> ParseResult<()> {
    if name.starts_with("primal:on") {
        return Err(ParseError::new(
            span,
            "html!: use on:<event> on a core element, not primal:on<event>",
        ));
    }
    if !matches!(
        name,
        "padding"
            | "spacing"
            | "width"
            | "height"
            | "align"
            | "axis"
            | "enabled"
            | "checked"
            | "hidden"
            | "key"
            | "a11y_label"
            | "color"
            | "font"
            | "background"
            | "source"
            | "placeholder"
            | "presentation"
            | "value"
    ) {
        return Err(ParseError::new(
            span,
            format!("html!: `{name}` is not a property of <{tag}>"),
        ));
    }
    let (value, value_span) = if cursor.is_punct('=') {
        let Some(TokenTree::Literal(lit)) = cursor.peek2() else {
            return Ok(());
        };
        let Ok(literal) = syn::parse2::<syn::LitStr>(quote::quote!(#lit)) else {
            return Ok(());
        };
        (literal.value(), literal.span())
    } else {
        (String::from("true"), span)
    };
    let error = match name {
        "padding" | "spacing" | "width" | "height"
            if value != "auto"
                && !value
                    .strip_suffix('%')
                    .unwrap_or(&value)
                    .parse::<f32>()
                    .is_ok_and(f32::is_finite) =>
        {
            "a number, a percent such as \"50%\" or \"auto\""
        }
        "align" if !matches!(value.as_str(), "start" | "center" | "end" | "stretch") => {
            "start, center, end or stretch"
        }
        "axis" if !matches!(value.as_str(), "vertical" | "horizontal") => "vertical or horizontal",
        "enabled" | "checked" | "hidden" if !matches!(value.as_str(), "true" | "false") => {
            "true or false"
        }
        _ => return Ok(()),
    };
    Err(ParseError::new(
        value_span,
        format!("html!: `{name}` needs {error}"),
    ))
}
