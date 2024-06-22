#![feature(test)]

extern crate test;

use self::test::{black_box, Bencher};

static HTML: &'static str = include_str!("./wikipedia-2020-12-21.html");

use ewe_html::parsers::{wrap_in_document_fragment_container, HTMLParser};

#[bench]
fn html_parser(b: &mut Bencher) {
    b.iter(|| {
        black_box({
            let parser = HTMLParser::default();
            parser
                .parse(&wrap_in_document_fragment_container(String::from(HTML)))
                .unwrap();
        })
    })
}
