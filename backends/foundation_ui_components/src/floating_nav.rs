//! # Floating nav (platform — persistent navigation toolbar)
//!
//! WHY: Every platform page needs a consistent way to navigate back, home,
//! and trigger app-switching. This component provides a single data-attribute-
//! driven toolbar that the platform shell injects once and all pages inherit.
//!
//! WHAT: [`floating_nav`] renders a fixed-position toolbar. On mobile it
//! anchors to the bottom; on desktop, to the top. Buttons emit custom events
//! that the platform shell listens for, falling back to `history.back()` and
//! `location.href` navigation.
//!
//! HOW: Pure static component — the JS behavior is scoped and self-contained.
//! Consumer CSS targets `[data-floating-nav]` and `[data-fn-button]` attribute
//! selectors. No signals needed.

use alloc::borrow::Cow;

use foundation_ui_traits::Html;
use foundation_wasm_ui::html;

use crate::machinery::scoped_script;

/// Config for the floating navigation toolbar.
pub struct FloatingNavConfig {
    /// Position: `"top"` (desktop default) or `"bottom"` (mobile).
    pub position: Cow<'static, str>,
    /// Title text in the center of the bar.
    pub title: Cow<'static, str>,
    /// Route for the home button (default: `"/app/"`).
    pub home_route: Cow<'static, str>,
    /// Route for the apps button (default: `"/"`).
    pub apps_route: Cow<'static, str>,
    /// Whether the toolbar is initially visible.
    pub visible: bool,
    /// Class override (default `"floating-nav"`).
    pub class: Cow<'static, str>,
}

impl Default for FloatingNavConfig {
    fn default() -> Self {
        Self {
            position: Cow::Borrowed("top"),
            title: Cow::Borrowed(""),
            home_route: Cow::Borrowed("/app/"),
            apps_route: Cow::Borrowed("/"),
            visible: true,
            class: Cow::Borrowed("floating-nav"),
        }
    }
}

/// Render a floating navigation toolbar with scoped JS behavior.
#[must_use]
pub fn floating_nav(config: FloatingNavConfig) -> Html {
    let hidden_attr = if !config.visible { "true" } else { "" };

    html! {
        <div class="floating-nav-wrapper" data-fn-wrapper="true">
            <nav class=[config.class]
                 data-floating-nav="true"
                 data-fn-position=[config.position]
                 data-fn-hidden={hidden_attr}>
                <button data-fn-button="back" title="Back" aria-label="Back">"Back"</button>
                <a data-fn-button="home" href={config.home_route.as_ref()} title="Home" aria-label="Home">"Home"</a>
                <span data-fn-title="true">{config.title.as_ref()}</span>
                <button data-fn-button="refresh" title="Refresh" aria-label="Refresh">"Refresh"</button>
                <a data-fn-button="apps" href={config.apps_route.as_ref()} title="Apps" aria-label="Apps">"Apps"</a>
            </nav>
            <Fragment>{scoped_script(FLOATING_NAV_JS)}</Fragment>
        </div>
    }
}

/// Scoped JS: wires button handlers and scroll-based auto-hide.
const FLOATING_NAV_JS: &str = r#"function(scope){
  var wrapper = scope.parent();
  if (!wrapper || wrapper.__fnWired) return; wrapper.__fnWired = true;
  var nav = wrapper.querySelector('[data-floating-nav]');
  if (!nav) return;
  var doc = nav.ownerDocument, win = doc.defaultView || window;

  // Back button
  var back = nav.querySelector('[data-fn-button="back"]');
  if (back) scope.addEvent(back, 'click', function(e){
    var ev = new CustomEvent('ewe:nav:back', {bubbles:true,cancelable:true});
    nav.dispatchEvent(ev);
    if (!ev.defaultPrevented) win.history.back();
  });

  // Refresh button
  var refresh = nav.querySelector('[data-fn-button="refresh"]');
  if (refresh) scope.addEvent(refresh, 'click', function(e){
    var ev = new CustomEvent('ewe:nav:refresh', {bubbles:true,cancelable:true});
    nav.dispatchEvent(ev);
    if (!ev.defaultPrevented) win.location.reload();
  });

  // Auto-hide on scroll
  var visible = true, lastY = 0;
  win.addEventListener('scroll', function(){
    var y = win.scrollY;
    if (Math.abs(y - lastY) < 40) return;
    if (y > lastY && visible) { nav.setAttribute('data-fn-hidden',''); visible = false; }
    else if (y < lastY && !visible) { nav.removeAttribute('data-fn-hidden'); visible = true; }
    lastY = y;
  }, {passive:true});

  // Public API
  win.__eweNav = {
    setTitle: function(t){ var s = nav.querySelector('[data-fn-title]'); if(s) s.textContent = t||''; },
    show: function(){ nav.removeAttribute('data-fn-hidden'); visible = true; },
    hide: function(){ nav.setAttribute('data-fn-hidden',''); visible = false; }
  };
}"#;
