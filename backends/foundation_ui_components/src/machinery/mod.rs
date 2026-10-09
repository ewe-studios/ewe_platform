//! # Machinery — JS-side component behaviors (spec-42 feature 05 §3)
//!
//! WHY: Some component behavior is genuinely the browser's job — roving focus
//! (M5), dismiss (M3), anchored positioning (M1), transitions (M7). Rather than
//! a monolithic hand-maintained runtime, each behavior's JS is **colocated**
//! here beside its Rust contract and DELIVERED as a scoped script.
//!
//! WHAT: [`scoped_script`] (the delivery primitive) + per-behavior submodules:
//! [`composite`] (M5 roving focus), [`dismiss`] (M3 light dismiss),
//! [`position`] (M1 anchored positioning), [`scroll_lock`] (M2 scroll lock),
//! [`focus_trap`] (M4 focus), [`transition`] (M7 enter/leave). A behavior is
//! `function(scope){…}` emitted as a `<script scoped primal:script>` node the
//! component embeds in its root.
//!
//! HOW: The existing scoped-script hydrator (spec-39 feature 09 §5) runs the
//! body ONCE with a `scope` object (`scope.parent()` = the component root),
//! then removes the script — so it's idempotent across morph/re-insert. Options
//! ride the component's own `data-*` attributes, read off `scope.parent()`.
//! Works for SSR (`to_markup`, hydrated at parse) and the reactive runtime
//! (the MutationObserver hydrates inserted subtrees). No `register_function`,
//! no `cfg(wasm32)` gating — a behavior is just markup.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;

use foundation_ui_traits::{AttrName, Html, HtmlTag};

pub mod composite;
pub mod dialog;
pub mod dismiss;
pub mod focus_trap;
pub mod gestures;
pub mod hover;
pub mod listbox;
pub mod measure;
pub mod position;
pub mod scroll_lock;
pub mod transition;

/// Build a `<script scoped primal:script>` node carrying `body` (a JS function
/// expression `function(scope){…}`). Embed it inside the component root; the
/// hydrator runs it once with `scope` (where `scope.parent()` is that root)
/// and removes it.
#[must_use]
pub fn scoped_script(body: impl Into<Cow<'static, str>>) -> Html {
    Html {
        tag: Some(HtmlTag::from_static("script")),
        text: None,
        children: vec![Html::text(body)],
        attributes: vec![
            (AttrName::from_static("primal:script"), Cow::Borrowed("true")),
            (AttrName::from_static("scoped"), Cow::Borrowed("")),
        ],
        parts: Vec::new(),
        runtime_id: None,
    }
}
