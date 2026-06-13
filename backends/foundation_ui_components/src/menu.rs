//! # Menu / context-menu / menubar (F5 — Menus)
//!
//! WHY: A popup of commands with the full WAI-ARIA menu keyboard contract —
//! virtual highlight, typeahead, checkbox/radio items, groups, submenus
//! (spec-42 §F5). Items are DATA (`MenuEntry`), not markup, because the
//! component owns the roles/highlight/typeahead ("don't make the caller wire
//! ARIA").
//!
//! WHAT: [`menu`] (trigger + popup over an `(open, set_open)` signal),
//! [`context_menu`] (a right-clickable surface opening a menu at the pointer),
//! [`menubar`] (a row of menus with roving triggers + open-intent), and the
//! [`MenuEntry`] enum.
//!
//! HOW: The popup is `role="menu"`, `tabindex=0`, embedding the M5
//! [`listbox_behavior`] (virtual highlight + typeahead), M3
//! [`dismiss_behavior`], M1 [`position_behavior`] and M7. Each entry renders to
//! the right role + `data-list-item` + `data-label`; selecting runs the
//! caller's action and (per `close_on_click`) flips `open`. Submenus recurse
//! the same renderer behind a nested open signal (click/→ to open; the
//! safe-polygon hover refinement is the documented M3 follow-up).

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::dismiss::dismiss_behavior;
use crate::machinery::listbox::listbox_behavior;
use crate::machinery::position::position_behavior;
use crate::machinery::scoped_script;
use crate::machinery::scroll_lock::scroll_lock_behavior;
use crate::machinery::transition::transition_behavior;
use crate::positioning::{PlacementAlign, PlacementSide};

/// An action run when a menu item is selected.
pub type MenuAction = Box<dyn Fn()>;

/// Per-item config.
pub struct ItemConfig {
    /// Typeahead label override (falls back to the slot's text).
    pub label: Option<Cow<'static, str>>,
    /// Disabled.
    pub disabled: bool,
    /// Close the menu on activate (default true; checkbox/radio override false).
    pub close_on_click: bool,
}

impl Default for ItemConfig {
    fn default() -> Self {
        Self { label: None, disabled: false, close_on_click: true }
    }
}

/// One radio option within a [`MenuEntry::Radio`].
pub struct RadioEntry {
    /// Submitted value.
    pub value: String,
    /// Item config.
    pub cfg: ItemConfig,
    /// Label content.
    pub label: Slot,
}

/// A menu entry — data the component turns into correctly-roled markup.
pub enum MenuEntry {
    /// A command item.
    Item {
        /// Item config.
        cfg: ItemConfig,
        /// Label content.
        label: Slot,
        /// Action run on select.
        on_select: MenuAction,
    },
    /// A checkbox item (`role="menuitemcheckbox"`), stays open on toggle.
    Checkbox {
        /// Item config.
        cfg: ItemConfig,
        /// Label content.
        label: Slot,
        /// Checked state.
        checked: SignalGetter<bool>,
        /// Checked setter.
        set_checked: SignalSetter<bool>,
    },
    /// A radio group (`role="menuitemradio"` items over one value).
    Radio {
        /// Selected value.
        value: SignalGetter<String>,
        /// Value setter.
        set_value: SignalSetter<String>,
        /// Options.
        items: Vec<RadioEntry>,
    },
    /// A link item — a real `<a>` that closes the menu on activate.
    Link {
        /// Item config.
        cfg: ItemConfig,
        /// Label content.
        label: Slot,
        /// `href`.
        href: Cow<'static, str>,
    },
    /// A labelled group of entries.
    Group {
        /// Group label content.
        label: Slot,
        /// Member entries.
        entries: Vec<MenuEntry>,
    },
    /// A submenu — a nested menu opened from a trigger item.
    Submenu {
        /// Trigger item config.
        cfg: ItemConfig,
        /// Trigger label content.
        label: Slot,
        /// Submenu entries.
        entries: Vec<MenuEntry>,
    },
    /// A separator.
    Separator,
}

