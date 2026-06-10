//! WHY: `IntoHtml`, `Html`, `Part`, and `DomOp` are the public surface the `html!`
//! macro and runtime build on. These tests pin their construction/conversion
//! behaviour from the outside, the same way a consumer crate uses them.
//!
//! WHAT: `IntoHtml` conversions for strings/options/vecs/primitives, `Html`
//! text/element flags, and `DomOp`/`Part` variant construction.
//!
//! HOW: Drives the public API via the crate root.

use foundation_ui_traits::{AttrPart, DomOp, Html, IntoHtml, Part, TextPart};

#[test]
fn into_html_string_produces_text_node() {
    let html = "hello".into_html();
    assert!(html.is_text());
    assert_eq!(html.text.as_deref(), Some("hello"));
}

#[test]
fn into_html_option_some_produces_child() {
    let html = Some(Html::new()).into_html();
    assert!(!html.is_text());
}

#[test]
fn into_html_option_none_produces_empty() {
    let html: Html = None.into_html();
    assert!(html.children.is_empty());
    assert!(html.text.is_none());
}

#[test]
fn into_html_vec_flattens_into_children() {
    let items = vec![Html::new(), Html::new()];
    let html = items.into_html();
    assert_eq!(html.children.len(), 2);
}

#[test]
fn into_html_primitives_produce_text() {
    let html = 42u32.into_html();
    assert_eq!(html.text.as_deref(), Some("42"));

    let html = true.into_html();
    assert_eq!(html.text.as_deref(), Some("true"));

    // A plain (non-PI-like) float literal, so the test stays lint-clean and the
    // expected formatting is unambiguous.
    let html = 2.5f64.into_html();
    assert_eq!(html.text.as_deref(), Some("2.5"));
}

#[test]
fn dom_op_variants_construct_correctly() {
    let op = DomOp::SetText {
        node_id: 1,
        text: "hello".into(),
    };
    match op {
        DomOp::SetText { node_id, text } => {
            assert_eq!(node_id, 1);
            assert_eq!(text, "hello");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn part_variants_construct_correctly() {
    let text_part = Part::Text(TextPart { node_id: 5 });
    match text_part {
        Part::Text(p) => assert_eq!(p.node_id, 5),
        _ => panic!("wrong variant"),
    }

    let attr_part = Part::Attribute(AttrPart {
        node_id: 5,
        attr_name: "class".into(),
    });
    match attr_part {
        Part::Attribute(p) => {
            assert_eq!(p.node_id, 5);
            assert_eq!(p.attr_name, "class");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn html_element_flags() {
    let text = Html {
        text: Some("hi".into()),
        ..Html::new()
    };
    assert!(text.is_text());
    assert!(!text.is_element());

    let el = Html {
        tag: Some("div".into()),
        ..Html::new()
    };
    assert!(!el.is_text());
    assert!(el.is_element());
}
