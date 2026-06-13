//! # OTP field (F7 — Form)
//!
//! WHY: A one-time-code entry split across per-digit cells, with the typing /
//! paste / backspace choreography users expect (spec-42 §F7).
//!
//! WHAT: [`otp_field`] renders Root → `length` cell `<input>`s + a hidden
//! aggregate input. The combined code is a `String` signal.
//!
//! HOW: A scoped script wires the cells: typing advances focus, Backspace on an
//! empty cell retreats, ←/→ move, paste distributes across cells from the
//! focused one, and any change re-assembles the value into the hidden input and
//! dispatches `change` → the `set_code` setter. `autocomplete="one-time-code"`
//! on the first cell gets SMS autofill; `mask` makes the cells password-style.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::scoped_script;

/// Static config for an OTP field.
pub struct OtpConfig {
    /// Number of cells (required).
    pub length: usize,
    /// `numeric` (default) or `text` input mode.
    pub numeric: bool,
    /// Password-style cells.
    pub mask: bool,
    /// Disabled.
    pub disabled: bool,
    /// `name` for the hidden aggregate input.
    pub name: Option<Cow<'static, str>>,
    /// Class override (default `"otp-field"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for OtpConfig {
    fn default() -> Self {
        Self { length: 6, numeric: true, mask: false, disabled: false, name: None, class: None }
    }
}

/// Cell choreography: advance/retreat focus, arrows, paste distribution, and
/// re-assembly of the hidden aggregate value.
const OTP_JS: &str = r#"function(scope){
  var root = scope.parent();
  if (!root || root.__otp) return; root.__otp = true;
  var cells = Array.prototype.slice.call(root.querySelectorAll('[data-otp-cell]'));
  var hidden = root.querySelector('[data-otp-value]');
  function sync(){
    if (!hidden) return;
    hidden.value = cells.map(function(c){ return c.value || ''; }).join('');
    hidden.dispatchEvent(new Event('change', { bubbles: true }));
  }
  cells.forEach(function(cell, i){
    scope.addEvent(cell, 'input', function(){
      if (cell.value.length > 1) cell.value = cell.value.slice(-1);
      if (cell.value && i < cells.length - 1) cells[i + 1].focus();
      sync();
    });
    scope.addEvent(cell, 'keydown', function(e){
      if (e.key === 'Backspace' && !cell.value && i > 0) { cells[i - 1].focus(); }
      else if (e.key === 'ArrowLeft' && i > 0) { cells[i - 1].focus(); }
      else if (e.key === 'ArrowRight' && i < cells.length - 1) { cells[i + 1].focus(); }
    });
    scope.addEvent(cell, 'focus', function(){ cell.select && cell.select(); });
    scope.addEvent(cell, 'paste', function(e){
      e.preventDefault();
      var data = (e.clipboardData || (scope.parent().ownerDocument.defaultView || window).clipboardData);
      var text = (data ? data.getData('text') : '').replace(/[\s-]/g, '');
      for (var j = 0; j + i < cells.length && j < text.length; j++) cells[i + j].value = text[j];
      sync();
      cells[Math.min(i + text.length, cells.length - 1)].focus();
    });
  });
}"#;

/// OTP-field component over a `String` code signal.
#[must_use]
pub fn otp_field(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: OtpConfig,
    set_code: SignalSetter<String>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("otp-field"));
    let cell_type: Cow<'static, str> =
        if config.mask { Cow::Borrowed("password") } else { Cow::Borrowed("text") };
    let inputmode: Cow<'static, str> =
        if config.numeric { Cow::Borrowed("numeric") } else { Cow::Borrowed("text") };
    let disabled = config.disabled;

    let cells: Vec<Html> = (0..config.length)
        .map(|i| {
            let ct = cell_type.clone();
            let im = inputmode.clone();
            let autocomplete: Option<&'static str> = (i == 0).then_some("one-time-code");
            html! { ctx, rcv,
                <input class="otp-cell" data-otp-cell="true"
                       type=[ct] inputmode=[im] maxlength="1"
                       autocomplete=[autocomplete]
                       aria-label="Digit"
                       disabled={disabled.then_some("")} />
            }
        })
        .collect();

    html! { ctx, rcv,
        <div class=[class] role="group" aria-label="One-time code"
             data-disabled={disabled.then_some("")}>
            <Fragment>{cells.clone()}</Fragment>
            <input type="hidden" data-otp-value="true" name=[config.name]
                   primal:onchange={set_code} />
            <Fragment>{scoped_script(OTP_JS)}</Fragment>
        </div>
    }
}
