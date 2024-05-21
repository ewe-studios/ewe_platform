use std::borrow::Cow;
use std::str;

pub trait Encoder {
    fn encode<'a>(&self, text: &'a str) -> Cow<'a, [u8]>;
}

pub trait Decoder {
    fn decode<'a>(&self, text: &'a [u8]) -> &'a str;
}

pub trait Encoding: Encoder + Decoder {}

pub struct UTF8Encoding;

impl Encoder for UTF8Encoding {
    fn encode<'a>(&self, text: &'a str) -> Cow<'a, [u8]> {
        Cow::from(text.as_bytes())
    }
}

impl Decoder for UTF8Encoding {
    fn decode<'a>(&self, text: &'a [u8]) -> &'a str {
        str::from_utf8(text).expect("should be utf8 string")
    }
}
