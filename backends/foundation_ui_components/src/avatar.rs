//! # Avatar (F1 — Primitives)
//!
//! WHY: An image that falls back to initials/icon on load failure, without a
//! flash of fallback while the image is still loading (spec-42 §F1).
//!
//! WHAT: Parts Root (`<span>`), Image (`<img>`), Fallback (`<span>`). The
//! image's `load`/`error` events drive an internal `loading_status` signal;
//! the fallback is hidden once the image is `loaded` (`data-hidden`).
//!
//! HOW: `load`/`error` carry no value the default setter could read, so they
//! wire through `ctx.callback` (the event escape hatch). `fallback_delay_ms`
//! is emitted as the `--avatar-fallback-delay` custom property — CSS owns the
//! "wait before showing fallback" via `transition-delay`, no JS timer.

use alloc::borrow::Cow;
use alloc::string::String;

use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Image loading status (mirrors base-ui's state enum).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageLoadingStatus {
    /// Not yet attempted (no `src`).
    #[default]
    Idle,
    /// Image is loading.
    Loading,
    /// Image loaded successfully.
    Loaded,
    /// Image failed to load.
    Error,
}

impl ImageLoadingStatus {
    /// The `data-loading-status` token.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ImageLoadingStatus::Idle => "idle",
            ImageLoadingStatus::Loading => "loading",
            ImageLoadingStatus::Loaded => "loaded",
            ImageLoadingStatus::Error => "error",
        }
    }
}

/// Static config for an avatar. Text fields are `Cow<'static, str>`.
pub struct AvatarConfig {
    /// Image URL — if absent, the fallback shows immediately.
    pub src: Option<Cow<'static, str>>,
    /// Alt text for the image.
    pub alt: Option<Cow<'static, str>>,
    /// Delay before the fallback appears, in ms (emitted as a CSS var so CSS
    /// owns the timing; `0` = immediate).
    pub fallback_delay_ms: u32,
    /// Class override for the root (default: `"avatar"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for AvatarConfig {
    fn default() -> Self {
        Self {
            src: None,
            alt: None,
            fallback_delay_ms: 0,
            class: None,
        }
    }
}

/// Slots for the avatar.
pub struct AvatarSlots {
    /// Fallback content (shown when the image is not loaded).
    pub fallback: Option<Slot>,
}

impl Default for AvatarSlots {
    fn default() -> Self {
        Self { fallback: None }
    }
}

/// Avatar component — image with fallback.
///
/// Internal signal: `loading_status` (idle → loading → loaded|error), driven
/// by the image's `load`/`error` events. Static: `src`, `alt`,
/// `fallback_delay_ms`, `class`.
#[must_use]
pub fn avatar(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: AvatarConfig,
    slots: AvatarSlots,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("avatar"));
    let fallback_html = slots.fallback.map(|slot| slot.render(ctx, rcv));

    let Some(src) = config.src else {
        // No image — fallback shows immediately.
        return html! { ctx, rcv,
            <span class=[class] data-loading-status="idle">
                {fallback_html.clone()}
            </span>
        };
    };

    let alt = config.alt.unwrap_or(Cow::Borrowed(""));
    let delay_style: String = alloc::format!("--avatar-fallback-delay: {}ms", config.fallback_delay_ms);
    let (status, set_status) = ctx.signal(ImageLoadingStatus::Loading);

    // load/error carry no value → Callback escape hatch flips the status.
    let on_load = {
        let set = set_status.clone();
        ctx.callback(move |_| set.set(ImageLoadingStatus::Loaded))
    };
    let on_error = {
        let set = set_status.clone();
        ctx.callback(move |_| set.set(ImageLoadingStatus::Error))
    };

    let status_attr = status.clone();
    let fallback_status = status.clone();
    html! { ctx, rcv,
        <span class=[class]
              style=[delay_style]
              data-loading-status={status_attr.get().as_str()}>
            <img src=[src] alt=[alt] class="avatar-image"
                 primal:onload={on_load}
                 primal:onerror={on_error}/>
            <span class="avatar-fallback"
                  data-hidden={(fallback_status.get() == ImageLoadingStatus::Loaded).then_some("")}>
                {fallback_html.clone()}
            </span>
        </span>
    }
}
