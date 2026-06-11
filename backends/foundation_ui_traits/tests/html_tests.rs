//! WHY: `IntoHtml` is the contract the `html!` macro and the signal system lean
//! on — every `{expr}` goes through it, so each implementation's exact output
//! shape (tag/text/children) must be pinned.
//!
//! WHAT: Feature 01 spec section 10, tests 1-10 (`IntoHtml`), plus the
//! `HtmlTag`/`AttrName` wire-id contract the encoders depend on.

use std::borrow::Cow;

use foundation_ui_traits::{
    AttrName, Html, HtmlTag, IntoHtml, ATTR_CLASS, ATTR_ID, ATTR_NAMES, ATTR_STYLE, ATTR_VALUE,
    TAG_BUTTON, TAG_DIV, TAG_INPUT, TAG_NAMES, TAG_SPAN,
};

// ─── Tests 1-10: IntoHtml ──────────────────────────────────────────────────────

/// Test 1 — `Html` identity.
#[test]
fn html_into_html_is_identity() {
    let html = Html {
        tag: Some(HtmlTag::Id(TAG_DIV)),
        attributes: vec![(AttrName::Id(ATTR_CLASS), Cow::Borrowed("main"))],
        children: vec![Html::text("hi")],
        text: None,
        parts: vec![],
    };
    assert_eq!(html.clone().into_html(), html);
}

/// Test 2 — `&str` becomes a text node.
#[test]
fn str_into_html_is_text_node() {
    let html = "hello".into_html();
    assert_eq!(html.text.as_deref(), Some("hello"));
    assert_eq!(html.tag, None);
    assert!(html.is_text());
    assert!(!html.is_element());
}

/// Test 3 — `String` has the same structure as `&str`.
#[test]
fn string_into_html_matches_str() {
    assert_eq!(String::from("world").into_html(), "world".into_html());
}

/// Test 4 — `42u32` renders its Display output.
#[test]
fn u32_into_html() {
    assert_eq!(42u32.into_html().text.as_deref(), Some("42"));
}

/// Test 5 — `true` renders as-is.
#[test]
fn bool_into_html() {
    assert_eq!(true.into_html().text.as_deref(), Some("true"));
}

/// Test 6 — floats render their Display output.
#[test]
fn f64_into_html() {
    assert_eq!(2.5f64.into_html().text.as_deref(), Some("2.5"));
}

/// Test 7 — all 13 primitives produce a text node with the Display output.
#[test]
fn all_primitives_into_html() {
    macro_rules! check {
        ($($value:expr),*) => {
            $(
                let html = $value.into_html();
                assert_eq!(html.text.as_deref(), Some($value.to_string().as_str()));
                assert_eq!(html.tag, None);
                assert!(html.children.is_empty());
            )*
        };
    }
    check!(
        7usize, -7isize, 8u8, 16u16, 32u32, 64u64, -8i8, -16i16, -32i32, -64i64, false, 1.5f32,
        2.5f64
    );
}

/// Test 8 — `Some(html)` returns the inner Html.
#[test]
fn some_html_unwraps() {
    let inner = Html {
        tag: Some(HtmlTag::Id(TAG_SPAN)),
        ..Html::new()
    };
    assert_eq!(Some(inner.clone()).into_html(), inner);
}

/// Test 9 — `None::<Html>` produces an empty node.
#[test]
fn none_html_is_empty_node() {
    let html = None::<Html>.into_html();
    assert_eq!(html, Html::new());
    assert!(!html.is_text());
    assert!(!html.is_element());
}

/// Test 10 — `vec![a, b]` wraps as two children of a tagless node.
#[test]
fn vec_html_wraps_as_children() {
    let a = Html::text("a");
    let b = Html::text("b");
    let html = vec![a.clone(), b.clone()].into_html();
    assert_eq!(html.tag, None);
    assert_eq!(html.text, None);
    assert_eq!(html.children, vec![a, b]);
}

// ─── HtmlTag / AttrName wire identity ──────────────────────────────────────────

/// The four pinned tag ids are ABI — the JS table mirrors them by position.
#[test]
fn pinned_tag_ids_hold() {
    assert_eq!(HtmlTag::from_name("div"), HtmlTag::Id(TAG_DIV));
    assert_eq!(HtmlTag::from_name("span"), HtmlTag::Id(TAG_SPAN));
    assert_eq!(HtmlTag::from_name("input"), HtmlTag::Id(TAG_INPUT));
    assert_eq!(HtmlTag::from_name("button"), HtmlTag::Id(TAG_BUTTON));
    assert_eq!(TAG_NAMES[0], "div");
    assert_eq!(TAG_NAMES[3], "button");
}

/// The four pinned attribute ids are ABI.
#[test]
fn pinned_attr_ids_hold() {
    assert_eq!(AttrName::from_name("class"), AttrName::Id(ATTR_CLASS));
    assert_eq!(AttrName::from_name("id"), AttrName::Id(ATTR_ID));
    assert_eq!(AttrName::from_name("style"), AttrName::Id(ATTR_STYLE));
    assert_eq!(AttrName::from_name("value"), AttrName::Id(ATTR_VALUE));
    assert_eq!(ATTR_NAMES[0], "class");
    assert_eq!(ATTR_NAMES[3], "value");
}

/// Every table entry round-trips name -> id -> name; ids are dense from 1.
#[test]
fn tag_and_attr_tables_round_trip() {
    for (i, name) in TAG_NAMES.iter().enumerate() {
        let tag = HtmlTag::from_name(name);
        assert_eq!(tag, HtmlTag::Id(u16::try_from(i + 1).unwrap()), "{name}");
        assert_eq!(tag.name(), Some(*name));
    }
    for (i, name) in ATTR_NAMES.iter().enumerate() {
        let attr = AttrName::from_name(name);
        assert_eq!(attr, AttrName::Id(u16::try_from(i + 1).unwrap()), "{name}");
        assert_eq!(attr.name(), Some(*name));
    }
}

/// Tables contain no duplicate names (a duplicate would shadow the later id).
#[test]
fn tables_have_no_duplicates() {
    for table in [TAG_NAMES, ATTR_NAMES] {
        let mut seen = std::collections::HashSet::new();
        for name in table {
            assert!(seen.insert(*name), "duplicate table entry: {name}");
        }
    }
}

/// Unknown names fall back to the `Name` form and survive the wire format.
#[test]
fn unknown_names_round_trip_the_wire_form() {
    let custom = HtmlTag::from_name("my-element");
    assert_eq!(custom, HtmlTag::Name(Cow::Borrowed("my-element")));
    assert_eq!(custom.to_wire_string(), "my-element");
    assert_eq!(HtmlTag::from_wire_str("my-element"), custom);

    let known = HtmlTag::Id(TAG_DIV);
    assert_eq!(known.to_wire_string(), "id:1");
    assert_eq!(HtmlTag::from_wire_str("id:1"), known);

    // A literal name that LOOKS like the prefix but isn't a valid id stays a name.
    assert_eq!(
        AttrName::from_wire_str("id:notanumber"),
        AttrName::Name(Cow::Owned("id:notanumber".into()))
    );
}

/// `name()` returns `None` for out-of-table ids (wire corruption signal), and
/// Display falls back gracefully.
#[test]
fn out_of_table_id_has_no_name() {
    let bogus = HtmlTag::Id(60000);
    assert_eq!(bogus.name(), None);
    assert_eq!(bogus.to_string(), "id:?");
    assert_eq!(HtmlTag::Id(TAG_DIV).to_string(), "div");
}
