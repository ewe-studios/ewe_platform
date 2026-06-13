//! # Number field (F7 — Form) — with M8 hold-repeat + scrub
//!
//! WHY: A numeric input with decrement/increment affordances, step semantics,
//! press-and-hold auto-repeat, and pointer-drag scrubbing — the reasons it
//! exists over `<input type=number>` (spec-42 §F7, machinery.md §M8).
//!
//! WHAT: [`number_field`] renders Root → Decrement / Input / Increment (+ an
//! optional ScrubArea/Cursor) over a `f64` signal, driving the full M8 JS
//! behavior ported 1:1 from base-ui (`usePressAndHold` + `NumberFieldScrubArea`
//! + `toValidatedNumber`).
//!
//! HOW: The input is `inputmode="decimal"` TEXT (not `type=number`). A scoped
//! script owns ALL value mutation so modifiers/snap/clamp are uniform: on
//! pointer-down of a button it steps once then, after `START_AUTO_CHANGE_DELAY`
//! (400 ms), repeats every `CHANGE_VALUE_TICK_DELAY` (60 ms) until pointer-up
//! (cancelling if the pointer moves > `SCROLLING_POINTER_MOVE_DISTANCE` = 8 px);
//! keyboard ↑/↓/PageUp/PageDown/Home/End step; wheel scrubs (when enabled); the
//! ScrubArea drags with pointer lock + a virtual cursor (`pixelSensitivity = 2`,
//! viewport-edge teleport). Alt = `smallStep`, Shift = `largeStep`. Step
//! interactions always clamp (and snap when `snapOnStep`); only TYPED text may
//! exceed the range. Each mutation writes the input + dispatches `change` →
//! the Rust `set_value` parses it.

use alloc::borrow::Cow;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::scoped_script;

/// Static config for a number field.
pub struct NumberFieldConfig {
    /// Minimum value (inclusive).
    pub min: Option<f64>,
    /// Maximum value (inclusive).
    pub max: Option<f64>,
    /// Step for arrow/button changes (default 1.0).
    pub step: f64,
    /// Step when Shift is held (default 10.0).
    pub large_step: f64,
    /// Step when Alt is held (default 0.1).
    pub small_step: f64,
    /// Snap to the step grid on change.
    pub snap_on_step: bool,
    /// Allow the wheel to scrub the value when the input is focused.
    pub allow_wheel_scrub: bool,
    /// Allow TYPED text to exceed the range (step interactions still clamp).
    pub allow_out_of_range: bool,
    /// Render a ScrubArea (pointer-drag scrubbing with a virtual cursor).
    pub scrub: bool,
    /// Disabled.
    pub disabled: bool,
    /// `name` for form submission.
    pub name: Option<Cow<'static, str>>,
    /// Class override (default `"number-field"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for NumberFieldConfig {
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            step: 1.0,
            large_step: 10.0,
            small_step: 0.1,
            snap_on_step: false,
            allow_wheel_scrub: false,
            allow_out_of_range: false,
            scrub: false,
            disabled: false,
            name: None,
            class: None,
            aria_label: None,
        }
    }
}

