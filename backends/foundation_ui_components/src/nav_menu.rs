//! # Navigation menu (F5 — Menus)
//!
//! WHY: Website navigation (links + mega-panels) — distinct from `menu`: it's
//! `nav` + disclosure (NOT `role="menu"`), one panel open at a time, with ONE
//! shared morphing Viewport that resizes/slides between items' content
//! (spec-42 §F5; base-ui navigation-menu).
//!
//! WHAT: [`navigation_menu`] over an `(active, set_active)` `Option<String>`
//! signal + a `Vec<NavItem>` (link items or trigger+content items).
//!
//! HOW: `<nav>` → list (M5 composite roving across triggers) → a single
//! Positioner/Viewport popup whose contents are all mounted but only the active
//! one is shown (`data-open`). A scoped script measures the active content into
//! `--popup-width/--popup-height` (the resize transition) and computes
//! `data-activation-direction` (`left|right`) by comparing the newly-active
//! trigger's rect to the previous one — ported from base-ui
//! `NavigationMenuTrigger`/`Viewport`. M1 anchors the Viewport to the list; M7
//! animates content swaps via `data-activation-direction` + starting/ending
//! styles. Link items are real `<a data-active>`.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::composite::composite_behavior;
use crate::machinery::position::position_behavior;
use crate::machinery::scoped_script;
use crate::machinery::transition::transition_behavior;

/// One navigation-menu item: either a link, or a trigger with a content panel.
pub struct NavItem {
    /// Identifying value (active membership).
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

/// Shared-Viewport behavior: measure the active content → `--popup-width/height`
/// and compute `data-activation-direction` from trigger-rect comparison.
pub const NAV_MENU_JS: &str = r#"function(scope){
  var popup = scope.parent();
  if (!popup || popup.__nav) return; popup.__nav = true;
  var win = (popup.ownerDocument && popup.ownerDocument.defaultView) || window;
  var nav = popup.closest('nav') || popup.ownerDocument;
  var prev = null;
  function activeContent(){ return popup.querySelector('[data-nav-content][data-open]'); }
  function update(){
    var val = popup.getAttribute('data-active-value') || '';
    if (val && prev && val !== prev) {
      var t1 = nav.querySelector('[data-nav-trigger="' + prev + '"]');
      var t2 = nav.querySelector('[data-nav-trigger="' + val + '"]');
      if (t1 && t2) {
        var r1 = t1.getBoundingClientRect(), r2 = t2.getBoundingClientRect();
        var dir = r2.left > r1.left ? 'right' : (r2.left < r1.left ? 'left' : '');
        if (dir) {
          popup.setAttribute('data-activation-direction', dir);
          var cs = popup.querySelectorAll('[data-nav-content]');
          for (var i = 0; i < cs.length; i++) cs[i].setAttribute('data-activation-direction', dir);
        }
      }
    }
    if (val) prev = val;
    var c = activeContent();
    if (c) {
      popup.style.setProperty('--popup-width', c.offsetWidth + 'px');
      popup.style.setProperty('--popup-height', c.offsetHeight + 'px');
    }
  }
  update();
  var mo = new MutationObserver(update);
  mo.observe(popup, { attributes: true, attributeFilter: ['data-active-value', 'data-open'] });
  if (win.ResizeObserver) { var ro = new win.ResizeObserver(update); ro.observe(popup); }
}"#;

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
    let id_n = ctx.allocate_id_block(1);
    let list_id = alloc::format!("nav-list-{id_n}");

    // Render triggers/links into the list, and contents into the shared viewport.
    let mut triggers: Vec<Html> = Vec::new();
    let mut contents: Vec<Html> = Vec::new();
    for item in items {
        let value = item.value.clone();
        let trigger_html = item.trigger.render(ctx, rcv);
        let trigger_id: Cow<'static, str> = Cow::Owned(alloc::format!("nav-trigger-{value}"));
        let panel_id: Cow<'static, str> = Cow::Owned(alloc::format!("nav-panel-{value}"));

        if let Some(href) = item.href {
            triggers.push(html! { ctx, rcv,
                <li class="navigation-menu-item">
                    <a class="navigation-menu-link" role="menuitem"
                       data-composite-item="true"
                       href=[href]
                       data-active={item.current.then_some("")}
                       aria-current={item.current.then_some("page")}>
                        <Fragment>{trigger_html.clone()}</Fragment>
                    </a>
                </li>
            });
            continue;
        }

        let nav_trigger_attr: Cow<'static, str> = Cow::Owned(value.clone());
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
        let t_expanded = {
            let active = active.clone();
            let value = value.clone();
            move || active.get().as_deref() == Some(value.as_str())
        };
        let t_popup = t_expanded.clone();
        triggers.push(html! { ctx, rcv,
            <li class="navigation-menu-item">
                <button type="button" class="navigation-menu-trigger"
                        id=[trigger_id.clone()]
                        data-composite-item="true"
                        data-nav-trigger=[nav_trigger_attr]
                        aria-expanded={t_expanded()}
                        aria-controls=[panel_ref]
                        data-popup-open={t_popup().then_some("")}
                        primal:onclick={toggle}>
                    <Fragment>{trigger_html.clone()}</Fragment>
                </button>
            </li>
        });

        // Content goes into the shared viewport (mounted; shown when active).
        let content_html = item.content.map(|s| s.render(ctx, rcv));
        let c_open = {
            let active = active.clone();
            let value = value.clone();
            move || active.get().as_deref() == Some(value.as_str())
        };
        let c_closed = c_open.clone();
        let c_hidden = c_open.clone();
        contents.push(html! { ctx, rcv,
            <div class="navigation-menu-content" id=[panel_id]
                 role="region" aria-labelledby=[trigger_id]
                 data-nav-content="true"
                 data-open={c_open().then_some("")}
                 data-closed={(!c_closed()).then_some("")}
                 hidden={(!c_hidden()).then_some("")}>
                <Fragment>{content_html.clone()}</Fragment>
                <Fragment>{transition_behavior()}</Fragment>
            </div>
        });
    }

    let list_id_attr: Cow<'static, str> = Cow::Owned(list_id.clone());
    let anchor_attr: Cow<'static, str> = Cow::Owned(list_id);
    let p_open = active.clone();
    let p_closed = active.clone();
    let p_active = active.clone();
    html! { ctx, rcv,
        <nav class=[class] aria-label=[config.aria_label]>
            <ul class="navigation-menu-list" role="menubar" id=[list_id_attr]
                data-composite="true" data-orientation="horizontal">
                <Fragment>{triggers.clone()}</Fragment>
                <Fragment>{composite_behavior()}</Fragment>
            </ul>
            <div class="navigation-menu-positioner"
                 data-anchor=[anchor_attr]
                 data-prefer-side="bottom" data-prefer-align="start">
                <div class="navigation-menu-viewport"
                     data-active-value={p_active.get().unwrap_or_default()}
                     data-open={p_open.get().is_some().then_some("")}
                     data-closed={p_closed.get().is_none().then_some("")}>
                    <Fragment>{contents.clone()}</Fragment>
                    <Fragment>{scoped_script(NAV_MENU_JS)}</Fragment>
                </div>
                <Fragment>{position_behavior()}</Fragment>
            </div>
        </nav>
    }
}
