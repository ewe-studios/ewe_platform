use std::borrow::Cow;
use std::fmt::{self, Debug};
use std::ops::Deref;
use std::{slice, str};

use super::encoding;

pub trait Align {
    fn align(&mut self, offset: usize);
}

impl<T: Align> Align for Vec<T> {
    #[inline]
    fn align(&mut self, offset: usize) {
        for item in self.iter_mut() {
            item.align(offset);
        }
    }
}

impl<T: Align> Align for Option<T> {
    #[inline]
    fn align(&mut self, offset: usize) {
        if let Some(val) = self {
            val.align(offset);
        }
    }
}

impl Align for usize {
    #[inline]
    fn align(&mut self, offset: usize) {
        if *self >= offset {
            *self -= offset;
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl Align for Range {
    fn align(&mut self, offset: usize) {
        self.start.align(offset);
        self.end.align(offset);
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Bytes<'b>(Cow<'b, [u8]>);

impl<'b> Bytes<'b> {
    #[inline]
    #[must_use] 
    pub fn from_slice(slice: &'b [u8]) -> Bytes<'b> {
        Bytes(Cow::from(slice))
    }

    #[inline]
    pub fn from_str(text: &'b str, encoder: encoding::SharedEncoding) -> Bytes<'b> {
        Self(encoder.encode(text))
    }

    #[inline]
    pub fn to_string(&self, decoder: encoding::SharedEncoding) -> String {
        decoder.decode(self).to_owned()
    }

    #[inline]
    #[must_use] 
    pub fn to_utf8_string(&self) -> String {
        str::from_utf8(self.0.as_ref())
            .expect("should be utf8 string")
            .to_string()
    }

    #[inline]
    #[must_use] 
    pub fn to_upper(self) -> Bytes<'b> {
        Bytes(Cow::from(self.0.to_ascii_uppercase()))
    }

    #[inline]
    #[must_use] 
    pub fn to_lower(self) -> Bytes<'b> {
        Bytes(Cow::from(self.0.to_ascii_lowercase()))
    }

    #[inline]
    pub fn as_upper(&self, encoder: encoding::SharedEncoding) -> String {
        encoder.decode(self).to_ascii_uppercase()
    }

    #[inline]
    pub fn as_lower(&self, encoder: encoding::SharedEncoding) -> String {
        encoder.decode(self).to_ascii_lowercase()
    }

    #[inline]
    #[must_use] 
    pub fn into_owned(self) -> Bytes<'static> {
        Bytes(Cow::Owned(self.0.into_owned()))
    }

    #[inline]
    #[must_use] 
    pub fn to_slice(&'b self, range: Range) -> Bytes<'b> {
        let byte_slice = self.0[range.start..range.end].into();
        Bytes(Cow::Borrowed(byte_slice))
    }

    #[inline]
    #[must_use] 
    pub fn opt_slice(&self, range: Option<Range>) -> Option<Bytes<'_>> {
        range.map(|range| self.to_slice(range))
    }

    #[inline]
    pub fn into_iter(&'b self) -> slice::Iter<'b, u8> {
        self.0.iter()
    }
}

impl Deref for Bytes<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl<'b> From<Cow<'b, [u8]>> for Bytes<'b> {
    #[inline]
    fn from(bytes: Cow<'b, [u8]>) -> Self {
        Bytes(bytes)
    }
}

impl<'b> From<&'b [u8]> for Bytes<'b> {
    #[inline]
    fn from(bytes: &'b [u8]) -> Self {
        Bytes(bytes.into())
    }
}

impl Debug for Bytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}`", self.to_utf8_string())
    }
}
