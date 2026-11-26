use alloc::string::String;
use core::fmt::Write;

use foundation_ui_traits::{ATTR_NAMES, TAG_NAMES};

use super::{prop_kind, MANAGED_CONNECTION_VERSION, STRUCTURED_INPUT_VERSION, VOCABULARY_VERSION};
use crate::{EventId, NodeKind, PropId};

/// The web element that represents a shared node kind.
pub struct WebKind {
    pub kind: NodeKind,
    pub tag: &'static str,
    pub class: &'static str,
    pub attrs: &'static [(&'static str, &'static str)],
}

pub static WEB_KINDS: &[WebKind] = &[
    WebKind {
        kind: NodeKind::STACK,
        tag: "div",
        class: "ewe-stack",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::TEXT,
        tag: "span",
        class: "ewe-text",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::IMAGE,
        tag: "img",
        class: "ewe-image",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::BUTTON,
        tag: "button",
        class: "ewe-button",
        attrs: &[("type", "button")],
    },
    WebKind {
        kind: NodeKind::TEXT_INPUT,
        tag: "input",
        class: "ewe-text-input",
        attrs: &[("type", "text")],
    },
    WebKind {
        kind: NodeKind::TOGGLE,
        tag: "input",
        class: "ewe-toggle",
        attrs: &[("type", "checkbox"), ("role", "switch")],
    },
    WebKind {
        kind: NodeKind::LIST,
        tag: "div",
        class: "ewe-list",
        attrs: &[("role", "list")],
    },
    WebKind {
        kind: NodeKind::SCROLL,
        tag: "div",
        class: "ewe-scroll",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::SPACER,
        tag: "div",
        class: "ewe-spacer",
        attrs: &[("aria-hidden", "true")],
    },
    WebKind {
        kind: NodeKind::OVERLAY,
        tag: "div",
        class: "ewe-overlay",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::MODAL,
        tag: "dialog",
        class: "ewe-modal",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::BOTTOM_SHEET,
        tag: "section",
        class: "ewe-bottom-sheet",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::NAVIGATION_TITLE,
        tag: "h1",
        class: "ewe-navigation-title",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::TOOLBAR_BUTTON,
        tag: "button",
        class: "ewe-toolbar-button",
        attrs: &[("type", "button")],
    },
    WebKind {
        kind: NodeKind::SEGMENTED_CONTROL,
        tag: "div",
        class: "ewe-segmented-control",
        attrs: &[("role", "group")],
    },
    WebKind {
        kind: NodeKind::INSET_LIST,
        tag: "ul",
        class: "ewe-inset-list",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::CHECKLIST_ROW,
        tag: "li",
        class: "ewe-checklist-row",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::CHECKMARK_BUTTON,
        tag: "button",
        class: "ewe-checkmark-button",
        attrs: &[("type", "button")],
    },
    WebKind {
        kind: NodeKind::DESTRUCTIVE_BUTTON,
        tag: "button",
        class: "ewe-destructive-button",
        attrs: &[("type", "button")],
    },
    WebKind {
        kind: NodeKind::LIST_FOOTER,
        tag: "p",
        class: "ewe-list-footer",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::CONFIRMATION_DIALOG,
        tag: "dialog",
        class: "ewe-confirmation-dialog",
        attrs: &[("role", "alertdialog")],
    },
    WebKind {
        kind: NodeKind::SHEET_TITLE,
        tag: "h2",
        class: "ewe-sheet-title",
        attrs: &[],
    },
    WebKind {
        kind: NodeKind::SUBMIT_BUTTON,
        tag: "button",
        class: "ewe-submit-button",
        attrs: &[("type", "submit")],
    },
    WebKind {
        kind: NodeKind::CANCEL_BUTTON,
        tag: "button",
        class: "ewe-cancel-button",
        attrs: &[("type", "button")],
    },
    WebKind {kind:NodeKind::DRAWING_SURFACE,tag:"canvas",class:"ewe-drawing-surface",attrs:&[]},
    WebKind {
        kind: NodeKind::SEARCH_INPUT,
        tag: "input",
        class: "ewe-search-input",
        attrs: &[("type", "search")],
    },
];

pub static WEB_EVENTS: &[(EventId, &str)] = &[
    (EventId::TAP, "click"),
    (EventId::CHANGE, "change"),
    (EventId::SUBMIT, "submit"),
    (EventId::FOCUS, "focus"),
    (EventId::BLUR, "blur"),
];

