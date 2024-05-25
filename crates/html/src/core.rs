use crate::encoding::Encoding;
use crate::primitives::Bytes;

use std::{cell, rc};

pub type ElementResult<T> = std::result::Result<T, anyhow::Error>;

pub struct Attribute<'a> {
    name: Bytes<'a>,
    value: Bytes<'a>,
    encoding: &'static dyn Encoding,
}

impl<'a> Attribute<'a> {
    #[inline]
    pub fn new(name: Bytes<'a>, value: Bytes<'a>, encoding: &'static dyn Encoding) -> Self {
        Self {
            name,
            value,
            encoding,
        }
    }

    #[inline]
    pub fn try_from(
        name: &'a str,
        value: &'a str,
        encoding: &'static dyn Encoding,
    ) -> ElementResult<Self> {
        Ok(Self {
            name: Bytes::from_str(name, encoding),
            value: Bytes::from_str(value, encoding),
            encoding,
        })
    }

    #[inline]
    pub fn name_lower(&self) -> String {
        self.name.as_lower(self.encoding)
    }

    #[inline]
    pub fn name(&self) -> String {
        self.name.to_string(self.encoding)
    }

    #[inline]
    pub fn value(&self) -> String {
        self.value.to_string(self.encoding)
    }

    #[inline]
    pub fn set_value_from_slice(&mut self, value: &'a [u8]) {
        self.value = Bytes::from_slice(value);
    }

    #[inline]
    pub fn set_name_from_slice(&mut self, name: &'a [u8]) {
        self.name = Bytes::from_slice(name);
    }

    #[inline]
    pub fn set_name(&mut self, name: &'a str) {
        self.name = Bytes::from_str(name, self.encoding);
    }

    #[inline]
    pub fn set_value(&mut self, value: &'a str) {
        self.value = Bytes::from_str(value, self.encoding);
    }
}

pub type ElementHandle<'b> = rc::Rc<lazycell::LazyCell<Node<'b>>>;

pub type AttributeHandle<'b> = rc::Rc<lazycell::LazyCell<Attribute<'b>>>;

pub struct Node<'a> {
    name: Bytes<'a>,
    removed: bool,
    parent: Option<ElementHandle<'a>>,
    attributes: Option<Vec<AttributeHandle<'a>>>,

    // after nodes are special in that they are in the order of
    // precedence, where the first item is the most farthest element
    // after this one and the last the closest after this element.
    after: Option<Vec<ElementHandle<'a>>>,

    // before nodes are special in that they are in the order of
    // precedence, where the first item is the most farthest element
    // before this one and the last the closest before this element.
    before: Option<Vec<ElementHandle<'a>>>,

    // Content works different, content that is appended as normal
    // follow the order in the `Vec` with the first being the element
    // and the last the last element.
    //
    // Performing a `Node::before_content` must instead of adjusting the
    // `Vec` simply get the first element and uses it's before list.
    content: Option<Vec<ElementHandle<'a>>>,

    encoding: &'static dyn Encoding,
}

impl<'a> Node<'a> {
    pub fn new(name: &'a str, encoding: &'static dyn Encoding) -> Self {
        Self {
            encoding,
            parent: None,
            attributes: None,
            after: None,
            before: None,
            content: None,
            removed: false,
            name: Bytes::from_str(name, encoding),
        }
    }

    // Must implement the following html markup methods
    // before() - adds element before this element
    // after() - adds element after this element
    // replace() - replaces the Element at that specific point
    // remove() - mark the element as removed - with it's children
}

/// Markup defines a limited variation of XML/Html nodes with
/// a limited set of css selector support to allow filtering and
/// selective retrieval of internal nodes within
pub enum Markup<'a> {
    /// SelfClosing are Element who close themselves
    SelfClosing(Node<'a>),
    Standard(Node<'a>),
}

impl<'a> Markup<'a> {}
