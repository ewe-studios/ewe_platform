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
use crate::machinery::scoped_script;
use crate::machinery::scroll_lock::scroll_lock_behavior;
use crate::machinery::transition::transition_behavior;
use crate::positioning::{PlacementAlign, PlacementSide};

/// Select item-alignment positioning (base-ui `alignItemWithTrigger`): the popup
/// is `fixed` so the SELECTED item sits over the trigger; if it overflows the
/// viewport it clamps with margins and scrolls the selected item onto the
/// trigger line, with ScrollUp/Down arrows.
pub const SELECT_ALIGN_JS: &str = r#"function(scope){
  var pos = scope.parent();
  if (!pos || pos.__selAlign) return; pos.__selAlign = true;
  var doc = pos.ownerDocument || document;
  var win = doc.defaultView || window;
  var popup = pos.querySelector('[data-select-popup]');
  if (!popup) return;
  var anchorId = pos.getAttribute('data-anchor');
  function trigger(){ return anchorId ? doc.getElementById(anchorId) : pos.previousElementSibling; }
  var up = pos.querySelector('[data-select-scroll="up"]'), down = pos.querySelector('[data-select-scroll="down"]');
  var margin = 8;
  function updateArrows(){
    var max = popup.scrollHeight - popup.clientHeight;
    if (up) up.style.visibility = popup.scrollTop > 1 ? 'visible' : 'hidden';
    if (down) down.style.visibility = popup.scrollTop < max - 1 ? 'visible' : 'hidden';
  }
  function align(){
    if (pos.getAttribute('data-align-item') !== 'true' || !popup.hasAttribute('data-open')) return;
    var trg = trigger(); if (!trg) return;
    var r = trg.getBoundingClientRect();
    var sel = popup.querySelector('[data-selected]') || popup.querySelector('[role="option"]');
    if (!sel) return;
    var vh = doc.documentElement.clientHeight;
    pos.style.position = 'fixed';
    pos.style.left = r.left + 'px';
    pos.style.minWidth = r.width + 'px';
    pos.style.height = '';
    popup.scrollTop = 0;
    var selOffset = sel.offsetTop, selH = sel.offsetHeight, natural = popup.scrollHeight;
    var maxH = vh - 2 * margin, center = r.top + r.height / 2;
    var top = center - (selOffset + selH / 2);
    if (natural <= maxH) {
      if (top < margin) top = margin;
      if (top + natural > vh - margin) top = vh - margin - natural;
      pos.style.top = top + 'px';
      pos.style.height = natural + 'px';
      popup.scrollTop = 0;
    } else {
      pos.style.top = margin + 'px';
      pos.style.height = maxH + 'px';
      var st = (selOffset + selH / 2) - (center - margin);
      popup.scrollTop = Math.max(0, Math.min(st, popup.scrollHeight - popup.clientHeight));
    }
    updateArrows();
  }
  function holdScroll(arrow, dir){
    if (!arrow) return;
    scope.addEvent(arrow, 'pointerenter', function(){
      var iv = win.setInterval(function(){ popup.scrollTop += dir * popup.clientHeight * 0.25; updateArrows(); }, 50);
      var stop = function(){ win.clearInterval(iv); arrow.removeEventListener('pointerleave', stop); };
      arrow.addEventListener('pointerleave', stop);
    });
  }
  holdScroll(up, -1); holdScroll(down, 1);
  scope.addEvent(popup, 'scroll', updateArrows);
  var mo = new MutationObserver(function(){ if (popup.hasAttribute('data-open')) win.requestAnimationFrame(align); });
  mo.observe(popup, { attributes: true, attributeFilter: ['data-open'] });
  if (popup.hasAttribute('data-open')) win.requestAnimationFrame(align);
}"#;

/// One pickable item over a value of type `T`.
#[derive(Clone)]
pub struct PickItem<T> {
    /// Submitted value.
    pub value: T,
    /// Display + typeahead label.
    pub label: String,
    /// Disabled.
    pub disabled: bool,
}

