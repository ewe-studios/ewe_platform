//! Check typed delivery and compatibility with other view messages.
use foundation_ui_runtime::action::{RawInput, ViewData};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Count(u32);

impl ViewData for Count {
    const TOPIC: &'static str = "sample.count";
}

fn event(bytes: &[u8]) -> RawInput {
    RawInput::Json(serde_json::to_vec(bytes).unwrap())
}

#[test]
fn accepts_its_topic_and_ignores_other_messages() {
    let bytes = rmp_serde::to_vec_named(&(Count::TOPIC, Count(4))).unwrap();
    assert_eq!(Count::decode_input(event(&bytes)).unwrap(), Some(Count(4)));
    let other = rmp_serde::to_vec_named(&("sample.name", "name")).unwrap();
    assert_eq!(Count::decode_input(event(&other)).unwrap(), None);
    let signal = rmp_serde::to_vec("open:settings").unwrap();
    assert_eq!(Count::decode_input(event(&signal)).unwrap(), None);
}

#[test]
fn rejects_invalid_values_and_trailing_bytes_on_its_topic() {
    let malformed = rmp_serde::to_vec_named(&(Count::TOPIC, "wrong")).unwrap();
    assert!(Count::decode_input(event(&malformed)).is_err());
    let mut trailing = rmp_serde::to_vec_named(&(Count::TOPIC, Count(4))).unwrap();
    trailing.push(0);
    assert!(Count::decode_input(event(&trailing)).is_err());
}
