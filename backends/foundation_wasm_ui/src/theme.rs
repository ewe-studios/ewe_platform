//! WHY: Theme CSS must be in `<head>` BEFORE the first element renders
//! (feature 09 §8) — so it ships as the first `DomOp` batch after init.
//!
//! WHAT: [`inject_theme_css`] — queues the four-op sequence (create style,
//! register, set text, append to the reserved `<head>` ambient node) on a
//! receiver; flush it before any content batch.
//!
//! HOW: Uses the reserved ambient ids the JS `NodeRegistry` seeds (0 = head)
//! plus [`THEME_STYLE_NODE_ID`] from the reserved 0-15 range that
//! `allocate_id_block` never hands out.

use foundation_ui_traits::{AttrName, DomOp, HtmlTag};

use crate::runtime::SharedInstructionReceiver;

/// Reserved id for the theme `<style>` element (ambient range 0-15; the JS
/// registry seeds 0=head, 1=body, 2=html).
pub const THEME_STYLE_NODE_ID: u32 = 3;

/// Reserved ambient id of `<head>`.
pub const HEAD_NODE_ID: u32 = 0;

/// Queue the theme stylesheet as DOM ops targeting `<head>`. Call before the
/// first content batch (its flush IS the first batch, feature 09 §8).
pub fn inject_theme_css(receiver: &SharedInstructionReceiver, css: &'static str) {
    receiver.queue(DomOp::CreateElement {
        node_id: THEME_STYLE_NODE_ID,
        tag: HtmlTag::from_static("style"),
        class: "".into(),
    });
    receiver.queue(DomOp::RegisterNode {
        node_id: THEME_STYLE_NODE_ID,
    });
    receiver.queue(DomOp::SetAttribute {
        node_id: THEME_STYLE_NODE_ID,
        name: AttrName::from_static("id"),
        value: "primal-theme".into(),
    });
    receiver.queue(DomOp::SetText {
        node_id: THEME_STYLE_NODE_ID,
        text: css.into(),
    });
    receiver.queue(DomOp::AppendChild {
        parent_id: HEAD_NODE_ID,
        child_id: THEME_STYLE_NODE_ID,
    });
}
