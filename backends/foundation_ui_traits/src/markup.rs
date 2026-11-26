//! WHY: One rendering contract, any target (spec-42 feature 00 §5): a server
//! must be able to emit first-paint HTML from the SAME `Html` values that
//! drive the frame operation channel — otherwise the morph contract (server markup vs
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
        let result = self.write_markup(|chunk| {
            out.push_str(chunk);
            Ok::<_, core::convert::Infallible>(())
        });
        let Ok(()) = result;
        out
    }

    /// Writes markup in document order. The sink can stop the operation on any chunk.
    ///
    /// # Errors
    /// Returns the first error from the sink.
    pub fn write_markup<E>(&self, mut emit: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
        write_node(self, &mut emit)
    }
}

fn write_node<E>(node: &Html, emit: &mut impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
    let tag = node
        .tag
        .as_ref()
        .map(|tag| tag.name().unwrap_or("unknown-tag"));
    if let Some(name) = tag {
        emit("<")?;
        emit(name)?;
        for (attr, value) in &node.attributes {
            emit(" ")?;
            emit(attr.name().unwrap_or("unknown-attr"))?;
            emit("=\"")?;
            escape(value, true, emit)?;
            emit("\"")?;
        }
        emit(">")?;
        if VOID_ELEMENTS.contains(&name) {
            return Ok(());
        }
    }
    if let Some(text) = &node.text {
        escape(text, false, emit)?;
    }
    for child in &node.children {
        write_node(child, emit)?;
    }
    if let Some(name) = tag {
        emit("</")?;
        emit(name)?;
        emit(">")?;
    }
    Ok(())
}

fn escape<E>(
    value: &str,
    attribute: bool,
    emit: &mut impl FnMut(&str) -> Result<(), E>,
) -> Result<(), E> {
    let mut start = 0;
    for (index, byte) in value.bytes().enumerate() {
        let entity = match byte {
            b'&' => "&amp;",
            b'"' if attribute => "&quot;",
            b'<' if !attribute => "&lt;",
            b'>' if !attribute => "&gt;",
            _ => continue,
        };
        if start < index {
            emit(&value[start..index])?;
        }
        emit(entity)?;
        start = index + 1;
    }
    if start < value.len() {
        emit(&value[start..])?;
    }
    Ok(())
}