/// Base stylesheet for shared node classes.
pub const CORE_CSS: &str = "\
[primal-id][hidden]{display:none!important}
.ewe-stack{display:flex;flex-direction:column}
.ewe-list{display:flex;flex-direction:column}
.ewe-scroll{overflow:auto}
.ewe-spacer{flex:1 1 auto}
.ewe-drawing-surface{width:300px;height:150px}
.ewe-navigation-title,.ewe-sheet-title{font-family:system-ui,sans-serif;line-height:1.2;letter-spacing:-.025em;margin:.3em 0}
.ewe-toolbar-button,.ewe-submit-button,.ewe-cancel-button,.ewe-destructive-button,.ewe-checkmark-button,.ewe-segmented-control button{font:inherit;border:1px solid color-mix(in srgb,CanvasText 25%,Canvas);border-radius:.5em;padding:.45em .85em;cursor:pointer}
.ewe-toolbar-button,.ewe-submit-button{align-self:flex-start;background:var(--color-primary,ButtonText);color:Canvas;color:contrast-color(var(--color-primary,ButtonText))}
.ewe-cancel-button,.ewe-checkmark-button,.ewe-segmented-control button{background:Canvas;color:CanvasText}
.ewe-destructive-button{background:Canvas;color:var(--color-error,#a51f26)}
.ewe-segmented-control,.ewe-checklist-row{display:flex;align-items:center;flex-wrap:wrap;gap:.45em}
.ewe-segmented-control{margin-block:.75em}
.ewe-checklist-row{padding-block:.6em;border-block-end:1px solid color-mix(in srgb,CanvasText 15%,Canvas)}
.ewe-checklist-row>.ewe-text{flex:1 1 auto}
.ewe-inset-list{padding-inline-start:0;list-style:none;margin-block:.5em}
.ewe-list-footer{font-size:.9em}
.ewe-segmented-control button[aria-pressed=true]{background:var(--color-primary,ButtonText);color:Canvas;color:contrast-color(var(--color-primary,ButtonText));font-weight:600}
.ewe-confirmation-dialog,.ewe-modal{font:inherit;color:CanvasText;background:Canvas;border:1px solid ButtonBorder;border-radius:.75em}
.ewe-confirmation-dialog::backdrop,.ewe-modal::backdrop{background:rgb(0 0 0 / .35)}
.ewe-confirmation-dialog>.ewe-sheet-title{margin-block-end:.8em}
.ewe-overlay{position:fixed;inset:0;z-index:1;display:flex;flex-direction:column;pointer-events:none}
.ewe-overlay[popover]{margin:0;border:0;padding:0;inline-size:auto;block-size:auto;overflow:visible;background:transparent;color:inherit}
.ewe-overlay[popover]:not(:popover-open){display:none}
.ewe-overlay>*{pointer-events:auto}
.ewe-modal{max-block-size:100dvh;box-sizing:border-box}
.ewe-modal:has(>.ewe-bottom-sheet){inset:auto 0 0;margin:0;inline-size:100%;max-inline-size:none;padding:0;border:0}
.ewe-bottom-sheet{display:flex;flex-direction:column;margin-block-start:auto;max-block-size:100dvh;overflow:auto;padding-block-end:env(safe-area-inset-bottom);box-sizing:border-box;background:Canvas;color:CanvasText}
.ewe-bottom-sheet:has(>.ewe-submit-button){display:grid;grid-template-columns:minmax(0,1fr) auto;gap:.8em;padding:1.25em max(1.25em,calc((100% - 36em)/2)) max(1.25em,env(safe-area-inset-bottom))}
.ewe-bottom-sheet:has(>.ewe-submit-button)>*{grid-column:1/-1}
.ewe-bottom-sheet:has(>.ewe-submit-button)>.ewe-cancel-button{grid-column:1;justify-self:end}
.ewe-bottom-sheet:has(>.ewe-submit-button)>.ewe-submit-button{grid-column:2}
.ewe-bottom-sheet>.ewe-text-input{font:inherit;padding:.65em .8em;border:1px solid GrayText;border-radius:.5em}
";

/// Generate the browser vocabulary module from these tables.
#[must_use]
pub fn vocab_js() -> String {
    let mut out =
        String::from("// Generated by foundation_ui_wire::vocab::vocab_js. Do not edit.\n");
    writeln!(
        out,
        "export const VOCABULARY_VERSION = {VOCABULARY_VERSION};\nexport const STRUCTURED_INPUT_VERSION = {STRUCTURED_INPUT_VERSION};\nexport const MANAGED_CONNECTION_VERSION = {MANAGED_CONNECTION_VERSION};"
    )
    .expect("Writing to a String cannot fail");

    out.push_str("export const PROP_KIND = Object.freeze([\n");
    for number in 0..=PropId::ENUM_MAX {
        out.push_str("  ");
        if let Some(kind) = prop_kind(PropId(number)) {
            write!(out, "\"{kind:?}\"").expect("Writing to a String cannot fail");
        } else {
            out.push_str("null");
        }
        out.push_str(",\n");
    }
    out.push_str("]);\n");

    out.push_str("export const WEB_KINDS = Object.freeze([\n");
    for number in 0..=NodeKind::ENUM_MAX {
        if let Some(kind) = WEB_KINDS.iter().find(|row| row.kind.0 == number) {
            out.push_str("  Object.freeze({tag: ");
            quote(&mut out, kind.tag);
            out.push_str(", cls: ");
            quote(&mut out, kind.class);
            out.push_str(", attrs: Object.freeze([");
            for (index, (name, value)) in kind.attrs.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                out.push_str("Object.freeze([");
                quote(&mut out, name);
                out.push_str(", ");
                quote(&mut out, value);
                out.push_str("])");
            }
            out.push_str("])}),\n");
        } else {
            out.push_str("  null,\n");
        }
    }
    out.push_str("]);\n");

    out.push_str("export const WEB_EVENTS = Object.freeze([\n");
    for number in 0..=EventId::ENUM_MAX {
        out.push_str("  ");
        if let Some((_, event)) = WEB_EVENTS.iter().find(|(id, _)| id.0 == number) {
            quote(&mut out, event);
        } else {
            out.push_str("null");
        }
        out.push_str(",\n");
    }
    out.push_str("]);\n");

    export_names(&mut out, "TAG_NAMES", TAG_NAMES);
    export_names(&mut out, "ATTR_NAMES", ATTR_NAMES);
    out.push_str("export const CORE_CSS = ");
    quote(&mut out, CORE_CSS);
    out.push_str(";\n");
    out
}

fn export_names(out: &mut String, export: &str, names: &[&str]) {
    writeln!(out, "export const {export} = Object.freeze([")
        .expect("Writing to a String cannot fail");
    for name in names {
        out.push_str("  ");
        quote(out, name);
        out.push_str(",\n");
    }
    out.push_str("]);\n");
}

fn quote(out: &mut String, value: &str) {
    out.push('"');
    out.extend(value.chars().flat_map(char::escape_default));
    out.push('"');
}
