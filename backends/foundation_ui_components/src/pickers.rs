//! # Pickers: select, combobox, autocomplete (F6 — Pickers)
//!
//! WHY: Value-from-a-list controls — `select` (button + listbox), `combobox`
//! (select + filtering text input), `autocomplete` (combobox whose value IS the
//! input text). They compose F4 positioning + the F5 virtual highlight + M6
//! field semantics (spec-42 §F6).
//!
//! WHAT: [`PickItem`] (value + label + disabled), [`select`], [`combobox`],
//! [`autocomplete`]. v1 is `String`-valued; generic `T` + `to_form_value` is a
//! documented follow-up.
//!
//! HOW: The popup is `role="listbox"`, `tabindex=0`, embedding the M5
//! [`listbox_behavior`] (virtual highlight + typeahead), M3 dismiss, M1
//! position, M7. Options are `role="option"` + `data-list-item`/`data-label`/
//! `data-selected`; selecting writes the value signal (and, for select, closes).
//! Combobox/autocomplete filter the rendered list through `<For>` keyed on the
//! query signal (`contains`, locale-lowercased); a hidden `<input>` serializes
//! the value for forms. The `role="status"` live region announces the count.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::dismiss::dismiss_behavior;
use crate::machinery::listbox::listbox_behavior;
use crate::machinery::position::position_behavior;
use crate::machinery::scroll_lock::scroll_lock_behavior;
use crate::machinery::transition::transition_behavior;
use crate::positioning::{PlacementAlign, PlacementSide};

/// One pickable item.
#[derive(Clone)]
pub struct PickItem {
    /// Submitted value (`<For>` key + serialization).
    pub value: String,
    /// Display + typeahead label.
    pub label: String,
    /// Disabled.
    pub disabled: bool,
}

impl PickItem {
    /// Convenience: value == label.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self { label: value.clone(), value, disabled: false }
    }
}

/// Static config shared by the pickers.
pub struct SelectConfig {
    /// Placeholder shown on the trigger when nothing is selected.
    pub placeholder: Cow<'static, str>,
    /// Modal (scroll-lock while open). Select default true; combobox false.
    pub modal: bool,
    /// `name` for the hidden serialization input.
    pub name: Option<Cow<'static, str>>,
    /// Preferred side.
    pub side: PlacementSide,
    /// Preferred alignment.
    pub align: PlacementAlign,
    /// Disabled.
    pub disabled: bool,
    /// Class override for the trigger/root (default `"select"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for SelectConfig {
    fn default() -> Self {
        Self {
            placeholder: Cow::Borrowed("Select…"),
            modal: true,
            name: None,
            side: PlacementSide::Bottom,
            align: PlacementAlign::Start,
            disabled: false,
            class: None,
        }
    }
}

fn matches(item: &PickItem, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    item.label.to_lowercase().contains(&query.to_lowercase())
}

/// Build a `role="option"` from a `PickItem`, wired to select-and-(maybe-)close.
fn option_html(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    item: &PickItem,
    value: &SignalGetter<Option<String>>,
    set_value: &SignalSetter<Option<String>>,
    set_open: Option<&SignalSetter<bool>>,
) -> Html {
    let id: Cow<'static, str> = Cow::Owned(alloc::format!("option-{}", item.value));
    let label_attr: Cow<'static, str> = Cow::Owned(item.label.clone());
    let label_text = item.label.clone();
    let disabled = item.disabled;

    let on_click = {
        let set_value = set_value.clone();
        let set_open = set_open.cloned();
        let v = item.value.clone();
        ctx.callback(move |_| {
            set_value.set(Some(v.clone()));
            if let Some(so) = &set_open {
                so.set(false);
            }
        })
    };
    let sel_aria = value.clone();
    let sel_data = value.clone();
    let sel_active = value.clone();
    let v_aria = item.value.clone();
    let v_data = item.value.clone();
    let v_active = item.value.clone();
    html! { ctx, rcv,
        <div class="select-option" role="option"
             id=[id] data-list-item="true" data-label=[label_attr]
             aria-selected={sel_aria.get().as_deref() == Some(v_aria.as_str())}
             data-selected={(sel_data.get().as_deref() == Some(v_data.as_str())).then_some("")}
             data-active-item={(sel_active.get().as_deref() == Some(v_active.as_str())).then_some("")}
             aria-disabled={disabled.then_some("true")}
             data-disabled={disabled.then_some("")}
             primal:onclick={on_click}>
            <span class="select-item-indicator" aria-hidden="true"></span>
            <Fragment>{Html::text(label_text)}</Fragment>
        </div>
    }
}

