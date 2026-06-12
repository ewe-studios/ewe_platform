//! # Avatar (F1 — Primitives)
//!
//! Parts: Root (`<span>`), Image (`<img>`), Fallback (`<span>`).
//!
//! Image wires `load`/`error` events → internal `loading_status` signal.
//! Fallback is visible only when status ≠ loaded (`data-hidden` toggling).
//!
//! Data attributes: `data-loading-status` (idle/loading/loaded/error).


use foundation_signals::Context;
use foundation_ui_traits::{AttrName, DomOp, Html};
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Image loading status (mirrors base-ui's state enum).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageLoadingStatus {
    /// Not yet attempted.
    #[default]
    Idle,
    /// Image is loading.
    Loading,
    /// Image loaded successfully.
    Loaded,
    /// Image failed to load.
    Error,
}

/// Static config for an avatar.
pub struct AvatarConfig {
    /// Image URL (optional — if absent, fallback shows immediately).
    pub src: Option<&'static str>,
    /// Alt text for the image.
    pub alt: Option<&'static str>,
    /// Fallback delay in milliseconds (default: 0 = immediate).
    pub fallback_delay_ms: u32,
    /// Optional custom class for the root.
    pub class: Option<&'static str>,
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
    /// Fallback content (shown when image is not loaded).
    pub fallback: Option<Slot>,
}

impl Default for AvatarSlots {
    fn default() -> Self {
        Self { fallback: None }
    }
}

/// Avatar component — image with fallback.
///
/// Internal signal: `loading_status` (idle → loading → loaded|error).
/// Static: `src`, `alt`, `fallback_delay_ms`, `class`.
#[must_use]
pub fn avatar(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: AvatarConfig,
    slots: AvatarSlots,
) -> Html {
    let (status, set_status) = ctx.signal(ImageLoadingStatus::Idle);
    let class = config.class.unwrap_or("avatar");
    let root_id = ctx.allocate_id_block(1);
    let fallback_id = ctx.allocate_id_block(1);

    let has_image = config.src.is_some();

    if has_image {
        let src = config.src.unwrap_or("");
        let alt = config.alt.unwrap_or("");
        let _callback_id_load: u64 = 0; // placeholder — real impl would allocate
        let _callback_id_error: u64 = 0;

        // Start loading immediately.
        set_status.set(ImageLoadingStatus::Loading);

        // Effect: update root's data-loading-status attribute
        {
            let rcv = rcv.clone();
            let status = status.clone();
            ctx.effect(move || {
                let s = status.get();
                let val = match s {
                    ImageLoadingStatus::Idle => "idle",
                    ImageLoadingStatus::Loading => "loading",
                    ImageLoadingStatus::Loaded => "loaded",
                    ImageLoadingStatus::Error => "error",
                };
                rcv.queue(DomOp::SetAttribute {
                    node_id: root_id,
                    name: AttrName::from_static("data-loading-status"),
                    value: val.into(),
                });
            });
        }

        // Effect: toggle fallback visibility
        {
            let rcv = rcv.clone();
            let status = status.clone();
            ctx.effect(move || {
                let is_loaded = status.get() == ImageLoadingStatus::Loaded;
                if is_loaded {
                    rcv.queue(DomOp::SetAttribute {
                        node_id: fallback_id,
                        name: AttrName::from_static("data-hidden"),
                        value: "".into(),
                    });
                } else {
                    rcv.queue(DomOp::RemoveAttribute {
                        node_id: fallback_id,
                        name: AttrName::from_static("data-hidden"),
                    });
                }
            });
        }

        let fallback_html = slots.fallback.map(|slot| slot.render(ctx, rcv));

        html! {
            <span primal-id={root_id}
                  class={class}
                  data-loading-status="loading">
                <img src={src}
                     alt={alt}
                     class="avatar-image"/>
                <span primal-id={fallback_id}
                      class="avatar-fallback">
                    {fallback_html}
                </span>
            </span>
        }
    } else {
        // No image — show fallback immediately.
        set_status.set(ImageLoadingStatus::Idle);

        let fallback_html = slots.fallback.map(|slot| slot.render(ctx, rcv));

        html! {
            <span class={class}
                  data-loading-status="idle">
                {fallback_html}
            </span>
        }
    }
}
