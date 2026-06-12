//! WHY: Feature 09's two compile-time CSS systems — scoped `<style
//! primal:style>` transformation inside `html!`, and `#[derive(ThemeTokens)]`
//! utility generation — must produce exactly the CSS the spec's tables
//! describe, with zero runtime processing.
//!
//! WHAT: Scoped-style extraction/combination/`:parent` resolution in both
//! macro forms (inline `<style>` child for pure, head-injection ops for
//! reactive), the theme derive's custom properties / dark handling / utility
//! classes, and the first-batch injection helper.

use std::rc::Rc;

use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_ui_traits::DomOp;
use foundation_wasm::MemoryAllocations;
use foundation_wasm_ui::{
    html, inject_theme_css, MockProtocol, Runtime, ThemeTokens, HEAD_NODE_ID, THEME_STYLE_NODE_ID,
};

// ─── Scoped styles in html! (decision 019) ─────────────────────────────────────

/// Pure form: the style tag is REMOVED; a combined, transformed
/// `data-primal-scoped` style child appears instead.
#[test]
fn pure_form_transforms_and_combines_scoped_styles() {
    let tree = html! {
        <div id="menu-tabs">
            <style primal:style>{":parent { background: white; } .title { font-size: 20px; }"}</style>
            <style primal:style>{".subtitle { color: gray; }"}</style>
            <span>"content"</span>
        </div>
    };
    // Only the span + ONE combined scoped style remain.
    assert_eq!(tree.children.len(), 2);
    let style = &tree.children[1];
    assert_eq!(
        style.tag.as_ref().and_then(foundation_ui_traits::HtmlTag::name),
        Some("style")
    );
    let css = style.children[0].text.as_deref().unwrap();
    assert!(css.contains("#menu-tabs { background: white; }"), "{css}");
    assert!(css.contains("#menu-tabs .title { font-size: 20px; }"));
    assert!(css.contains("#menu-tabs .subtitle { color: gray; }"), "combined (§4)");
}

/// Class-identity fallback when there is no id (id wins when both exist).
#[test]
fn scoped_styles_use_first_class_without_id() {
    let tree = html! {
        <div class="sidebar wide">
            <style primal:style>{":parent { margin: 0; }"}</style>
        </div>
    };
    let css = tree.children[0].children[0].text.as_deref().unwrap();
    assert!(css.contains(".sidebar { margin: 0; }"), "{css}");
}

/// Reactive form: the scoped style ships as head-injection ops in the SAME
/// batch as the tree build (§8.3 — reserved ambient id 0 = head).
#[test]
fn reactive_form_ships_scoped_styles_to_head() {
    let signals = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&signals));
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let runtime = Runtime::builder()
        .protocol(mock)
        .memory(MemoryAllocations::new())
        .build();
    runtime.attach(&signals);
    let receiver = runtime.receiver();

    let _tree = html! { ctx, receiver,
        <div id="panel">
            <style primal:style>{":parent:hover { opacity: 0.8; }"}</style>
        </div>
    };
    signals.stabilize();

    let ops: Vec<DomOp> = sent.borrow().iter().flatten().cloned().collect();
    let style_create = ops.iter().find_map(|op| match op {
        DomOp::CreateElement { node_id, tag, .. } if tag.name() == Some("style") => Some(*node_id),
        _ => None,
    });
    let style_id = style_create.expect("style element created");
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::SetText { node_id, text } if *node_id == style_id
            && text.contains("#panel:hover { opacity: 0.8; }")
    )));
    assert!(
        ops.iter().any(|op| matches!(
            op,
            DomOp::AppendChild { parent_id: 0, child_id } if *child_id == style_id
        )),
        "appended to the reserved <head> ambient id"
    );
    assert!(style_id >= 16, "instance ids stay out of the reserved 0-15 range");
}

// ─── ThemeTokens derive (decision 020) ─────────────────────────────────────────

// Token fields are unit markers — the derive reads only their attributes.
#[allow(dead_code)]
#[derive(ThemeTokens)]
struct AppTheme {
    #[token(category = "color", light = "#3b82f6", dark = "#60a5fa")]
    primary: (),
    #[token(category = "color", light = "#10b981")]
    secondary: (),
    #[token(category = "spacing", value = "16px")]
    md: (),
    #[token(category = "radius", value = "4px")]
    sm: (),
    #[token(category = "shadow", value = "0 1px 2px rgba(0,0,0,0.1)")]
    soft: (),
}

#[test]
fn theme_tokens_generate_the_spec_css() {
    let theme = AppTheme::new();
    let css = theme.css_string();

    // Custom properties (light).
    assert!(css.contains("--color-primary: #3b82f6;"), "{css}");
    assert!(css.contains("--spacing-md: 16px;"));

    // Dark block: explicit override + auto-derived (~80% luminance).
    assert!(css.contains("@media (prefers-color-scheme: dark)"));
    assert!(css.contains("--color-primary: #60a5fa;"), "explicit dark");
    assert!(css.contains("--color-secondary: #0d9467;"), "auto-derived dark: {css}");

    // Utility classes per category (§7.1 table).
    assert!(css.contains(".bg-primary { background-color: var(--color-primary); }"));
    assert!(css.contains(".text-primary { color: var(--color-primary); }"));
    assert!(css.contains(".border-secondary { border-color: var(--color-secondary); }"));
    assert!(css.contains(".p-md { padding: var(--spacing-md); }"));
    assert!(css.contains(".m-md { margin: var(--spacing-md); }"));
    assert!(css.contains(".rounded-sm { border-radius: var(--border-radius-sm); }")
        || css.contains(".rounded-sm { border-radius: var(--radius-sm); }"));
    assert!(css.contains(".shadow-soft { box-shadow: var(--shadow-soft); }"));

    // Built-in utilities (§7.2).
    assert!(css.contains(".flex { display: flex; }"));
    assert!(css.contains(".hidden { display: none; }"));
    assert!(css.contains(".w-full { width: 100%; }"));

    // The whole thing is one 'static string.
    let _: &'static str = AppTheme::CSS;
}

/// First-batch injection (§8): four ops + the id attribute, targeting head.
#[test]
fn inject_theme_css_queues_the_head_sequence() {
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let runtime = Runtime::builder()
        .protocol(mock)
        .memory(MemoryAllocations::new())
        .build();
    let receiver = runtime.receiver();

    inject_theme_css(&receiver, AppTheme::CSS);
    receiver.flush().expect("theme batch ships");

    let batch = &sent.borrow()[0];
    assert!(matches!(
        batch[0],
        DomOp::CreateElement { node_id, ref tag, .. }
            if node_id == THEME_STYLE_NODE_ID && tag.name() == Some("style")
    ));
    assert!(matches!(batch[1], DomOp::RegisterNode { node_id } if node_id == THEME_STYLE_NODE_ID));
    assert!(matches!(
        &batch[2],
        DomOp::SetAttribute { value, .. } if value == "primal-theme"
    ));
    assert!(matches!(
        &batch[3],
        DomOp::SetText { text, .. } if text.contains("--color-primary")
    ));
    assert!(matches!(
        batch[4],
        DomOp::AppendChild { parent_id, child_id }
            if parent_id == HEAD_NODE_ID && child_id == THEME_STYLE_NODE_ID
    ));
}