impl PickItem<String> {
    /// Convenience for string-valued pickers: value == label.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self { label: value.clone(), value, disabled: false }
    }
}

impl<T> PickItem<T> {
    /// A pickable item with a distinct value and display label.
    pub fn with_label(value: T, label: impl Into<String>) -> Self {
        Self { value, label: label.into(), disabled: false }
    }
}

/// Static config shared by the pickers. `to_form_value` serializes `T` for the
/// hidden form input + option identity (base-ui `itemToStringValue`).
pub struct SelectConfig<T = String> {
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
    /// Align the selected item over the trigger (macOS-style). When false,
    /// anchored-below (M1). Combobox/autocomplete ignore this.
    pub align_item_with_trigger: bool,
    /// Disabled.
    pub disabled: bool,
    /// Serialize a value to its submitted/identity string.
    pub to_form_value: fn(&T) -> String,
    /// Class override for the trigger/root (default `"select"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for SelectConfig<String> {
    fn default() -> Self {
        Self::with_form_value(|s: &String| s.clone())
    }
}

impl<T> SelectConfig<T> {
    /// A config for a `T`-valued picker; supply the value serializer.
    #[must_use]
    pub fn with_form_value(to_form_value: fn(&T) -> String) -> Self {
        Self {
            placeholder: Cow::Borrowed("Select…"),
            modal: true,
            name: None,
            side: PlacementSide::Bottom,
            align: PlacementAlign::Start,
            align_item_with_trigger: true,
            disabled: false,
            to_form_value,
            class: None,
        }
    }
}

fn matches<T>(item: &PickItem<T>, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    item.label.to_lowercase().contains(&query.to_lowercase())
}

/// Build a `role="option"` from a `PickItem<T>`, wired to select-and-(maybe-)close.
fn option_html<T: Clone + PartialEq + 'static>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    item: &PickItem<T>,
    to_form_value: fn(&T) -> String,
    value: &SignalGetter<Option<T>>,
    set_value: &SignalSetter<Option<T>>,
    set_open: Option<&SignalSetter<bool>>,
) -> Html {
    let id: Cow<'static, str> = Cow::Owned(alloc::format!("option-{}", to_form_value(&item.value)));
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
             aria-selected={sel_aria.get().as_ref() == Some(&v_aria)}
             data-selected={(sel_data.get().as_ref() == Some(&v_data)).then_some("")}
             data-active-item={(sel_active.get().as_ref() == Some(&v_active)).then_some("")}
             aria-disabled={disabled.then_some("true")}
             data-disabled={disabled.then_some("")}
             primal:onclick={on_click}>
            <span class="select-item-indicator" aria-hidden="true"></span>
            <Fragment>{Html::text(label_text)}</Fragment>
        </div>
    }
}

