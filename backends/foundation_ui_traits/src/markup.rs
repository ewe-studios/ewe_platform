//! WHY: One rendering contract, any target (spec-42 feature 00 §5): a server
//! must be able to emit first-paint HTML from the SAME `Html` values that
//! drive the `DomOp` channel — otherwise the morph contract (server markup vs
//! live DOM diffing cleanly) is unenforceable.
//!
//! WHAT: [`Html::to_markup`] — serialize an `Html` tree to an HTML string:
//! escaped text, quoted/escaped attributes, void elements, and tagless
//! grouping nodes flattened into their children. `parts` and `runtime_id`
//! are mount-time concerns and do not serialize.
//!
//! HOW: A single recursive writer over `alloc::string::String`. Escaping
//! follows the WHATWG serialization rules: text escapes `& < >`, attribute
//! values escape `& "` (values are always double-quoted).

use alloc::string::String;

use crate::html::Html;

/// Tags serialized without children or a closing tag (WHATWG void elements;
/// same set the `html!` parser uses).
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

impl Html {
    /// Serialize this tree to an HTML string.
    ///
    /// Tagless nodes (`tag == None`) contribute their text (escaped) and
    /// children only — exactly how the wasm mount treats them — so
    /// `Vec<Html>` slot wrappers and `Option<Html>` empties serialize
    /// transparently.
    #[must_use]
    pub fn to_markup(&self) -> String {
        let mut out = String::new();
        write_node(self, &mut out);
        out
    }
}

fn write_node(node: &Html, out: &mut String) {
    let Some(tag) = &node.tag else {
        if let Some(text) = &node.text {
            escape_text(text, out);
        }
        for child in &node.children {
            write_node(child, out);
        }
        return;
    };

    // An out-of-table id is wire corruption; serialize a visible marker
    // rather than silently dropping the subtree.
    let name = tag.name().unwrap_or("unknown-tag");
    out.push('<');
    out.push_str(name);
    for (attr, value) in &node.attributes {
        out.push(' ');
        out.push_str(attr.name().unwrap_or("unknown-attr"));
        out.push_str("=\"");
        escape_attribute(value, out);
        out.push('"');
    }
    out.push('>');

    if VOID_ELEMENTS.contains(&name) {
        return;
    }

    if let Some(text) = &node.text {
        escape_text(text, out);
    }
    for child in &node.children {
        write_node(child, out);
    }

    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

fn escape_text(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
}

fn escape_attribute(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
}
