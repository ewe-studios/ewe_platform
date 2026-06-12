//! # Separator (F1 — Primitives)
//!
//! Pure markup. `<div role="separator" aria-orientation data-orientation>`.
//! All static; a PURE component (works without ctx/receiver — first proof
//! that the catalog supports no-runtime components).

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

/// Static config for a separator.
pub struct SeparatorConfig {
    /// Orientation (default: horizontal).
    pub orientation: Orientation,
    /// Optional custom class.
    pub class: Option<&'static str>,
    /// Optional decorative label (rendered visually via CSS).
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
/// Pure: no ctx/receiver needed. All config is static.
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
    let orientation_str = match config.orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    };
    let class = config.class.unwrap_or("separator");

    if config.decorative {
        html! {
            <div class={class}
                 role="separator"
                 aria-orientation={orientation_str}
                 data-orientation={orientation_str}
                 aria-hidden="true"/>
        }
    } else {
        html! {
            <div class={class}
                 role="separator"
                 aria-orientation={orientation_str}
                 data-orientation={orientation_str}/>
        }
    }
}