/// Select — a button + listbox over `Option<String>`.
#[must_use]
pub fn select(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    value: &SignalGetter<Option<String>>,
    set_value: SignalSetter<Option<String>>,
    items: Vec<PickItem>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("select"));
    let modal = config.modal;
    let id_n = ctx.allocate_id_block(1);
    let trigger_id = alloc::format!("select-trigger-{id_n}");
    let popup_id = alloc::format!("select-popup-{id_n}");

    let toggle = {
        let open = open.clone();
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(!open.get()))
    };
    let dismiss = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };

    // Options.
    let items_for_render = items.clone();
    let value_for_render = value.clone();
    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &PickItem| -> Html {
        option_html(c, r, item, &value_for_render, &set_value, Some(&set_open))
    };

    // Trigger label: selected item's label, else placeholder.
    let placeholder = config.placeholder.clone();
    let label_items = items;
    let label_value = value.clone();
    let trigger_label = move || -> String {
        match label_value.get() {
            Some(v) => label_items
                .iter()
                .find(|it| it.value == v)
                .map_or_else(|| placeholder.to_string(), |it| it.label.clone()),
            None => placeholder.to_string(),
        }
    };

    let trigger_id_attr: Cow<'static, str> = Cow::Owned(trigger_id.clone());
    let anchor_attr: Cow<'static, str> = Cow::Owned(trigger_id.clone());
    let anchor_attr_popup: Cow<'static, str> = Cow::Owned(trigger_id);
    let popup_id_attr: Cow<'static, str> = Cow::Owned(popup_id);
    let side_attr: Cow<'static, str> = Cow::Borrowed(config.side.as_str());
    let align_attr: Cow<'static, str> = Cow::Borrowed(config.align.as_str());

    let t_expanded = open.clone();
    let t_popup_open = open.clone();
    let t_placeholder = value.clone();
    let p_open = open.clone();
    let p_closed = open.clone();
    let h_value = value.clone();
    let disabled = config.disabled;
    html! { ctx, rcv,
        <div class="select-root">
            <button type="button" class=[class]
                    id=[trigger_id_attr]
                    aria-haspopup="listbox"
                    aria-expanded={t_expanded.get()}
                    aria-controls=[popup_id_attr.clone()]
                    data-popup-open={t_popup_open.get().then_some("")}
                    data-placeholder={t_placeholder.get().is_none().then_some("")}
                    disabled={disabled.then_some("")}
                    primal:onclick={toggle}>
                <span class="select-value">{trigger_label()}</span>
            </button>
            <input type="hidden" name=[config.name] value={h_value.get().unwrap_or_default()} />
            <div class="select-positioner"
                 data-anchor=[anchor_attr]
                 data-prefer-side=[side_attr]
                 data-prefer-align=[align_attr]>
                <div class="select-popup" id=[popup_id_attr]
                     role="listbox" tabindex="0"
                     data-anchor=[anchor_attr_popup]
                     data-dismiss="true"
                     data-scroll-lock=[modal.then_some("true")]
                     data-open={p_open.get().then_some("")}
                     data-closed={(!p_closed.get()).then_some("")}>
                    <For each={items_for_render.clone()} key={|it: &PickItem| it.value.clone()} render={render} />
                    <button type="button" hidden="" data-dismiss-action="true" primal:onclick={dismiss} />
                    <Fragment>{listbox_behavior()}</Fragment>
                    <Fragment>{dismiss_behavior()}</Fragment>
                    <Fragment>{transition_behavior()}</Fragment>
                    <Fragment>{modal.then(scroll_lock_behavior)}</Fragment>
                </div>
                <Fragment>{position_behavior()}</Fragment>
            </div>
        </div>
    }
}

/// Combobox — a filtering text input + listbox over `Option<String>`. The
/// `query` signal drives the rendered list (`contains`, locale-lowercased).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn combobox(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    value: &SignalGetter<Option<String>>,
    set_value: SignalSetter<Option<String>>,
    query: &SignalGetter<String>,
    set_query: SignalSetter<String>,
    items: Vec<PickItem>,
) -> Html {
    combobox_impl(ctx, rcv, config, open, set_open, value, set_value, query, set_query, items, false)
}

