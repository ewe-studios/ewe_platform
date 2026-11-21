use foundation_ui_http::{http, ViewRequest, ViewSession};
use foundation_ui_http::{Html, Response, ViewError};
use std::io::{self, Write};

fn session() -> ViewSession {
    let request = http::SimpleIncomingRequest::builder()
        .with_method(http::SimpleMethod::GET)
        .with_plain_url("/")
        .build()
        .unwrap();
    ViewRequest::new(&http::Ctx::host(), request)
        .unwrap()
        .session()
}

fn element(tag: &'static str, attrs: &[(&'static str, &'static str)], children: Vec<Html>) -> Html {
    Html {
        tag: Some(tag.into()),
        attributes: attrs
            .iter()
            .map(|&(key, value)| (key.into(), value.into()))
            .collect(),
        children,
        ..Html::new()
    }
}

#[test]
fn shared_controls_use_browser_roles_and_bindings() {
    let toggle = element(
        "Toggle",
        &[
            ("checked", "true"),
            ("enabled", "false"),
            ("ewe:on:change", "server:toggle"),
        ],
        vec![],
    );
    let list = element(
        "List",
        &[("spacing", "5"), ("axis", "horizontal")],
        vec![toggle],
    );
    let mut output = Vec::new();
    session()
        .body(Response::html(list))
        .write_body(&mut output)
        .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.starts_with("<div class=\"ewe-list\" role=\"list\""));
    assert!(text.contains("style=\"gap:5px;flex-direction:row;\""));
    assert!(text.contains("<input class=\"ewe-toggle\" type=\"checkbox\" role=\"switch\""));
    assert!(text.contains("checked=\"\" disabled=\"\" primal:onchange=\"server:toggle\">"));
    assert!(!text.contains("</input>"));
}

#[test]
fn html_writes_escaped_text_and_attributes_through_short_writes() {
    struct Short(Vec<u8>);
    impl Write for Short {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = bytes.len().min(2);
            self.0.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let view = element(
        "p",
        &[("title", "Fish & \"chips\"")],
        vec![Html::text("é < & >")],
    );
    let mut sink = Short(Vec::new());
    session()
        .body(Response::html(view))
        .write_body(&mut sink)
        .unwrap();
    assert_eq!(
        String::from_utf8(sink.0).unwrap(),
        "<p title=\"Fish &amp; &quot;chips&quot;\">é &lt; &amp; &gt;</p>"
    );
}

#[test]
fn invalid_control_properties_and_html_names_fail() {
    for tree in [
        element("Text", &[("hidden", "sometimes")], vec![]),
        element("p", &[("two names", "")], vec![]),
        element("p>", &[], vec![]),
        element("p", &[("primal::state", "unchecked")], vec![]),
    ] {
        let mut output = Vec::new();
        assert!(matches!(
            session().body(Response::html(tree)).write_body(&mut output),
            Err(ViewError::Render(_))
        ));
        assert_eq!(output.len(), 0);
    }
}
