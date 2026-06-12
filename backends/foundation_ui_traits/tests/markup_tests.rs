//! WHY: `to_markup` is the server leg of the one-rendering-contract rule
//! (spec-42 feature 00 §5) — its escaping and void handling must hold
//! without the macro in the picture.
//!
//! WHAT: Serialization unit tests over hand-built `Html` values: escaping
//! in text vs attributes, void elements, tagless flattening, custom
//! elements, and the corruption marker for out-of-table ids.

use foundation_ui_traits::{AttrName, Html, HtmlTag};

fn element(tag: &'static str) -> Html {
    Html {
        tag: Some(HtmlTag::from_static(tag)),
        ..Html::new()
    }
}

#[test]
fn text_escapes_amp_lt_gt() {
    let mut div = element("div");
    div.children.push(Html::text("a < b & c > d"));
    assert_eq!(div.to_markup(), "<div>a &lt; b &amp; c &gt; d</div>");
}

#[test]
fn attributes_quote_and_escape() {
    let mut a = element("a");
    a.attributes.push((
        AttrName::from_static("href"),
        "/q?x=1&y=\"z\"".into(),
    ));
    assert_eq!(a.to_markup(), "<a href=\"/q?x=1&amp;y=&quot;z&quot;\"></a>");
}

#[test]
fn void_elements_have_no_closing_tag() {
    let mut div = element("div");
    div.children.push(element("br"));
    div.children.push(element("img"));
    assert_eq!(div.to_markup(), "<div><br><img></div>");
}

#[test]
fn tagless_nodes_flatten() {
    let mut wrapper = Html::new();
    wrapper.children.push(element("li"));
    wrapper.children.push(element("li"));
    let mut ul = element("ul");
    ul.children.push(wrapper);
    assert_eq!(ul.to_markup(), "<ul><li></li><li></li></ul>");
}

#[test]
fn custom_elements_serialize_by_name() {
    let mut el = Html {
        tag: Some(HtmlTag::from_static("my-widget")),
        ..Html::new()
    };
    el.text = Some("x".into());
    assert_eq!(el.to_markup(), "<my-widget>x</my-widget>");
}

#[test]
fn out_of_table_id_is_a_visible_marker() {
    let el = Html {
        tag: Some(HtmlTag::Id(u16::MAX)),
        ..Html::new()
    };
    assert_eq!(el.to_markup(), "<unknown-tag></unknown-tag>");
}
