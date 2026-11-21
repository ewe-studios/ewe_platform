//! Shape and known-row tests for `data/mime.json`.
//!
//! The file is vendored, so these tests never touch the network. They check the
//! shape that the generator promises and the rows that the specification names.

use std::fmt;

use serde::de::{MapAccess, Visitor};

/// The pairs of the data file, in file order. `serde_json::Value` sorts the
/// keys, so the test walks the object itself.
struct Ordered(Vec<(String, String)>);

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Pairs;

        impl<'de> Visitor<'de> for Pairs {
            type Value = Vec<(String, String)>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object of strings")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut pairs = Vec::new();
                while let Some(pair) = map.next_entry::<String, String>()? {
                    pairs.push(pair);
                }
                Ok(pairs)
            }
        }

        deserializer.deserialize_map(Pairs).map(Ordered)
    }
}

fn table() -> Vec<(String, String)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/mime.json");
    let text = std::fs::read_to_string(path).expect("data/mime.json is readable");
    let Ordered(pairs) = serde_json::from_str(&text).expect("data/mime.json is a JSON object");
    pairs
}

fn is_media_type(text: &str) -> bool {
    let Some((kind, subtype)) = text.split_once('/') else {
        return false;
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".+-".contains(&byte)
            })
    };
    valid(kind) && valid(subtype)
}

#[test]
fn every_row_has_a_valid_shape() {
    let pairs = table();
    for (extension, media_type) in &pairs {
        assert!(
            !extension.is_empty()
                && extension.bytes().all(|byte| byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || byte == b'-'
                    || byte == b'_'),
            "extension {extension:?} is not lower-case ASCII without a dot"
        );
        assert!(
            is_media_type(media_type),
            "media type {media_type:?} for {extension:?} is not type/subtype"
        );
    }
}

#[test]
fn keys_are_sorted() {
    let pairs = table();
    for window in pairs.windows(2) {
        assert!(
            window[0].0 < window[1].0,
            "{:?} follows {:?}",
            window[1].0,
            window[0].0
        );
    }
}

#[test]
fn the_file_holds_at_least_one_thousand_two_hundred_rows() {
    assert!(
        table().len() >= 1_200,
        "the table holds fewer than 1,200 rows"
    );
}

#[test]
fn known_rows_hold() {
    let pairs = table();
    let lookup = |extension: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == extension)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(lookup("mjs"), Some("text/javascript"));
    assert_eq!(lookup("js"), Some("text/javascript"));
    assert_eq!(lookup("wasm"), Some("application/wasm"));
    assert_eq!(lookup("map"), Some("application/json"));
    assert_eq!(lookup("md"), Some("text/markdown"));
    assert_eq!(lookup("mp4"), Some("video/mp4"));
    assert_eq!(lookup("mpg4"), Some("video/mp4"));
    assert_eq!(lookup("ts"), Some("video/mp2t"));
}
