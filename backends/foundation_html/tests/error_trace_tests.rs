//! Feature 67: the public fallible APIs return an errstacks trace.

use foundation_html::markup::{ElementError, ElementResult};
use foundation_html::parsers::{HTMLParser, ParsingResult, ParsingTagError};

fn parse_error() -> ParsingResult<()> {
    let parser = HTMLParser::default();
    let data = foundation_html::parsers::wrap_in_document_fragment_container(String::from(
        "<div>hello<</div>",
    ));
    parser.parse(data.as_str()).map(|_| ())
}

#[test]
fn parser_error_is_an_errstacks_trace_with_parser_context() {
    let trace = parse_error().expect_err("malformed markup is rejected");

    assert!(
        matches!(
            trace.current_context(),
            ParsingTagError::FailedContentParsing(_)
        ),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let trace = parse_error()
        .expect_err("malformed markup is rejected")
        .attach("template=index.html")
        .attach("stack=body");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("template=index.html"),
        "template context lost: {rendered}"
    );
    assert!(
        rendered.contains("stack=body"),
        "stack context lost: {rendered}"
    );
}

#[test]
fn element_error_is_an_errstacks_trace_with_element_context() {
    fn fail() -> ElementResult<()> {
        Err(ElementError::NotUsable.into())
    }

    let trace = fail().expect_err("the element is not usable");
    assert!(matches!(trace.current_context(), ElementError::NotUsable));
}