/// Select — a button + listbox over `Option<T>`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn select<T: Clone + PartialEq + 'static>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig<T>,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    value: &SignalGetter<Option<T>>,
    set_value: SignalSetter<Option<T>>,
    items: Vec<PickItem<T>>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("select"));
    let modal = config.modal;
    let tfv = config.to_form_value;
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
    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &PickItem<T>| -> Html {
        option_html(c, r, item, tfv, &value_for_render, &set_value, Some(&set_open))
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

    let align_item = config.align_item_with_trigger;
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
            <input type="hidden" name=[config.name]
                   value={h_value.get().map(|v| tfv(&v)).unwrap_or_default()} />
            <div class="select-positioner"
                 data-anchor=[anchor_attr]
                 data-align-item=[align_item.then_some("true")]
                 data-prefer-side=[side_attr]
                 data-prefer-align=[align_attr]>
                <div class="select-scroll-arrow" data-select-scroll="up" aria-hidden="true"></div>
                <div class="select-popup" id=[popup_id_attr]
                     data-select-popup="true"
                     role="listbox" tabindex="0"
                     data-anchor=[anchor_attr_popup]
                     data-dismiss="true"
                     data-scroll-lock=[modal.then_some("true")]
                     data-open={p_open.get().then_some("")}
                     data-closed={(!p_closed.get()).then_some("")}>
                    <For each={items_for_render.clone()} key={move |it: &PickItem<T>| tfv(&it.value)} render={render} />
                    <button type="button" hidden="" data-dismiss-action="true" primal:onclick={dismiss} />
                    <Fragment>{listbox_behavior()}</Fragment>
                    <Fragment>{dismiss_behavior()}</Fragment>
                    <Fragment>{transition_behavior()}</Fragment>
                    <Fragment>{modal.then(scroll_lock_behavior)}</Fragment>
                </div>
                <div class="select-scroll-arrow" data-select-scroll="down" aria-hidden="true"></div>
                // Item-alignment mode positions itself; otherwise M1 anchors below.
                <Fragment>{if align_item { scoped_script(SELECT_ALIGN_JS) } else { position_behavior() }}</Fragment>
            </div>
        </div>
    }
}

/// Combobox — a filtering text input + listbox over `Option<String>`. The
/// `query` signal drives the rendered list (`contains`, locale-lowercased).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn combobox<T: Clone + PartialEq + 'static>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig<T>,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    value: &SignalGetter<Option<T>>,
    set_value: SignalSetter<Option<T>>,
    query: &SignalGetter<String>,
    set_query: SignalSetter<String>,
    items: Vec<PickItem<T>>,
) -> Html {
    combobox_impl(ctx, rcv, config, open, set_open, value, set_value, query, set_query, items, false)
}

/// Autocomplete — a combobox whose value IS the input text (suggestions list,
/// no separate value signal). The input is the form field. String-valued.
#[must_use]
pub fn autocomplete(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig<String>,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    query: &SignalGetter<String>,
    set_query: SignalSetter<String>,
    items: Vec<PickItem<String>>,
) -> Html {
    // value ≡ query: selecting writes the query, no hidden value input.
    let (value, set_value) = ctx.signal::<Option<String>>(None);
    combobox_impl(ctx, rcv, config, open, set_open, &value, set_value, query, set_query, items, true)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn combobox_impl<T: Clone + PartialEq + 'static>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SelectConfig<T>,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    value: &SignalGetter<Option<T>>,
    set_value: SignalSetter<Option<T>>,
    query: &SignalGetter<String>,
    set_query: SignalSetter<String>,
    items: Vec<PickItem<T>>,
    inline_value: bool,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("combobox"));
    let tfv = config.to_form_value;
    let id_n = ctx.allocate_id_block(1);
    let input_id = alloc::format!("combobox-input-{id_n}");
    let popup_id = alloc::format!("combobox-popup-{id_n}");

    let dismiss = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };
    // Typing opens the list and updates the query. (Autocomplete's value ≡ the
    // input text: it tracks via selection; the input itself is the form field.)
    let on_input = {
        let set_query = set_query.clone();
        let set_open = set_open.clone();
        ctx.callback(move |data| {
            set_query.set(data.value.clone().unwrap_or_default());
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
    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &PickItem<T>| -> Html {
        // Selecting fills the input (query) and, for select-mode, the value.
        let id: Cow<'static, str> = Cow::Owned(alloc::format!("option-{}", tfv(&item.value)));
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
                set_query.set(if inline { tfv(&v) } else { lbl.clone() });
                set_value.set(Some(v.clone()));
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
                 aria-selected={sel.get().as_ref() == Some(&v1)}
                 data-selected={(sel2.get().as_ref() == Some(&v2)).then_some("")}
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
        move || -> Vec<PickItem<T>> {
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
                    <For each={filtered()} key={move |it: &PickItem<T>| tfv(&it.value)} render={render} />
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
