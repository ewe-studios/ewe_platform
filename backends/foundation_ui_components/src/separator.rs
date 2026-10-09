//! # Separator (F1 — Primitives)
//!
//! WHY: A divider that styles cleanly and carries the right semantics for
//! assistive tech (spec-42 feature 05 §F1).
//!
//! WHAT: Pure markup — `<div role="separator">` with `aria-orientation` /
//! `data-orientation`. A PURE component: no `ctx`/receiver, all config static.
//! First proof the catalog supports no-runtime components.
//!
//! HOW: One `html!` block in the pure form. `aria-hidden` uses an Option-valued
//! attribute (features.md §8.2) so it is PRESENT only when decorative — never
//! `aria-hidden="false"`, which would be a different (worse) semantic.

use alloc::borrow::Cow;

use foundation_ui_traits::Html;
use foundation_wasm_ui::html;

/// Orientation of the separator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal divider.
    #[default]
    Horizontal,
    /// Vertical divider.
    Vertical,
}

impl Orientation {
    /// The `aria-orientation` / `data-orientation` token.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Horizontal => "horizontal",
            Orientation::Vertical => "vertical",
        }
    }
}

/// Static config for a separator. Text fields are `Cow<'static, str>`
/// (features.md §8.1): literals cost nothing, owned `String` is accepted too.
pub struct SeparatorConfig {
    /// Orientation (default: horizontal).
    pub orientation: Orientation,
    /// Class override for the divider element (default: `"separator"`).
    pub class: Option<Cow<'static, str>>,
    /// Decorative dividers are hidden from assistive tech (`aria-hidden`).
    pub decorative: bool,
}

impl Default for SeparatorConfig {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            class: None,
            decorative: true,
        }
    }
}

/// Separator component — renders a themed divider.
///
/// Pure: no `ctx`/receiver needed. All config is static.
///
/// # Example
///
/// ```ignore
/// html! {
///     <div class="card">
///         <h2>Header</h2>
///         {separator(SeparatorConfig::default())}
///         <p>Content</p>
///     </div>
/// }
/// ```
#[must_use]
pub fn separator(config: SeparatorConfig) -> Html {
    let orientation = config.orientation.as_str();
    let class = config.class.unwrap_or(Cow::Borrowed("separator"));
    html! {
        <div class={class}
             role="separator"
             aria-orientation={orientation}
             data-orientation={orientation}
             aria-hidden={config.decorative.then_some("true")}/>
    }
}