/// Static config for a menu.
pub struct MenuConfig {
    /// Preferred side.
    pub side: PlacementSide,
    /// Preferred alignment.
    pub align: PlacementAlign,
    /// Modal (scroll-lock + pointer-inert while open).
    pub modal: bool,
    /// Whether roving/highlight loops.
    pub loop_focus: bool,
    /// Class override for the popup (default `"menu"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for MenuConfig {
    fn default() -> Self {
        Self {
            side: PlacementSide::Bottom,
            align: PlacementAlign::Start,
            modal: true,
            loop_focus: true,
            class: None,
        }
    }
}

/// Menu component — a trigger that opens a command popup.
#[must_use]
pub fn menu(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: MenuConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    trigger: Vec<Slot>,
    items: Vec<MenuEntry>,
) -> Html {
    let id_n = ctx.allocate_id_block(1);
    let trigger_id = alloc::format!("menu-trigger-{id_n}");
    let popup_id = alloc::format!("menu-popup-{id_n}");
    let trigger_html: Vec<Html> = trigger.into_iter().map(|s| s.render(ctx, rcv)).collect();

    let toggle = {
        let open = open.clone();
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(!open.get()))
    };

    let popup = menu_popup(
        ctx, rcv, &config, open, &set_open, &trigger_id, &popup_id, items,
    );

    let trigger_id_attr: Cow<'static, str> = Cow::Owned(trigger_id);
    let popup_id_attr: Cow<'static, str> = Cow::Owned(popup_id);
    let t_expanded = open.clone();
    let t_popup_open = open.clone();
    html! { ctx, rcv,
        <div class="menu-root">
            <button type="button" class="menu-trigger"
                    id=[trigger_id_attr]
                    aria-haspopup="menu"
                    aria-expanded={t_expanded.get()}
                    aria-controls=[popup_id_attr]
                    data-popup-open={t_popup_open.get().then_some("")}
                    primal:onclick={toggle}>
                <Fragment>{trigger_html.clone()}</Fragment>
            </button>
            <Fragment>{popup}</Fragment>
        </div>
    }
}

/// Build the positioner + popup (shared by menu/context-menu/submenu).
#[allow(clippy::too_many_arguments)]
fn menu_popup(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: &MenuConfig,
    open: &SignalGetter<bool>,
    set_open: &SignalSetter<bool>,
    anchor_id: &str,
    popup_id: &str,
    items: Vec<MenuEntry>,
) -> Html {
    let class = config.class.clone().unwrap_or(Cow::Borrowed("menu"));
    let modal = config.modal;
    let loop_focus = config.loop_focus;
    let counter = Cell::new(0u32);
    let entries = render_entries(ctx, rcv, set_open, items, popup_id, &counter);

    let dismiss = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };

    let anchor_attr: Cow<'static, str> = Cow::Owned(anchor_id.to_string());
    let anchor_attr_popup: Cow<'static, str> = Cow::Owned(anchor_id.to_string());
    let popup_id_attr: Cow<'static, str> = Cow::Owned(popup_id.to_string());
    let side_attr: Cow<'static, str> = Cow::Borrowed(config.side.as_str());
    let align_attr: Cow<'static, str> = Cow::Borrowed(config.align.as_str());
    let p_open = open.clone();
    let p_closed = open.clone();

    html! { ctx, rcv,
        <div class="menu-positioner"
             data-anchor=[anchor_attr]
             data-prefer-side=[side_attr]
             data-prefer-align=[align_attr]>
            <div class=[class] id=[popup_id_attr]
                 role="menu" tabindex="0"
                 data-anchor=[anchor_attr_popup]
                 data-dismiss="true"
                 data-scroll-lock=[modal.then_some("true")]
                 data-loop=[(!loop_focus).then_some("false")]
                 data-open={p_open.get().then_some("")}
                 data-closed={(!p_closed.get()).then_some("")}>
                <Fragment>{entries}</Fragment>
                <button type="button" hidden="" data-dismiss-action="true" primal:onclick={dismiss} />
                <Fragment>{listbox_behavior()}</Fragment>
                <Fragment>{dismiss_behavior()}</Fragment>
                <Fragment>{transition_behavior()}</Fragment>
                <Fragment>{modal.then(scroll_lock_behavior)}</Fragment>
            </div>
            <Fragment>{position_behavior()}</Fragment>
        </div>
    }
}

