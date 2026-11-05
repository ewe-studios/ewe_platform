//! HTML parser tests moved out of `src/parsers.rs` (spec 70 feature 59,
//! rule 7). Every item the tests use is public: `HTMLParser`,
//! `wrap_in_document_fragment_container`, and the `Stack`/`AttrValue` types.

use foundation_html::parsers::*;

#[cfg(test)]
mod html_parser_test {

    use super::*;
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_empty_string() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::new());
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(54),
                attrs: vec![],
                children: vec![]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_with_double_quoted_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype lang=\"en\">"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(74),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    start_range: Some(29),
                    end_range: Some(36),
                    attrs: vec![("lang", AttrValue::Text("en"))],
                    children: vec![]
                }]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_single_quoted_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype lang='en'>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(74),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    start_range: Some(29),
                    end_range: Some(36),
                    attrs: vec![("lang", AttrValue::Text("en"))],
                    children: vec![]
                }]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_alphaneumeric_only_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype lang=en>"));
        let result = parser.parse(data.as_str());

        tracing::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(72),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    end_range: Some(36),
                    start_range: Some(29),
                    attrs: vec![("lang", AttrValue::Text("en"))],
                    children: vec![]
                }]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_doctype() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(64),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    start_range: Some(29),
                    end_range: Some(36),
                    attrs: vec![],
                    children: vec![]
                }]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_with_spaced_tag_starter() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello< </div>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(72),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::HTML(HTMLTags::Div)),
                    closed: true,
                    start_range: Some(28),
                    end_range: Some(44),
                    attrs: vec![],
                    children: vec![Stack {
                        tag: Some(MarkupTags::Text("hello< ".to_string())),
                        closed: false,
                        attrs: vec![],
                        children: vec![],
                        start_range: Some(32),
                        end_range: Some(39),
                    }]
                }]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello</div>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(70),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::HTML(HTMLTags::Div)),
                    closed: true,
                    start_range: Some(28),
                    end_range: Some(42),
                    attrs: vec![],
                    children: vec![Stack {
                        tag: Some(MarkupTags::Text("hello".to_string())),
                        closed: false,
                        attrs: vec![],
                        children: vec![],
                        start_range: Some(32),
                        end_range: Some(37),
                    }]
                }]
            }
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_with_tag_starter_in_text() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello<</div>"));
        let result = parser.parse(data.as_str());

        assert!(matches!(
            result,
            ParsingResult::Err(ParsingTagError::FailedContentParsing(_))
        ));
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_special_characters() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello:_-</div>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Text(String::from("hello:_-")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_multiple_children_with_self_closing_br_and_circle(
    ) {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div><circle/></div><div><br/></div>",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_invalid_html_parsing_multiple_breakpoint_tags_with_sibling_with_bad_closer() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
                <circle/>
            </div>
            <div>
            <br>
            <br>
            </section>
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Err(_)));
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_multiple_breakpoint_tags_with_sibling_with_no_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
                <circle/>
            </div>
            <br>
            <br>
            <div>
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Div),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_multiple_breakpoint_tags_as_children() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
            <circle/>
            </div>
            <div>
            <br>
            <br>
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_multiple_breakpoint_tags() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
            <circle/>
            </div>
            <div>
            <br/>
            <br/>
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_multiple_children_with_self_closing_br() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div><circle></circle></div><div><br/></div>",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_multiple_children() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div><circle></circle></div><div><br></div>",
        ));
        let result = parser.parse(data.as_str());

        tracing::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();
        tracing::debug!("ParsedTags: {:?}", parsed);

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_html_with_script_text_element() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"<div>
                <script>
                  let some_var = window.get('alex');
                    let elem = (<section>alex</section>)
                </script>
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Script),
                MarkupTags::Text(String::from("\n                  let some_var = window.get('alex');\n                    let elem = (<section>alex</section>)\n                ")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_comments_with_extended_ender() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"<div>
                <!-- This is a comment--->
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from("<!-- This is a comment--->")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_comments_with_character() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"<div>
                <!-- This is a comment-->
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from("<!-- This is a comment-->")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_comments() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"<div>
                <!-- This is a comment -->
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from("<!-- This is a comment -->")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_multiline_comments() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"<div>
                <!--
                    This is a comment
                -->
            </div>
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from(
                    "<!--\n                    This is a comment\n                -->"
                ))
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_with_section_tag_with_at_sign_self_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
                <sec@tion />
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::Component(String::from("sec@tion")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_with_section_tag_self_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
                <section />
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Section),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_with_div_tag_self_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
                <div />
            ",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_xml_starter() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <?xml version="1.0" ?>
            "#,
        ));
        let result = parser.parse(data.as_str());

        tracing::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Xml),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_style_tag() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
              <style type="text/css">
                  body {
                      background: black;
                    }

                    h1 {
                      color: white;
                        background-color: #43433;
                    }
                </style>
            "#,
        ));
        let result = parser.parse(data.as_str());

        tracing::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Style),
                MarkupTags::Text(String::from("\n                  body {\n                      background: black;\n                    }\n\n                    h1 {\n                      color: white;\n                        background-color: #43433;\n                    }\n                ")),
            ],
            parsed
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_variation_with_double_curly_bracket() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div jail={{some of other vaulue}}>hello</div>",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.first().unwrap();

        assert_eq!(
            *div.attrs.first().unwrap(),
            ("jail", AttrValue::Code("{{some of other vaulue}}"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_variation_with_curly_bracket() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div jail={some of other vaulue}>hello</div>",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.first().unwrap();

        assert_eq!(
            *div.attrs.first().unwrap(),
            ("jail", AttrValue::Block("{some of other vaulue}"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_can_handle_non_english_characters() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
                     <html lang=\"sv\">
                        <head>
                            <title>Här kan man va</title>
                        </head>
                        <body>
                          <boäk näme=20 age="äry" flag={ämu}>Hi</boäk>
                            <h1>Tjena världen!</h1>
                            <p>Tänkte bara informera om att Sverige är bättre än Finland i ishockey.</p>
                        </body>
                    </html>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    // The samples are one table of raw HTML. Splitting the list hides which
    // documents the test covers, so the length lint is wrong for this item.
    #[allow(clippy::too_many_lines)]
    fn valid_samples() -> Vec<String> {
        vec![
            wrap_in_document_fragment_container(String::from(
                r#"
        <svg width="600" height="600">
            <rect id="rec" x="300" y="100" width="300" height="100" style="fill:lime">
            <animate attributeName="x" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="300" to="0" />
            <animate attributeName="y" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="100" to="0" />
            <animate attributeName="width" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="300" to="800" />
            <animate attributeName="height" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="100" to="300" />
            <animate attributeName="fill" attributeType="CSS" from="lime" to="red" begin="2s" dur="4s" fill="freeze" />
            </rect>
            <g transform="translate(100,100)">
            <text id="TextElement" x="0" y="0" style="font-family:Verdana;font-size:24; visibility:hidden"> It's SVG!
                <set attributeName="visibility" attributeType="CSS" to="visible" begin="1s" dur="5s" fill="freeze" />
                <animateMotion path="M 0 0 L 100 100" begin="1s" dur="5s" fill="freeze" />
                <animate attributeName="fill" attributeType="CSS" from="red" to="blue" begin="1s" dur="5s" fill="freeze" />
                <animateTransform attributeName="transform" attributeType="XML" type="rotate" from="-30" to="0" begin="1s" dur="5s" fill="freeze" />
                <animateTransform attributeName="transform" attributeType="XML" type="scale" from="1" to="3" additive="sum" begin="1s" dur="5s" fill="freeze" />
            </text>
            </g>
            Sorry, your browser does not support inline SVG.
        </svg>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1.0">
                    <title>Document</title>
                    <style>
                        body {
                            background: black;
                        }

                        h1 {
                            color: white;
                        }
                    </style>
                </head>
                <body>
                    <h1>Hello world</h1>
                    <!-- There should be more text here -->
                    <script>
                        const title = document.querySelector("h1")
                        title.innerText = "Hello from script"
                    </script>
                </body>
            </html>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
                     <html lang=\"sv\">
                        <head>
                            <title>Här kan man va</title>
                        </head>
                        <body>
                            <h1>Tjena världen!</h1>
                            <p>Tänkte bara informera om att Sverige är bättre än Finland i ishockey.</p>
                        </body>
                    </html>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r"
                <HTML>
                    <head>
                        <title>title</title>
                    </head>
                    <body>
                        <ul>
                            <li></li>
                            <li></li>
                            <li></li>
                        </ul>
                    </body>
                </hTML>
            ",
            )),
            wrap_in_document_fragment_container(String::from(
                r"
                    <div class='1'>
                        <div class='1'>
                            <div class='1'>
                                <div class='1'>
                                    <div class='1'>
                                        <div class='1'>
                                            <div class='1'>
                                                <div class='1'>
                                                    <!--this is deep-->
                                                    hello world
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
            ",
            )),
            wrap_in_document_fragment_container(String::from(
                r"
                    <script>
                        const person_creator = ({ name, symtoms }) => {
                            let person = {}
                            person.name = name
                            person.symtoms = {}
                            for (symtom of symtoms) {
                                person.symtoms[symtom] = true
                            }
                            return person
                        }

                        const main = () => {
                            let name = 'mathias'
                            let symtoms = ['Dunning-Kruger', 'ACDC', 'Slacker']

                            setTimeout(() => {
                                let person = person_creator({ name, symtoms })
                                if (person.symtoms.hasOwnProperty('Dunning-Kruger')) {
                                    console.log('yeah buddy, that\'s right')
                                }
                            }, 1337)
                        }

                        main()
                    </script>
            ",
            )),
            wrap_in_document_fragment_container(String::from(
                r"
                  <!-- comment -->
                    <!-- comment -->
                    <!DOCTYPE html>
                    <!-- comment -->
                    <!-- comment -->
                    <html>
                    <!-- comment -->
                    </html>
                    <!-- comment -->
                    <!-- comment -->
            ",
            )),
            wrap_in_document_fragment_container(String::from(
                r"
        hello<!--world?--><div/>
            ",
            )),
            wrap_in_document_fragment_container(String::from(
                r"
        hello<!--world?--><div/>
            ",
            )),
        ]
    }

    #[traced_test]
    #[test]
    fn test_can_parse_different_samples_that_should_be_valid() {
        let parser = HTMLParser::default();

        for test_case in valid_samples() {
            let result = parser.parse(test_case.as_str());
            assert!(matches!(result, ParsingResult::Ok(_)));
        }
    }

    static HTML: &str = include_str!("../benches/wikipedia-2020-12-21.html");

    #[traced_test]
    #[test]
    fn test_html_can_handle_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(HTML.to_string());
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    static HTML_BIG: &str = include_str!("../benches/wikipedia_on_wikipedia.html");

    #[traced_test]
    #[test]
    fn test_html_can_handle_big_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(HTML_BIG.to_string());
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    static HTML_SMALLEST: &str = include_str!("../benches/scraping_course.html");

    #[traced_test]
    #[test]
    fn test_html_can_handle_smallest_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(HTML_SMALLEST.to_string());
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_with_rust_code_block() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div jail={{some of other vaulue}} rust={{{some rust value}}}>hello</div>",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.first().unwrap();

        assert_eq!(
            *div.attrs.first().unwrap(),
            ("jail", AttrValue::Code("{{some of other vaulue}}"))
        );

        assert_eq!(
            *div.attrs.get(1).unwrap(),
            ("rust", AttrValue::Rust("{{{some rust value}}}"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_with_rust_with_other_attributes_and_multiline_code_block() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div jail={{
                some of other vaulue
            }} name={name} rust={{{
             some rust value
             }}} addr="supply">hello</div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.first().unwrap();

        assert_eq!(
            *div.attrs.first().unwrap(),
            (
                "jail",
                AttrValue::Code("{{\n                some of other vaulue\n            }}")
            )
        );

        assert_eq!(
            *div.attrs.get(1).unwrap(),
            ("name", AttrValue::Block("{name}"))
        );

        assert_eq!(
            *div.attrs.get(2).unwrap(),
            (
                "rust",
                AttrValue::Rust("{{{\n             some rust value\n             }}}")
            )
        );

        assert_eq!(
            *div.attrs.get(3).unwrap(),
            ("addr", AttrValue::Text("supply"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_with_rust_multiline_code_block() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div jail={{
                some of other vaulue
            }} rust={{{
             some rust value
             }}}>hello</div>
            ",
        ));
        let result = parser.parse(data.as_str());
        println!("Results: {result:?}");
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.first().unwrap();

        assert_eq!(
            *div.attrs.first().unwrap(),
            (
                "jail",
                AttrValue::Code("{{\n                some of other vaulue\n            }}")
            )
        );

        assert_eq!(
            *div.attrs.get(1).unwrap(),
            (
                "rust",
                AttrValue::Rust("{{{\n             some rust value\n             }}}")
            )
        );
    }

    #[traced_test]
    #[test]
    fn test_html_can_customize_what_is_block_text_tag() {
        let parser = HTMLParser::with_custom_block_text_tags(|tag| tag.to_str().unwrap() == "div");

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
              <section></section>
             </div>
            ",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();

        let _ = parsed.children.first().unwrap();

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Text(String::from(
                    "\n              <section></section>\n             "
                )),
            ],
            tags
        );
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_code_block_in_body_and_as_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div jail={{
                some of other vaulue
            }}>
             {{
                some of other vaulue
             }}
             </div>
            ",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();

        let div = parsed.children.first().unwrap();

        assert_eq!(
            *div.attrs.first().unwrap(),
            (
                "jail",
                AttrValue::Code("{{\n                some of other vaulue\n            }}")
            )
        );

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Code(String::from(
                    "{{\n                some of other vaulue\n             }}"
                )),
            ],
            tags
        );
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_code_block_in_body() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
             {{
                some of other vaulue
             }}
             </div>
            ",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let _ = parsed.children.first().unwrap();

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Code(String::from(
                    "{{\n                some of other vaulue\n             }}"
                )),
            ],
            tags
        );
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_multiple_code_block_in_body() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
             {{
                some of other vaulue
             }}
             {{{
             some rust value
             }}}
             </div>
            ",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let _ = parsed.children.first().unwrap();

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Code(String::from(
                    "{{\n                some of other vaulue\n             }}"
                )),
                MarkupTags::Rust(String::from(
                    "{{{\n             some rust value\n             }}}"
                )),
            ],
            tags
        );
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_multiple_code_block_in_body_with_bad_closer() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r"
            <div>
             {{
                some of other vaulue
             }}
             {{{
             some rust value
             }} }
             </div>
            ",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Err(_)));
    }
}
