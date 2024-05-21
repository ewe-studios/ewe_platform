use std::borrow::Cow;
use std::str;

pub trait Encoder {
    fn encode<'a>(&self, text: &'a str) -> Cow<'a, [u8]>;
}

pub trait Decoder {
    fn decode<'a>(&self, text: &'a [u8]) -> &'a str;
}

pub trait Encoding: Encoder + Decoder {}
