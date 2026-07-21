//! # Remote page (platform — iframe wrapper with toolbar)
//!
//! WHY: Every platform app that renders external content needs a consistent
//! wrapper: an iframe with sandboxing, a navigation toolbar, and scheme
//! interception. Baking this into a component means route handlers never
//! hand-roll the wrapper HTML.
//!
//! WHAT: [`remote_page`] renders a full-viewport page: toolbar at the top,
//! a sandboxed iframe filling the remaining height, and a scoped JS behavior
//! that wires the back/refresh buttons and injects the scheme interceptor.
//!
//! HOW: Pure component — no signals. The toolbar buttons are data-attribute-
//! driven; the iframe `srcdoc` receives escaped remote content.

use alloc::borrow::Cow;
use alloc::string::String;

use foundation_ui_traits::Html;
use foundation_wasm_ui::html;

use crate::machinery::scoped_script;

/// Config for a remote page wrapper.
pub struct RemotePageConfig {
    /// Title shown in the toolbar center.
    pub title: Cow<'static, str>,
    /// The HTML content to render inside the sandboxed iframe (via `srcdoc`).
    pub content: String,
    /// Class override for the root wrapper (default `"remote-page"`).
    pub class: Cow<'static, str>,
    /// Whether to show the back button (default: true).
    pub show_back: bool,
    /// Whether to show the home button (default: true).
    pub show_home: bool,
    /// Whether to show the refresh button (default: true).
    pub show_refresh: bool,
    /// The route to navigate to when home is clicked (e.g. `"ewe://localhost/app/"`).
    pub home_route: Cow<'static, str>,
}

impl Default for RemotePageConfig {
    fn default() -> Self {
        Self {
            title: Cow::Borrowed("Remote"),
            content: String::new(),
            class: Cow::Borrowed("remote-page"),
            show_back: true,
            show_home: true,
            show_refresh: true,
            home_route: Cow::Borrowed("/app/"),
        }
    }
}

/// Render a remote page: toolbar + sandboxed iframe with the given content.
#[must_use]
pub fn remote_page(config: RemotePageConfig) -> Html {
    // Escape the content for safe embedding in a srcdoc attribute.
    let srcdoc: Cow<'static, str> = Cow::Owned(
        config
            .content
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
    );

    html! {
        <div class=[config.class] data-remote-page="true">
            <div class="remote-toolbar" data-remote-toolbar="true">
                <button data-remote-nav="back" title="Back">"Back"</button>
                <a data-remote-nav="home" href=[config.home_route] title="Home">"Home"</a>
                <span class="remote-title" data-remote-title="true">{config.title.as_ref()}</span>
                <button data-remote-nav="refresh" title="Refresh">"Refresh"</button>
            </div>
            <div class="remote-iframe-container" data-remote-iframe-container="true">
                <iframe
                    sandbox="allow-scripts allow-same-origin allow-forms"
                    srcdoc=[srcdoc]
                    title="Remote content"
                />
            </div>
            <Fragment>{scoped_script(REMOTE_PAGE_JS)}</Fragment>
        </div>
    }
}

/// Scoped JS: wires toolbar buttons and injects the scheme interceptor.
const REMOTE_PAGE_JS: &str = r#"function(scope){
  var root = scope.parent();
  if (!root || root.__remotePage) return; root.__remotePage = true;
  var doc = root.ownerDocument, win = doc.defaultView || window;

  // Scheme interceptor — catches ewe:// links in the iframe
  doc.addEventListener('click', function(e){
    var a = e.target.closest('a');
    if (!a) return;
    var href = a.getAttribute('href') || '';
    if (/^(ewe|foundation|platform):\/\//.test(href)){
      e.preventDefault();
      win.location.href = href;
    }
  }, true);

  // Back button
  var back = root.querySelector('[data-remote-nav="back"]');
  if (back) scope.addEvent(back, 'click', function(){ win.history.back(); });

  // Refresh button
  var refresh = root.querySelector('[data-remote-nav="refresh"]');
  if (refresh) scope.addEvent(refresh, 'click', function(){ win.location.reload(); });
}"#;
