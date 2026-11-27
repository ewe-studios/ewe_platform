use std::{fs, path::PathBuf};

use foundation_ui_wire::{
    vocab::{vocab_js, CORE_CSS, WEB_EVENTS, WEB_KINDS},
    EventId, NodeKind,
};

#[test]
fn js_file_matches_tables() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../foundation_wasm_ui/runtimes/foundation-ui-vocab.js");
    let expected = vocab_js();
    if std::env::var_os("UPDATE_FIXTURES").as_deref() == Some(std::ffi::OsStr::new("1")) {
        fs::write(&path, &expected).unwrap();
    }
    assert_eq!(fs::read_to_string(path).unwrap(), expected);
}

#[test]
fn web_mapping_matches_the_core_names() {
    let rows = [
        (NodeKind::STACK, "div", "ewe-stack", &[][..]),
        (NodeKind::TEXT, "span", "ewe-text", &[][..]),
        (NodeKind::IMAGE, "img", "ewe-image", &[][..]),
        (
            NodeKind::BUTTON,
            "button",
            "ewe-button",
            &[("type", "button")][..],
        ),
        (
            NodeKind::TEXT_INPUT,
            "input",
            "ewe-text-input",
            &[("type", "text")][..],
        ),
        (
            NodeKind::TOGGLE,
            "input",
            "ewe-toggle",
            &[("type", "checkbox"), ("role", "switch")][..],
        ),
        (NodeKind::LIST, "div", "ewe-list", &[("role", "list")][..]),
        (NodeKind::SCROLL, "div", "ewe-scroll", &[][..]),
        (
            NodeKind::SPACER,
            "div",
            "ewe-spacer",
            &[("aria-hidden", "true")][..],
        ),
        (NodeKind::OVERLAY, "div", "ewe-overlay", &[][..]),
        (NodeKind::MODAL, "dialog", "ewe-modal", &[][..]),
        (
            NodeKind::BOTTOM_SHEET,
            "section",
            "ewe-bottom-sheet",
            &[][..],
        ),
        (
            NodeKind::NAVIGATION_TITLE,
            "h1",
            "ewe-navigation-title",
            &[][..],
        ),
        (
            NodeKind::TOOLBAR_BUTTON,
            "button",
            "ewe-toolbar-button",
            &[("type", "button")][..],
        ),
        (
            NodeKind::SEGMENTED_CONTROL,
            "div",
            "ewe-segmented-control",
            &[("role", "group")][..],
        ),
        (NodeKind::INSET_LIST, "ul", "ewe-inset-list", &[][..]),
        (NodeKind::CHECKLIST_ROW, "li", "ewe-checklist-row", &[][..]),
        (
            NodeKind::CHECKMARK_BUTTON,
            "button",
            "ewe-checkmark-button",
            &[("type", "button")][..],
        ),
        (
            NodeKind::DESTRUCTIVE_BUTTON,
            "button",
            "ewe-destructive-button",
            &[("type", "button")][..],
        ),
        (NodeKind::LIST_FOOTER, "p", "ewe-list-footer", &[][..]),
        (
            NodeKind::CONFIRMATION_DIALOG,
            "dialog",
            "ewe-confirmation-dialog",
            &[("role", "alertdialog")][..],
        ),
        (NodeKind::SHEET_TITLE, "h2", "ewe-sheet-title", &[][..]),
        (
            NodeKind::SUBMIT_BUTTON,
            "button",
            "ewe-submit-button",
            &[("type", "submit")][..],
        ),
        (
            NodeKind::CANCEL_BUTTON,
            "button",
            "ewe-cancel-button",
            &[("type", "button")][..],
        ),
        (NodeKind::DRAWING_SURFACE, "canvas", "ewe-drawing-surface", &[][..]),
        (
            NodeKind::SEARCH_INPUT,
            "input",
            "ewe-search-input",
            &[("type", "search")][..],
        ),
    ];
    assert_eq!(WEB_KINDS.len(), rows.len());
    for (row, expected) in WEB_KINDS.iter().zip(rows) {
        assert_eq!((row.kind, row.tag, row.class, row.attrs), expected);
    }
}

#[test]
fn web_tables_cover_all_kinds_and_events() {
    assert_eq!(
        WEB_KINDS.iter().map(|row| row.kind).collect::<Vec<_>>(),
        &NodeKind::ENUM_VALUES[1..],
    );
    assert_eq!(
        WEB_EVENTS,
        [
            (EventId::TAP, "click"),
            (EventId::CHANGE, "change"),
            (EventId::SUBMIT, "submit"),
            (EventId::FOCUS, "focus"),
            (EventId::BLUR, "blur"),
        ],
    );
    assert_eq!(
        WEB_EVENTS.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        &EventId::ENUM_VALUES[1..],
    );
    for row in WEB_KINDS {
        if [
            NodeKind::BUTTON,
            NodeKind::OVERLAY,
            NodeKind::MODAL,
            NodeKind::BOTTOM_SHEET,
        ]
        .contains(&row.kind)
        {
            assert!(CORE_CSS.contains(row.class));
        }
    }
}
