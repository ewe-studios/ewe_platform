//! Browser and native behaviours for interactive components.
//! Named helpers return markers. A renderer attaches the behaviour to the marker's parent.
//! An anonymous scoped script carries an application-specific JavaScript implementation.

pub use crate::behaviour::behaviour_script;
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
            (
                AttrName::from_static("primal:script"),
                Cow::Borrowed("true"),
            ),
            (AttrName::from_static("scoped"), Cow::Borrowed("")),
        ],
        parts: Vec::new(),
        runtime_id: None,
    }
}
