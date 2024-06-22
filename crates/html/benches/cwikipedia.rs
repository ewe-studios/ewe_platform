#![feature(test)]

extern crate test;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

static HTML: &'static str = include_str!("./wikipedia-2020-12-21.html");

use ewe_html::parsers::{wrap_in_document_fragment_container, HTMLParser};

fn html_parser(c: &mut Criterion) {
    c.bench_function("wikipedia_blackbox", |b| {
        b.iter(|| {
            black_box({
                let parser = HTMLParser::default();
                parser
                    .parse(&wrap_in_document_fragment_container(HTML.to_string()))
                    .unwrap();
            });
        });
    });
}

fn html_parser_no_blackbox(c: &mut Criterion) {
    c.bench_function("wikipedia_no_blackbox", |b| {
        b.iter(|| {
            let parser = HTMLParser::default();
            parser
                .parse(&wrap_in_document_fragment_container(HTML.to_string()))
                .unwrap();
        });
    });
}

fn html_parser_with_svg(c: &mut Criterion) {
    c.bench_function("html_svg", |b| {
        b.iter(|| {
            let parser = HTMLParser::default();
            parser
                .parse(&wrap_in_document_fragment_container(String::from(
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
                    )))
                .unwrap();
            });
    });
}

criterion_group!(
    benches,
    html_parser,
    html_parser_no_blackbox,
    html_parser_with_svg
);
criterion_main!(benches);