/// The M8 number-field behavior: hold-repeat, keyboard step, wheel + pointer
/// scrubbing, with step-modifier / snap / clamp semantics ported from base-ui.
pub const NUMBER_FIELD_JS: &str = r#"function(scope){
  var root = scope.parent();
  if (!root || root.__nf) return; root.__nf = true;
  var win = (root.ownerDocument && root.ownerDocument.defaultView) || window;
  var doc = root.ownerDocument || document;
  var input = root.querySelector('[data-nf-input]');
  var inc = root.querySelector('[data-nf-increment]');
  var dec = root.querySelector('[data-nf-decrement]');
  var scrub = root.querySelector('[data-nf-scrub]');
  var cursor = root.querySelector('[data-nf-cursor]');
  if (!input) return;
  function num(a){ var v = parseFloat(root.getAttribute(a)); return isNaN(v) ? null : v; }
  var min = num('data-min'), max = num('data-max');
  var step = num('data-step') || 1, largeStep = num('data-large-step') || 10, smallStep = num('data-small-step') || 0.1;
  var snap = root.getAttribute('data-snap') === 'true';
  var allowWheel = root.getAttribute('data-allow-wheel') === 'true';
  function stepAmount(e){ if (e && e.altKey) return smallStep; if (e && e.shiftKey) return largeStep; return step; }
  function clampV(v){ if (min != null && v < min) v = min; if (max != null && v > max) v = max; return v; }
  function snapTo(v, st, nearest){
    var base = (min != null) ? min : 0, size = Math.abs(st), raw = v - base;
    if (nearest) return base + Math.round(raw / st) * st;
    var snapped = Math.sign(st) > 0 ? Math.floor(raw / size) : Math.ceil(raw / size);
    return base + snapped * size;
  }
  function curVal(){ var v = parseFloat(input.value); return isNaN(v) ? null : v; }
  function commit(v){
    input.value = (v == null) ? '' : String(Math.round(v * 1e10) / 1e10);
    if (input.hasAttribute('aria-valuenow')) input.setAttribute('aria-valuenow', input.value);
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }
  function applyStep(dir, e){
    var amt = stepAmount(e), cur = curVal();
    if (cur == null) cur = (min != null ? min : 0);
    var nv = cur + dir * amt;
    if (snap) nv = snapTo(nv, amt, !!(e && e.altKey));
    commit(clampV(nv));
  }
  function holdRepeat(btn, dir){
    if (!btn) return;
    scope.addEvent(btn, 'pointerdown', function(e){
      if (e.button) return;
      e.preventDefault();
      if (e.pointerType !== 'touch' && e.pointerType !== 'pen') input.focus();
      var sx = e.clientX, sy = e.clientY, startT = null, iv = null;
      applyStep(dir, e);
      startT = win.setTimeout(function(){ iv = win.setInterval(function(){ applyStep(dir, e); }, 60); }, 400);
      function stop(){
        if (startT) win.clearTimeout(startT);
        if (iv) win.clearInterval(iv);
        win.removeEventListener('pointerup', stop, true);
        win.removeEventListener('pointermove', mv, true);
        win.removeEventListener('contextmenu', ctx, true);
      }
      function mv(ev){ if (Math.abs(ev.clientX - sx) > 8 || Math.abs(ev.clientY - sy) > 8) stop(); }
      function ctx(ev){ ev.preventDefault(); }
      win.addEventListener('pointerup', stop, true);
      win.addEventListener('pointermove', mv, true);
      win.addEventListener('contextmenu', ctx, true);
    });
  }
  holdRepeat(inc, 1); holdRepeat(dec, -1);
  scope.addEvent(input, 'keydown', function(e){
    if (e.key === 'ArrowUp') { e.preventDefault(); applyStep(1, e); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); applyStep(-1, e); }
    else if (e.key === 'PageUp') { e.preventDefault(); applyStep(1, { shiftKey: true }); }
    else if (e.key === 'PageDown') { e.preventDefault(); applyStep(-1, { shiftKey: true }); }
    else if (e.key === 'Home' && min != null) { e.preventDefault(); commit(min); }
    else if (e.key === 'End' && max != null) { e.preventDefault(); commit(max); }
  });
  if (allowWheel) {
    scope.addEvent(input, 'wheel', function(e){
      if (doc.activeElement !== input) return;
      e.preventDefault();
      applyStep(e.deltaY < 0 ? 1 : -1, e);
    });
  }
  if (scrub) {
    var scrubbing = false, cum = 0, coords = { x: 0, y: 0 };
    function setCursor(x, y){ if (cursor) cursor.style.transform = 'translate3d(' + x + 'px,' + y + 'px,0)'; }
    function onScrub(e){
      if (!cursor) return;
      var w = win.innerWidth, h = win.innerHeight, cw = cursor.offsetWidth, ch = cursor.offsetHeight;
      var nx = Math.round(coords.x + e.movementX), ny = Math.round(coords.y + e.movementY);
      if (nx + cw / 2 < 0) nx = w - cw / 2; else if (nx + cw / 2 > w) nx = -cw / 2;
      if (ny + ch / 2 < 0) ny = h - ch / 2; else if (ny + ch / 2 > h) ny = -ch / 2;
      coords = { x: nx, y: ny }; setCursor(nx, ny);
    }
    scope.addEvent(scrub, 'pointerdown', function(e){
      if (e.button) return;
      e.preventDefault(); input.focus();
      scrubbing = true; cum = 0;
      scrub.setAttribute('data-scrubbing', '');
      if (cursor) { coords = { x: e.clientX - cursor.offsetWidth / 2, y: e.clientY - cursor.offsetHeight / 2 }; setCursor(coords.x, coords.y); }
      if (e.pointerType !== 'touch') { try { doc.body.requestPointerLock(); } catch (_) {} }
    });
    scope.addEvent(win, 'pointermove', function(e){
      if (!scrubbing) return;
      e.preventDefault(); onScrub(e);
      cum += e.movementX;
      if (Math.abs(cum) >= 2) { cum = 0; applyStep(e.movementX >= 0 ? 1 : -1, e); }
    });
    scope.addEvent(win, 'pointerup', function(){
      if (!scrubbing) return; scrubbing = false;
      scrub.removeAttribute('data-scrubbing');
      try { doc.exitPointerLock(); } catch (_) {}
    });
  }
}"#;