/// Autocomplete — a combobox whose value IS the input text (suggestions list,
/// no separate value signal). The input is the form field.
#[must_use]
pub fn autocomplete(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    query: &SignalGetter<String>,
    set_query: SignalSetter<String>,
    items: Vec<PickItem>,
) -> Html {
    // value ≡ query: selecting writes the query, no hidden value input.
    let (value, set_value) = ctx.signal::<Option<String>>(None);
    combobox_impl(ctx, rcv, config, open, set_open, &value, set_value, query, set_query, items, true)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn combobox_impl(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    value: &SignalGetter<Option<String>>,
    set_value: SignalSetter<Option<String>>,
    query: &SignalGetter<String>,
    set_query: SignalSetter<String>,
    items: Vec<PickItem>,
    inline_value: bool,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("combobox"));
    let id_n = ctx.allocate_id_block(1);
    let input_id = alloc::format!("combobox-input-{id_n}");
    let popup_id = alloc::format!("combobox-popup-{id_n}");

    let dismiss = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };
    // Typing opens the list and updates the query (+ value when autocomplete).
    let on_input = {
        let set_query = set_query.clone();
        let set_open = set_open.clone();
        let set_value = set_value.clone();
        ctx.callback(move |data| {
            let text = data.value.clone().unwrap_or_default();
            if inline_value {
                set_value.set(Some(text.clone()));
            }
            set_query.set(text);
            set_open.set(true);
        })
    };

    // Filtered options, recomputed when the query changes.
    let all_items = items.clone();
    let query_for_filter = query.clone();
    let value_for_render = value.clone();
    let set_value_render = set_value.clone();
    let set_open_render = set_open.clone();
    let set_query_render = set_query.clone();
    let inline = inline_value;
    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &PickItem| -> Html {
        // Selecting fills the input (query) and, for select-mode, the value.
        let id: Cow<'static, str> = Cow::Owned(alloc::format!("option-{}", item.value));
        let label_attr: Cow<'static, str> = Cow::Owned(item.label.clone());
        let label_text = item.label.clone();
        let disabled = item.disabled;
        let on_click = {
            let set_value = set_value_render.clone();
            let set_open = set_open_render.clone();
            let set_query = set_query_render.clone();
            let v = item.value.clone();
            let lbl = item.label.clone();
            c.callback(move |_| {
                set_value.set(Some(v.clone()));
                set_query.set(if inline { v.clone() } else { lbl.clone() });
                set_open.set(false);
            })
        };
        let sel = value_for_render.clone();
        let sel2 = value_for_render.clone();
        let v1 = item.value.clone();
        let v2 = item.value.clone();
        html! { c, r,
            <div class="combobox-option" role="option"
                 id=[id] data-list-item="true" data-label=[label_attr]
                 aria-selected={sel.get().as_deref() == Some(v1.as_str())}
                 data-selected={(sel2.get().as_deref() == Some(v2.as_str())).then_some("")}
                 aria-disabled={disabled.then_some("true")}
                 data-disabled={disabled.then_some("")}
                 primal:onclick={on_click}>
                <Fragment>{Html::text(label_text)}</Fragment>
            </div>
        }
    };

    let filtered = {
        let all_items = all_items.clone();
        let q = query_for_filter.clone();
        move || -> Vec<PickItem> {
            let query = q.get();
            all_items.iter().filter(|it| matches(it, &query)).cloned().collect()
        }
    };
    let count = {
        let f = filtered.clone();
        move || f().len()
    };

    let input_id_attr: Cow<'static, str> = Cow::Owned(input_id.clone());
    let anchor_attr: Cow<'static, str> = Cow::Owned(input_id.clone());
    let anchor_attr_popup: Cow<'static, str> = Cow::Owned(input_id);
    let popup_id_attr: Cow<'static, str> = Cow::Owned(popup_id);
    let side_attr: Cow<'static, str> = Cow::Borrowed(config.side.as_str());
    let align_attr: Cow<'static, str> = Cow::Borrowed(config.align.as_str());
    let placeholder = config.placeholder.clone();

    let i_expanded = open.clone();
    let i_popup_open = open.clone();
    let i_popup_open2 = open.clone();
    let i_query = query.clone();
    let p_open = open.clone();
    let p_closed = open.clone();
    let p_empty = { let c = count.clone(); move || c() == 0 };
    let s_count = count.clone();
    let disabled = config.disabled;
    html! { ctx, rcv,
        <div class=[class] role="combobox"
             aria-expanded={i_expanded.get()}
             aria-haspopup="listbox"
             aria-controls=[popup_id_attr.clone()]>
            <input class="combobox-input" type="text"
                   id=[input_id_attr]
                   role="combobox"
                   aria-autocomplete="list"
                   aria-expanded={i_popup_open.get()}
                   placeholder=[placeholder]
                   value={i_query.get()}
                   data-popup-open={i_popup_open2.get().then_some("")}
                   disabled={disabled.then_some("")}
                   primal:oninput={on_input} />
            <div class="combobox-positioner"
                 data-anchor=[anchor_attr]
                 data-prefer-side=[side_attr]
                 data-prefer-align=[align_attr]>
                <div class="combobox-popup" id=[popup_id_attr]
                     role="listbox" tabindex="0"
                     data-anchor=[anchor_attr_popup]
                     data-dismiss="true"
                     data-empty={p_empty().then_some("")}
                     data-open={p_open.get().then_some("")}
                     data-closed={(!p_closed.get()).then_some("")}>
                    <For each={filtered()} key={|it: &PickItem| it.value.clone()} render={render} />
                    <button type="button" hidden="" data-dismiss-action="true" primal:onclick={dismiss} />
                    <Fragment>{listbox_behavior()}</Fragment>
                    <Fragment>{dismiss_behavior()}</Fragment>
                    <Fragment>{transition_behavior()}</Fragment>
                </div>
                <Fragment>{position_behavior()}</Fragment>
            </div>
            <div class="combobox-status" role="status" aria-live="polite">
                {alloc::format!("{} results", s_count())}
            </div>
        </div>
    }
}
