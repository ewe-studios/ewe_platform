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
    fn new(span: Span, message: impl Into<String>) -> Self {
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
    /// Literal text content (quoted strings AND bare token runs).
    Text(String),
    /// A `{ ... }` slot — the expression tokens, untouched.
    Slot(TokenStream),
}

/// One parsed attribute.
pub(crate) enum ParsedAttr {
    /// `name="value"` or bare boolean `name` (value "true").
    Static { name: String, value: String },
    /// `name={expr}`.
    Dynamic { name: String, tokens: TokenStream },
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
fn parse_attributes(cursor: &mut Cursor, tag: &str, tag_span: Span) -> ParseResult<Vec<ParsedAttr>> {
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

        // `primal:` namespace: only `primal:on*` is meaningful in templates.
        if let Some(rest) = name.strip_prefix("primal:") {
            if let Some(event) = rest.strip_prefix("on") {
                if !cursor.eat_punct('=') {
                    return Err(ParseError::new(
                        name_span,
                        format!("html!: attribute '{name}' has '=' but no value"),
                    ));
                }
                let tokens = parse_brace_value(cursor, &name, name_span)?;
                attrs.push(ParsedAttr::Event {
                    event_name: event.to_string(),
                    tokens,
                });
                continue;
            }
            return Err(ParseError::new(
                name_span,
                format!("html!: unknown primal attribute '{name}'"),
            ));
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
                Some(TokenTree::Literal(_)) => {
                    let TokenTree::Literal(lit) = cursor.next().expect("peeked") else {
                        unreachable!()
                    };
                    let value = syn::parse2::<syn::LitStr>(quote::quote!(#lit))
                        .map_err(|_| {
                            ParseError::new(
                                lit.span(),
                                format!(
                                    "html!: unterminated string in attr '{name}' of '<{tag}>'"
                                ),
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
#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn err(stream: TokenStream) -> String {
        match parse_template(stream) {
            Err(e) => e.message,
            Ok(_) => panic!("expected a parse error"),
        }
    }

    /// Spec test 35.
    #[test]
    fn unclosed_tag() {
        assert_eq!(err(quote! { <div> }), "html!: unclosed tag '<div>'");
    }

    /// Spec test 36.
    #[test]
    fn mismatched_closing_tag() {
        assert_eq!(
            err(quote! { <div></span> }),
            "html!: expected closing '</div>', found '</span>'"
        );
    }

    /// Spec test 37.
    #[test]
    fn empty_input() {
        assert_eq!(err(TokenStream::new()), "html!: empty input");
    }

    /// Spec test 38.
    #[test]
    fn unclosed_nested_tag() {
        assert_eq!(err(quote! { <div><p></p> }), "html!: unclosed tag '<div>'");
    }

    /// Spec test 39.
    #[test]
    fn equals_without_value() {
        assert_eq!(
            err(quote! { <div class= ></div> }),
            "html!: attribute 'class' has '=' but no value"
        );
    }

    #[test]
    fn multiple_roots_rejected() {
        assert_eq!(
            err(quote! { <div></div><span></span> }),
            "html!: multiple root elements — wrap in a parent"
        );
    }

    #[test]
    fn unknown_primal_attribute() {
        assert_eq!(
            err(quote! { <div primal:bogus="x"></div> }),
            "html!: unknown primal attribute 'primal:bogus'"
        );
    }

    #[test]
    fn void_elements_need_no_close() {
        let node = parse_template(quote! { <div><br><img /><hr></div> }).unwrap();
        let ParsedNode::Element { children, .. } = node else {
            panic!("root must be an element");
        };
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn bare_text_keeps_punct_glue() {
        let node = parse_template(quote! { <b>Count: {n}</b> }).unwrap();
        let ParsedNode::Element { children, .. } = node else {
            panic!()
        };
        assert!(matches!(&children[0], ParsedNode::Text(t) if t == "Count:"));
        assert!(matches!(&children[1], ParsedNode::Slot(_)));
    }
}
