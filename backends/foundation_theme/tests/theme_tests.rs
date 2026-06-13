//! WHY: `theme_css` is the single CSS generator behind the `theme!{}` macro,
//! the `ThemeTokens` derive, and the runtime `Theme` builder — its output IS
//! the styling contract (spec-39 decision 021 / feature 09 §7).
//!
//! WHAT: the generated `:root` custom properties, the dark `@media` block with
//! explicit-vs-auto-derived values, per-category utility classes, and the
//! `auto_dark` luminance math (the worked example must hold exactly).
//!
//! HOW: build tokens via the runtime `Theme` builder (same path the macro
//! lowers to) and assert on the produced string.

use foundation_theme::{auto_dark, theme_css, GeneratedTheme, Theme, ThemeToken};

#[test]
fn root_custom_properties_and_utilities() {
    let theme = Theme::new()
        .color("primary", "#3b82f6", Some("#60a5fa"))
        .color("secondary", "#10b981", None)
        .spacing("md", "16px")
        .radius("sm", "4px")
        .shadow("soft", "0 1px 2px rgba(0,0,0,0.1)")
        .build();
    let css = theme.to_css();

    // Custom properties.
    assert!(css.contains("--color-primary: #3b82f6;"), "{css}");
    assert!(css.contains("--spacing-md: 16px;"));
    assert!(css.contains("--radius-sm: 4px;"));

    // Dark block: explicit override + auto-derived (~80% luminance).
    assert!(css.contains("@media (prefers-color-scheme: dark)"));
    assert!(css.contains("--color-primary: #60a5fa;"), "explicit dark");
    assert!(css.contains("--color-secondary: #0d9467;"), "auto dark: {css}");

    // Per-category utility classes.
    assert!(css.contains(".bg-primary { background-color: var(--color-primary); }"));
    assert!(css.contains(".text-primary { color: var(--color-primary); }"));
    assert!(css.contains(".p-md { padding: var(--spacing-md); }"));
    assert!(css.contains(".m-md { margin: var(--spacing-md); }"));
    assert!(css.contains(".rounded-sm { border-radius: var(--radius-sm); }"));
    assert!(css.contains(".shadow-soft { box-shadow: var(--shadow-soft); }"));

    // Built-in utilities.
    assert!(css.contains(".flex { display: flex; }"));
    assert!(css.contains(".hidden { display: none; }"));

    // Generated opacity scale: 0 -> 100 step 5, endpoints and fine variants.
    assert!(css.contains(".opacity-0 { opacity: 0; }"));
    assert!(css.contains(".opacity-5 { opacity: 0.05; }"));
    assert!(css.contains(".opacity-50 { opacity: 0.50; }"));
    assert!(css.contains(".opacity-100 { opacity: 1; }"));
    assert!(!css.contains(".opacity-105"), "scale stops at 100");
}

#[test]
fn auto_dark_handles_both_hex_widths_and_rejects_non_hex() {
    // 6-digit: the canonical worked example.
    assert_eq!(auto_dark("#10b981").as_deref(), Some("#0d9467"));
    // 3-digit expands then darkens (#fff -> ff,ff,ff -> cc,cc,cc).
    assert_eq!(auto_dark("#fff").as_deref(), Some("#cccccc"));
    // Non-hex: no derivation.
    assert_eq!(auto_dark("rgba(0,0,0,0.1)"), None);
    assert_eq!(auto_dark("16px"), None);
}

#[test]
fn non_color_categories_get_no_dark_entry() {
    // Only spacing — the dark @media block must be absent entirely.
    let css = theme_css(&[ThemeToken::owned("md", "spacing", "16px", None)]);
    assert!(!css.contains("prefers-color-scheme"), "{css}");
}

#[test]
fn const_from_static_is_inspectable() {
    // What the `theme!{}` macro emits: a const-capable GeneratedTheme.
    static TOKENS: &[ThemeToken] = &[ThemeToken::from_static(
        "primary",
        "color",
        "#3b82f6",
        Some("#60a5fa"),
    )];
    const THEME: GeneratedTheme = GeneratedTheme::from_static(
        ":root {\n  --color-primary: #3b82f6;\n}\n",
        TOKENS,
    );
    assert_eq!(THEME.value("color", "primary"), Some("#3b82f6"));
    assert!(THEME.to_css().contains("--color-primary"));
    assert_eq!(THEME.tokens().len(), 1);
}
