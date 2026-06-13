//! # Navigation menu (F5 — Menus)
//!
//! WHY: Website navigation (links + mega-panels) — distinct from `menu`: it's
//! `nav` + disclosure (NOT `role="menu"`), one panel open at a time (spec-42
//! §F5).
//!
//! WHAT: [`navigation_menu`] over an `(active, set_active)` `Option<String>`
//! signal (which item's panel is open) + a `Vec<NavItem>` (link items or
//! trigger+content items).
//!
//! HOW: `<nav>` → list → items. A trigger item toggles its panel via `active`
//! (`aria-expanded` + linked panel, mounted/`hidden` per the visibility model,
//! M7 transitions, `data-activation-direction` for slides). Link items are real
//! `<a data-active>`. The list is an M5 composite (arrow roving across
//! triggers). Hover-intent (delay 50/50) and the single morphing Viewport are
//! documented follow-ups; per-item panels ship in v1.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::composite::composite_behavior;
use crate::machinery::transition::transition_behavior;

/// One navigation-menu item: either a link, or a trigger with a content panel.
pub struct NavItem {
    /// Identifying value (`<For>` key + active membership).
    pub value: String,
    /// Trigger/link content.
    pub trigger: Slot,
    /// Panel content (ignored when `href` is set).
    pub content: Option<Slot>,
    /// If set, this is a plain link item (`<a href>`), no panel.
    pub href: Option<Cow<'static, str>>,
    /// For link items: mark the current page (`data-active`).
    pub current: bool,
}

/// Static config for a navigation menu.
pub struct NavMenuConfig {
    /// Class override (default `"navigation-menu"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` for the `<nav>`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for NavMenuConfig {
    fn default() -> Self {
        Self { class: None, aria_label: None }
    }
}

/// Navigation-menu component.
#[must_use]
pub fn navigation_menu(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: NavMenuConfig,
    active: &SignalGetter<Option<String>>,
    set_active: SignalSetter<Option<String>>,
    items: Vec<NavItem>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("navigation-menu"));

    // The item set is static; render eagerly (each item's per-open reactivity
    // rides its own reactive attrs). `NavItem` holds a `Slot`, so it isn't
    // `Clone` and can't go through `<For>`.
    let rendered: Vec<Html> = items
        .into_iter()
        .map(|item| nav_item_html(ctx, rcv, active, &set_active, item))
        .collect();

    html! { ctx, rcv,
        <nav class=[class] aria-label=[config.aria_label]>
            <ul class="navigation-menu-list" role="menubar"
                data-composite="true" data-orientation="horizontal">
                <Fragment>{rendered.clone()}</Fragment>
                <Fragment>{composite_behavior()}</Fragment>
            </ul>
        </nav>
    }
}

fn nav_item_html(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    active: &SignalGetter<Option<String>>,
    set_active: &SignalSetter<Option<String>>,
    item: NavItem,
) -> Html {
    let value = item.value.clone();
    let trigger_html = item.trigger.render(ctx, rcv);
    let trigger_id: Cow<'static, str> = Cow::Owned(alloc::format!("nav-trigger-{value}"));
    let panel_id: Cow<'static, str> = Cow::Owned(alloc::format!("nav-panel-{value}"));

    if let Some(href) = item.href {
        return html! { ctx, rcv,
            <li class="navigation-menu-item">
                <a class="navigation-menu-link" role="menuitem"
                   data-composite-item="true"
                   href=[href]
                   data-active={item.current.then_some("")}
                   aria-current={item.current.then_some("page")}>
                    <Fragment>{trigger_html.clone()}</Fragment>
                </a>
            </li>
        };
    }

    let content_html = item.content.map(|s| s.render(ctx, rcv));
    let trigger_ref = trigger_id.clone();
    let panel_ref = panel_id.clone();

    let toggle = {
        let active = active.clone();
        let set_active = set_active.clone();
        let value = value.clone();
        ctx.callback(move |_| {
            if active.get().as_deref() == Some(value.as_str()) {
                set_active.set(None);
            } else {
                set_active.set(Some(value.clone()));
            }
        })
    };

    let is_open = {
        let active = active.clone();
        let value = value.clone();
        move || active.get().as_deref() == Some(value.as_str())
    };
    let t_expanded = is_open.clone();
    let t_popup = is_open.clone();
    let p_open = is_open.clone();
    let p_closed = is_open.clone();
    let p_hidden = is_open.clone();
    html! { ctx, rcv,
        <li class="navigation-menu-item">
            <button type="button" class="navigation-menu-trigger"
                    id=[trigger_id]
                    data-composite-item="true"
                    aria-expanded={t_expanded()}
                    aria-controls=[panel_ref]
                    data-popup-open={t_popup().then_some("")}
                    primal:onclick={toggle}>
                <Fragment>{trigger_html.clone()}</Fragment>
            </button>
            <div class="navigation-menu-content" id=[panel_id]
                 role="region" aria-labelledby=[trigger_ref]
                 data-open={p_open().then_some("")}
                 data-closed={(!p_closed()).then_some("")}
                 hidden={(!p_hidden()).then_some("")}>
                <Fragment>{content_html.clone()}</Fragment>
                <Fragment>{transition_behavior()}</Fragment>
            </div>
        </li>
    }
}
