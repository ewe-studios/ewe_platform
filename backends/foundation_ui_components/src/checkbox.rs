//! # Checkbox + checkbox-group (F2 — Selection controls)
//!
//! WHY: A tri-state checkbox (checked / unchecked / indeterminate) that is a
//! real form field, plus a group whose "select-all" parent derives its state
//! from the children — the spec's showcase that `ctx.computed` replaces
//! React's derived-state plumbing (spec-42 feature 05 §F2).
//!
//! WHAT: [`CheckState`], [`checkbox`] (hidden input + indicator slot),
//! [`checkbox_group`] (the `role="group"` container), and
//! [`parent_check_state`] (the computed tri-state for a select-all parent).
//!
//! HOW: The hidden `<input type="checkbox">` is the control; its `change`
//! advances the [`CheckState`] via `ctx.callback` (3-state ≠ bool, so the
//! default setter can't map it). `aria-checked="mixed"` + `data-indeterminate`
//! carry the indeterminate state.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{ComputedGetter, Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Tri-state checkbox value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    /// Checked.
    Checked,
    /// Unchecked.
    #[default]
    Unchecked,
    /// Mixed (e.g. a select-all parent with some children checked).
    Indeterminate,
}

impl CheckState {
    /// The `aria-checked` token (`true`/`false`/`mixed`).
    #[must_use]
    pub fn aria(self) -> &'static str {
        match self {
            CheckState::Checked => "true",
            CheckState::Unchecked => "false",
            CheckState::Indeterminate => "mixed",
        }
    }
}

/// Static config for a checkbox. Text fields are `Cow<'static, str>`.
pub struct CheckboxConfig {
    /// Submitted field name.
    pub name: Cow<'static, str>,
    /// Identity within a group / submitted value.
    pub value: Cow<'static, str>,
    /// Disabled.
    pub disabled: bool,
    /// `id` for the hidden input (label `for` target).
    pub id: Option<Cow<'static, str>>,
    /// Class override for the root (default: `"checkbox"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for CheckboxConfig {
    fn default() -> Self {
        Self {
            name: Cow::Borrowed(""),
            value: Cow::Borrowed("on"),
            disabled: false,
            id: None,
            class: None,
        }
    }
}

/// Slots for the checkbox.
pub struct CheckboxSlots {
    /// Indicator content (tick/dash); visible via `[data-checked]` CSS.
    pub indicator: Option<Slot>,
}

impl Default for CheckboxSlots {
    fn default() -> Self {
        Self { indicator: None }
    }
}

/// Checkbox component — hidden input + indicator, tri-state.
///
/// Signal: `state` ([`CheckState`]); activation advances it (indeterminate →
/// checked, checked → unchecked, unchecked → checked). Data attrs (on root):
/// `data-checked`/`data-unchecked`/`data-indeterminate`, `data-disabled`.
#[must_use]
pub fn checkbox(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: CheckboxConfig,
    state: &SignalGetter<CheckState>,
    set: SignalSetter<CheckState>,
    slots: CheckboxSlots,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("checkbox"));
    let indicator = slots.indicator.map(|slot| slot.render(ctx, rcv));

    let advance = {
        let state = state.clone();
        ctx.callback(move |_| {
            let next = match state.get() {
                CheckState::Checked => CheckState::Unchecked,
                CheckState::Unchecked | CheckState::Indeterminate => CheckState::Checked,
            };
            set.set(next);
        })
    };

    let s_checked = state.clone();
    let s_unchecked = state.clone();
    let s_indet = state.clone();
    let s_aria = state.clone();
    let s_input = state.clone();
    html! { ctx, rcv,
        <span class=[class]
              data-checked={(s_checked.get() == CheckState::Checked).then_some("")}
              data-unchecked={(s_unchecked.get() == CheckState::Unchecked).then_some("")}
              data-indeterminate={(s_indet.get() == CheckState::Indeterminate).then_some("")}
              data-disabled=[config.disabled.then_some("")]>
            <input type="checkbox" class="checkbox-input"
                   id=[config.id]
                   name=[config.name]
                   value=[config.value]
                   aria-checked={s_aria.get().aria()}
                   checked={(s_input.get() == CheckState::Checked).then_some("")}
                   disabled=[config.disabled.then_some("")]
                   primal:onchange={advance} />
            <span class="checkbox-indicator"><Fragment>{indicator.clone()}</Fragment></span>
        </span>
    }
}

/// Static config for a checkbox group.
pub struct CheckboxGroupConfig {
    /// Whether the whole group is disabled.
    pub disabled: bool,
    /// Class override (default: `"checkbox-group"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` for the group.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for CheckboxGroupConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            class: None,
            aria_label: None,
        }
    }
}

/// Checkbox group — a `role="group"` container over child checkboxes (passed as
/// rendered slots). Each child binds to membership in the shared `values`
/// signal; the optional select-all parent derives its tri-state via
/// [`parent_check_state`].
#[must_use]
pub fn checkbox_group(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: CheckboxGroupConfig,
    children: Vec<Slot>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("checkbox-group"));
    let kids: Vec<Html> = children.into_iter().map(|slot| slot.render(ctx, rcv)).collect();
    html! { ctx, rcv,
        <div class=[class] role="group"
             aria-label=[config.aria_label]
             data-disabled=[config.disabled.then_some("")]>
            <Fragment>{kids.clone()}</Fragment>
        </div>
    }
}

/// The select-all parent's tri-state, COMPUTED from the group's selection:
/// `Unchecked` when none selected, `Checked` when all of `all_values` are,
/// `Indeterminate` otherwise. This is the computed that replaces React's
/// derived-state plumbing — feed it into a parent checkbox's display.
#[must_use]
pub fn parent_check_state(
    ctx: &Context,
    values: &SignalGetter<Vec<String>>,
    all_values: Vec<String>,
) -> ComputedGetter<CheckState> {
    let values = values.clone();
    ctx.computed(move || {
        let selected = values.get();
        if selected.is_empty() {
            CheckState::Unchecked
        } else if all_values.iter().all(|v| selected.contains(v)) {
            CheckState::Checked
        } else {
            CheckState::Indeterminate
        }
    })
}