fn fmt(v: f64) -> Cow<'static, str> {
    Cow::Owned(alloc::format!("{v}"))
}

/// Number-field component over a `f64` signal.
#[must_use]
pub fn number_field(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: NumberFieldConfig,
    value: &SignalGetter<f64>,
    set_value: SignalSetter<f64>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("number-field"));
    let disabled = config.disabled;

    // Typing parses; the JS path already clamps step interactions and writes the
    // input, so here we just adopt the parsed value (typed text may exceed range).
    let on_change = {
        let set = set_value.clone();
        ctx.callback(move |data| {
            if let Some(text) = data.value.as_deref() {
                if let Ok(n) = text.parse::<f64>() {
                    set.set(n);
                }
            }
        })
    };

    let min_attr = config.min.map(fmt);
    let max_attr = config.max.map(fmt);
    let scrub = config.scrub;
    let v_attr = value.clone();
    let aria_now = value.clone();
    html! { ctx, rcv,
        <div class=[class] role="group"
             aria-label=[config.aria_label]
             data-disabled={disabled.then_some("")}
             data-min=[min_attr]
             data-max=[max_attr]
             data-step={fmt(config.step)}
             data-large-step={fmt(config.large_step)}
             data-small-step={fmt(config.small_step)}
             data-snap=[config.snap_on_step.then_some("true")]
             data-allow-wheel=[config.allow_wheel_scrub.then_some("true")]
             data-allow-out-of-range=[config.allow_out_of_range.then_some("true")]>
            <button type="button" class="number-field-decrement"
                    data-nf-decrement="true" aria-label="Decrease" tabindex="-1"
                    disabled={disabled.then_some("")}>{"−"}</button>
            <Fragment>{scrub.then(|| html! { ctx, rcv,
                <span class="number-field-scrub-area" data-nf-scrub="true" role="presentation">
                    <span class="number-field-scrub-cursor" data-nf-cursor="true" aria-hidden="true"></span>
                </span>
            })}</Fragment>
            <input class="number-field-input" type="text" inputmode="decimal"
                   data-nf-input="true"
                   name=[config.name]
                   role="spinbutton"
                   aria-valuenow={aria_now.get()}
                   value={v_attr.get()}
                   disabled={disabled.then_some("")}
                   primal:onchange={on_change} />
            <button type="button" class="number-field-increment"
                    data-nf-increment="true" aria-label="Increase" tabindex="-1"
                    disabled={disabled.then_some("")}>{"+"}</button>
            <Fragment>{scoped_script(NUMBER_FIELD_JS)}</Fragment>
        </div>
    }
}
