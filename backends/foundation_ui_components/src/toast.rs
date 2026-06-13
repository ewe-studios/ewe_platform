//! # Toast (F4 — Overlays, the `<For>` proving component)
//!
//! WHY: A transient notification stack — the one overlay where the LIST is the
//! component (spec-42 §F4). It's the catalog's `<For>` proving ground: a
//! `Vec<Toast>` signal rendered as a live, keyed list.
//!
//! WHAT: [`Toast`] (the value), [`ToastManager`] (`add`/`close`/`update` over a
//! `Vec<Toast>` signal — the base-ui `useToastManager` analog), and
//! [`toast_viewport`] (the `role="region"` live region rendering the stack via
//! `<For>`).
//!
//! HOW: The viewport is `popover="manual"` (top layer, no light dismiss) with
//! `aria-live` (polite; assertive for high priority). Each toast carries
//! `data-type`/`data-toast` for styling and embeds an auto-dismiss timer
//! (scoped script: clicks `[data-toast-close]` after `data-timeout` ms, paused
//! while hovered). Close `.click()`s the wired callback → the manager drops it
//! from the Vec. Swipe-to-dismiss + stacking offsets are the M8 follow-up.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::scoped_script;

/// A single toast value.
#[derive(Clone, PartialEq, Eq)]
pub struct Toast {
    /// Stable id (`<For>` key).
    pub id: String,
    /// Type token for styling + `data-type` (`info`/`success`/`error`/…).
    pub kind: Cow<'static, str>,
    /// Title content.
    pub title: Cow<'static, str>,
    /// Optional description.
    pub description: Option<Cow<'static, str>>,
    /// Auto-dismiss timeout in ms (0 = sticky).
    pub timeout_ms: u32,
    /// High priority → assertive live region.
    pub high_priority: bool,
}

impl Toast {
    /// A minimal toast with the default 5 s timeout.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<Cow<'static, str>>) -> Self {
        Self {
            id: id.into(),
            kind: Cow::Borrowed("info"),
            title: title.into(),
            description: None,
            timeout_ms: 5000,
            high_priority: false,
        }
    }
}

/// Manager over a `Vec<Toast>` signal — `add`/`close`/`update`.
#[derive(Clone)]
pub struct ToastManager {
    toasts: SignalGetter<Vec<Toast>>,
    set_toasts: SignalSetter<Vec<Toast>>,
    next_id: alloc::rc::Rc<Cell<u64>>,
}

impl ToastManager {
    /// Create a manager over a fresh `Vec<Toast>` signal.
    #[must_use]
    pub fn new(ctx: &Context) -> Self {
        let (toasts, set_toasts) = ctx.signal::<Vec<Toast>>(Vec::new());
        Self { toasts, set_toasts, next_id: alloc::rc::Rc::new(Cell::new(0)) }
    }

    /// The toasts getter (so the viewport can render them).
    #[must_use]
    pub fn toasts(&self) -> &SignalGetter<Vec<Toast>> {
        &self.toasts
    }

    /// Add a toast; returns its id (auto-assigned if empty).
    pub fn add(&self, mut toast: Toast) -> String {
        if toast.id.is_empty() {
            let n = self.next_id.get();
            self.next_id.set(n + 1);
            toast.id = alloc::format!("toast-{n}");
        }
        let id = toast.id.clone();
        let mut current = self.toasts.get();
        current.push(toast);
        self.set_toasts.set(current);
        id
    }

    /// Remove a toast by id.
    pub fn close(&self, id: &str) {
        let mut current = self.toasts.get();
        current.retain(|t| t.id != id);
        self.set_toasts.set(current);
    }

    /// Replace a toast in place (same id), e.g. a promise resolving.
    pub fn update(&self, toast: Toast) {
        let mut current = self.toasts.get();
        if let Some(slot) = current.iter_mut().find(|t| t.id == toast.id) {
            *slot = toast;
            self.set_toasts.set(current);
        }
    }
}

/// Auto-dismiss timer for one toast: clicks `[data-toast-close]` after
/// `data-timeout` ms, pausing while hovered/focused (base-ui's pause-on-hover).
const TOAST_TIMER_JS: &str = r#"function(scope){
  var el = scope.parent();
  if (!el || el.__toastTimer) return; el.__toastTimer = true;
  var win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
  var ms = parseInt(el.getAttribute('data-timeout'), 10) || 0;
  if (ms <= 0) return;
  var timer = null;
  function start(){ stop(); timer = win.setTimeout(function(){
    var a = el.querySelector('[data-toast-close]'); if (a) a.click();
  }, ms); }
  function stop(){ if (timer) { win.clearTimeout(timer); timer = null; } }
  scope.addEvent(el, 'pointerenter', stop);
  scope.addEvent(el, 'pointerleave', start);
  scope.addEvent(el, 'focusin', stop);
  scope.addEvent(el, 'focusout', start);
  start();
}"#;

/// Render the toast viewport — a live region rendering the stack via `<For>`.
#[must_use]
pub fn toast_viewport(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    manager: &ToastManager,
) -> Html {
    let toasts = manager.toasts.clone();
    let manager_for_render = manager.clone();

    let render = move |c: &Context, r: &SharedInstructionReceiver, toast: &Toast| -> Html {
        let id = toast.id.clone();
        let kind: Cow<'static, str> = toast.kind.clone();
        let title: Cow<'static, str> = toast.title.clone();
        let description = toast.description.clone();
        let live: Cow<'static, str> =
            if toast.high_priority { Cow::Borrowed("assertive") } else { Cow::Borrowed("polite") };
        let timeout_attr: Cow<'static, str> = Cow::Owned(alloc::format!("{}", toast.timeout_ms));

        let close = {
            let manager = manager_for_render.clone();
            let id = id.clone();
            c.callback(move |_| manager.close(&id))
        };

        html! { c, r,
            <div class="toast" role="status"
                 aria-live=[live]
                 data-toast="true"
                 data-type=[kind]
                 data-timeout=[timeout_attr]>
                <div class="toast-title"><Fragment>{Html::text(title)}</Fragment></div>
                <Fragment>{description.map(|d| html! { c, r,
                    <div class="toast-description"><Fragment>{Html::text(d)}</Fragment></div>
                })}</Fragment>
                <button type="button" class="toast-close" data-toast-close="true"
                        aria-label="Close" primal:onclick={close}><Fragment>{Html::text("×")}</Fragment></button>
                <Fragment>{scoped_script(TOAST_TIMER_JS)}</Fragment>
            </div>
        }
    };

    html! { ctx, rcv,
        <div class="toast-viewport" role="region" aria-label="Notifications"
             popover="manual" tabindex="-1">
            <For each={toasts.get()} key={|t: &Toast| t.id.clone()} render={render} />
        </div>
    }
}
