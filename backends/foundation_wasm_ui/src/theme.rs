//! WHY: Theme CSS must be in `<head>` BEFORE the first element renders
//! (feature 09 §8) — so it ships as the first operation batch after init.
//!
//! WHAT: [`inject_theme_css`] — queues the four-op sequence (create style,
//! register, set text, append to the reserved `<head>` ambient node) on a
//! receiver; flush it before any content batch.
//!
//! HOW: Uses the reserved ambient ids the JS `NodeRegistry` seeds (0 = head)
//! plus [`THEME_STYLE_NODE_ID`] from the reserved 0-15 range that
//! `allocate_id_block` never hands out.

use crate::ops;
use foundation_ui_traits::{AttrName, HtmlTag};

use crate::runtime::SharedInstructionReceiver;

/// Reserved id for the theme `<style>` element (ambient range 0-15; the JS
/// registry seeds 0=head, 1=body, 2=html).
pub const THEME_STYLE_NODE_ID: u32 = 3;

/// Reserved ambient id of `<head>`.
pub const HEAD_NODE_ID: u32 = 0;

/// Reserved ambient id of `<body>` — what a root tree mounts onto (the JS
/// `NodeRegistry` seeds it from `document.body`). See [`crate::App::mount`].
pub const BODY_NODE_ID: u32 = 1;

/// Queue the theme stylesheet as DOM ops targeting `<head>`. Call before the
/// first content batch (its flush IS the first batch, feature 09 §8).
///
/// Accepts any `&str` — a `&'static` from the `theme!{}` macro / `ThemeTokens`
/// derive, or a runtime-built `String`'s slice from the `Theme` builder; the
/// text is copied into the op either way.
pub fn inject_theme_css(receiver: &SharedInstructionReceiver, css: &str) {
    receiver.queue(ops::create_element(
        THEME_STYLE_NODE_ID,
        &HtmlTag::from_static("style"),
        "",
    ));
    receiver.queue(ops::register_node(THEME_STYLE_NODE_ID));
    receiver.queue(ops::set_attribute(
        THEME_STYLE_NODE_ID,
        &AttrName::from_static("id"),
        "primal-theme",
    ));
    receiver.queue(ops::set_text(THEME_STYLE_NODE_ID, css));
    receiver.queue(ops::append_child(HEAD_NODE_ID, THEME_STYLE_NODE_ID));
}