/// Recursively render entries to `Html`. `counter` gives stable per-item ids.
fn render_entries(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    set_open: &SignalSetter<bool>,
    entries: Vec<MenuEntry>,
    prefix: &str,
    counter: &Cell<u32>,
) -> Vec<Html> {
    let mut out = Vec::new();
    for entry in entries {
        out.push(render_entry(ctx, rcv, set_open, entry, prefix, counter));
    }
    out
}

fn next_id(prefix: &str, counter: &Cell<u32>) -> String {
    let n = counter.get();
    counter.set(n + 1);
    alloc::format!("{prefix}-i{n}")
}

#[allow(clippy::too_many_lines)]
fn render_entry(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    set_open: &SignalSetter<bool>,
    entry: MenuEntry,
    prefix: &str,
    counter: &Cell<u32>,
) -> Html {
    match entry {
        MenuEntry::Separator => html! { ctx, rcv,
            <div class="menu-separator" role="separator"></div>
        },
        MenuEntry::Item { cfg, label, on_select } => {
            let id: Cow<'static, str> = Cow::Owned(next_id(prefix, counter));
            let label_html = label.render(ctx, rcv);
            let label_text = cfg.label.clone();
            let disabled = cfg.disabled;
            let close = cfg.close_on_click;
            // The action box is owned by the entry; move it into the callback.
            let on_click = {
                let set_open = set_open.clone();
                ctx.callback(move |_| {
                    on_select();
                    if close {
                        set_open.set(false);
                    }
                })
            };
            html! { ctx, rcv,
                <div class="menu-item" role="menuitem"
                     id=[id] data-list-item="true" tabindex="-1"
                     data-label=[label_text]
                     aria-disabled={disabled.then_some("true")}
                     data-disabled={disabled.then_some("")}
                     primal:onclick={on_click}>
                    <Fragment>{label_html.clone()}</Fragment>
                </div>
            }
        }
        MenuEntry::Checkbox { cfg, label, checked, set_checked } => {
            let id: Cow<'static, str> = Cow::Owned(next_id(prefix, counter));
            let label_html = label.render(ctx, rcv);
            let label_text = cfg.label.clone();
            let disabled = cfg.disabled;
            let on_click = {
                let checked = checked.clone();
                let set = set_checked.clone();
                ctx.callback(move |_| set.set(!checked.get()))
            };
            let c_aria = checked.clone();
            let c_data = checked.clone();
            let c_un = checked.clone();
            html! { ctx, rcv,
                <div class="menu-item" role="menuitemcheckbox"
                     id=[id] data-list-item="true" tabindex="-1"
                     data-label=[label_text]
                     aria-checked={c_aria.get()}
                     data-checked={c_data.get().then_some("")}
                     data-unchecked={(!c_un.get()).then_some("")}
                     aria-disabled={disabled.then_some("true")}
                     data-disabled={disabled.then_some("")}
                     primal:onclick={on_click}>
                    <span class="menu-item-indicator" aria-hidden="true"></span>
                    <Fragment>{label_html.clone()}</Fragment>
                </div>
            }
        }
        MenuEntry::Radio { value, set_value, items } => {
            let mut children = Vec::new();
            for opt in items {
                let id: Cow<'static, str> = Cow::Owned(next_id(prefix, counter));
                let label_html = opt.label.render(ctx, rcv);
                let label_text = opt.cfg.label.clone();
                let disabled = opt.cfg.disabled;
                let opt_value = opt.value.clone();
                let on_click = {
                    let set = set_value.clone();
                    let v = opt.value.clone();
                    ctx.callback(move |_| set.set(v.clone()))
                };
                let checked = value.clone();
                let cv = opt_value.clone();
                let checked2 = value.clone();
                let cv2 = opt_value;
                children.push(html! { ctx, rcv,
                    <div class="menu-item" role="menuitemradio"
                         id=[id] data-list-item="true" tabindex="-1"
                         data-label=[label_text]
                         aria-checked={checked.get() == cv}
                         data-checked={(checked2.get() == cv2).then_some("")}
                         aria-disabled={disabled.then_some("true")}
                         data-disabled={disabled.then_some("")}
                         primal:onclick={on_click}>
                        <span class="menu-item-indicator" aria-hidden="true"></span>
                        <Fragment>{label_html.clone()}</Fragment>
                    </div>
                });
            }
            html! { ctx, rcv, <div class="menu-radio-group" role="group"><Fragment>{children.clone()}</Fragment></div> }
        }
        MenuEntry::Link { cfg, label, href } => {
            let id: Cow<'static, str> = Cow::Owned(next_id(prefix, counter));
            let label_html = label.render(ctx, rcv);
            let label_text = cfg.label.clone();
            let disabled = cfg.disabled;
            let href = href.clone();
            let on_click = {
                let set_open = set_open.clone();
                ctx.callback(move |_| set_open.set(false))
            };
            html! { ctx, rcv,
                <a class="menu-item" role="menuitem"
                   id=[id] data-list-item="true" tabindex="-1"
                   href=[href]
                   data-label=[label_text]
                   aria-disabled={disabled.then_some("true")}
                   data-disabled={disabled.then_some("")}
                   primal:onclick={on_click}>
                    <Fragment>{label_html.clone()}</Fragment>
                </a>
            }
        }
        MenuEntry::Group { label, entries } => {
            let label_id: Cow<'static, str> = Cow::Owned(next_id(prefix, counter));
            let label_id_ref = label_id.clone();
            let label_html = label.render(ctx, rcv);
            let members = render_entries(ctx, rcv, set_open, entries, prefix, counter);
            html! { ctx, rcv,
                <div class="menu-group" role="group" aria-labelledby=[label_id_ref]>
                    <div class="menu-group-label" id=[label_id] role="presentation">
                        <Fragment>{label_html.clone()}</Fragment>
                    </div>
                    <Fragment>{members}</Fragment>
                </div>
            }
        }
        MenuEntry::Submenu { cfg, label, entries } => {
            // A submenu is a nested menu opened from a trigger item (click / →).
            let (sub_open, set_sub_open) = ctx.signal(false);
            let trigger_id = next_id(prefix, counter);
            let popup_id = alloc::format!("{trigger_id}-sub");
            let label_html = label.render(ctx, rcv);
            let label_text = cfg.label.clone();
            let disabled = cfg.disabled;
            let sub_counter = Cell::new(0u32);
            let members = render_entries(ctx, rcv, &set_sub_open, entries, &popup_id, &sub_counter);

            let toggle = {
                let sub_open = sub_open.clone();
                let set_sub_open = set_sub_open.clone();
                ctx.callback(move |_| set_sub_open.set(!sub_open.get()))
            };
            let dismiss = {
                let set_sub_open = set_sub_open.clone();
                ctx.callback(move |_| set_sub_open.set(false))
            };
            let trigger_id_attr: Cow<'static, str> = Cow::Owned(trigger_id.clone());
            let anchor_attr: Cow<'static, str> = Cow::Owned(trigger_id);
            let popup_id_attr: Cow<'static, str> = Cow::Owned(popup_id);
            let s_open = sub_open.clone();
            let s_closed = sub_open.clone();
            let t_expanded = sub_open.clone();
            html! { ctx, rcv,
                <div class="menu-submenu">
                    <div class="menu-item menu-submenu-trigger" role="menuitem"
                         id=[trigger_id_attr] data-list-item="true" tabindex="-1"
                         aria-haspopup="menu"
                         aria-expanded={t_expanded.get()}
                         data-label=[label_text]
                         aria-disabled={disabled.then_some("true")}
                         data-disabled={disabled.then_some("")}
                         primal:onclick={toggle}>
                        <Fragment>{label_html.clone()}</Fragment>
                        <span class="menu-submenu-arrow" aria-hidden="true">{"›"}</span>
                    </div>
                    <div class="menu-positioner"
                         data-anchor=[anchor_attr]
                         data-prefer-side="right" data-prefer-align="start">
                        <div class="menu" id=[popup_id_attr]
                             role="menu" tabindex="0"
                             data-dismiss="true"
                             data-open={s_open.get().then_some("")}
                             data-closed={(!s_closed.get()).then_some("")}>
                            <Fragment>{members}</Fragment>
                            <button type="button" hidden="" data-dismiss-action="true" primal:onclick={dismiss} />
                            <Fragment>{listbox_behavior()}</Fragment>
                            <Fragment>{dismiss_behavior()}</Fragment>
                            <Fragment>{transition_behavior()}</Fragment>
                        </div>
                        <Fragment>{position_behavior()}</Fragment>
                    </div>
                </div>
            }
        }
    }
}
/// Right-click handler: prevent the native menu, open ours.
const CONTEXT_MENU_JS: &str = r#"function(scope){
  var root = scope.parent();
  if (!root || root.__cm) return; root.__cm = true;
  var surface = root.querySelector('[data-cm-surface]');
  var openBtn = root.querySelector('[data-cm-open]');
  if (!surface) return;
  scope.addEvent(surface, 'contextmenu', function(e){ e.preventDefault(); if (openBtn) openBtn.click(); });
}"#;

/// Context-menu — a right-clickable surface opening a menu anchored to it.
/// (Exact-pointer placement + touch long-press are documented M1/M3 follow-ups;
/// v1 anchors to the surface.)
#[must_use]
pub fn context_menu(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: MenuConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    surface: Vec<Slot>,
    items: Vec<MenuEntry>,
) -> Html {
    let id_n = ctx.allocate_id_block(1);
    let surface_id = alloc::format!("context-menu-surface-{id_n}");
    let popup_id = alloc::format!("context-menu-popup-{id_n}");
    let surface_html: Vec<Html> = surface.into_iter().map(|s| s.render(ctx, rcv)).collect();

    let open_action = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(true))
    };
    let popup = menu_popup(ctx, rcv, &config, open, &set_open, &surface_id, &popup_id, items);
    let surface_id_attr: Cow<'static, str> = Cow::Owned(surface_id);

    html! { ctx, rcv,
        <div class="context-menu-root">
            <div class="context-menu-surface" id=[surface_id_attr] data-cm-surface="true">
                <Fragment>{surface_html.clone()}</Fragment>
            </div>
            <button type="button" hidden="" data-cm-open="true" primal:onclick={open_action} />
            <Fragment>{popup}</Fragment>
            <Fragment>{scoped_script(CONTEXT_MENU_JS)}</Fragment>
        </div>
    }
}

/// Menubar — a horizontal row of menus. Each `(trigger, entries)` becomes a
/// menu with its own open signal. `role="menubar"`. (Cross-trigger roving +
/// hover open-intent are documented M5 follow-ups; the individual menus work.)
#[must_use]
pub fn menubar(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    menus: Vec<(Slot, Vec<MenuEntry>)>,
) -> Html {
    let rendered: Vec<Html> = menus
        .into_iter()
        .map(|(trigger, entries)| {
            let (open, set_open) = ctx.signal(false);
            menu(ctx, rcv, MenuConfig::default(), &open, set_open, vec![trigger], entries)
        })
        .collect();
    html! { ctx, rcv,
        <div class="menubar" role="menubar" aria-orientation="horizontal" data-orientation="horizontal">
            <Fragment>{rendered.clone()}</Fragment>
        </div>
    }
}
