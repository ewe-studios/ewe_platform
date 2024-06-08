use ewe_mem::{
    encoding::{self, Encoding, SharedEncoding},
    memory::{self, Resetable},
    primitives::Bytes,
};

use anyhow::anyhow;
use thiserror::Error;

use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{cell, rc};

pub type ElementResult<T> = std::result::Result<T, ElementError>;

#[derive(Error, Debug)]
pub enum ElementError {
    #[error("failed to create element")]
    CreationFailure,

    #[error("failed to update element")]
    UpdateFailure,

    #[error("you can't replace the root")]
    FailedReplaceRoot,

    #[error("failed to detach child from parent")]
    FailedChildDetach,

    #[error("Node has no children")]
    NoChildren,

    #[error("ParentLink has no index")]
    NoIndexInLink,

    #[error("Node has no ParentLink")]
    NoParentLink,

    #[error("Appending a RootNode into a NonRoot is not allowed")]
    RootCantBeChild,

    #[error("Found a container with no children")]
    ContainerHasNoContent,

    #[error("Can't have children")]
    CantHaveChildren,

    #[error("Failed to add child")]
    FailedAddChild,

    #[error("provided index is out of bounds: {0}")]
    OutofBoundIndex(usize),

    #[error("not usable for operation")]
    NotUsable,
}

type AttributePool<'a> = memory::SharedArenaPool<Attribute<'a>>;

#[derive(Clone)]
pub struct Attribute<'a> {
    name: Option<Bytes<'a>>,
    value: Option<Bytes<'a>>,

    encoding: encoding::SharedEncoding,
}

impl<'a> memory::Resetable for Attribute<'a> {
    fn reset(&mut self) {
        self.name.take();
        self.value.take();
    }
}

impl<'a> Attribute<'a> {
    #[inline]
    pub fn empty(encoding: encoding::SharedEncoding) -> Self {
        Self {
            name: None,
            value: None,
            encoding,
        }
    }

    #[inline]
    pub fn new(name: Bytes<'a>, value: Bytes<'a>, encoding: encoding::SharedEncoding) -> Self {
        Self {
            name: Some(name),
            value: Some(value),
            encoding,
        }
    }

    #[inline]
    pub fn try_from(
        name: &'a str,
        value: &'a str,
        encoding: encoding::SharedEncoding,
    ) -> ElementResult<Self> {
        Ok(Self {
            name: Some(Bytes::from_str(name, rc::Rc::clone(&encoding))),
            value: Some(Bytes::from_str(value, rc::Rc::clone(&encoding))),
            encoding,
        })
    }

    #[inline]
    pub fn name_lower(&self) -> Option<String> {
        if let Some(name) = self.name.clone() {
            return Some(name.as_lower(rc::Rc::clone(&self.encoding)));
        }
        None
    }

    #[inline]
    pub fn value_bytes(&self) -> Option<Bytes<'a>> {
        self.value.clone()
    }

    #[inline]
    pub fn name_bytes(&self) -> Option<Bytes<'a>> {
        self.name.clone()
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        if let Some(name) = self.name.clone() {
            return Some(name.to_string(rc::Rc::clone(&self.encoding)));
        }
        None
    }

    #[inline]
    pub fn value(&self) -> Option<String> {
        if let Some(value) = self.value.clone() {
            return Some(value.to_string(rc::Rc::clone(&self.encoding)));
        }
        None
    }

    #[inline]
    pub fn set_value_from_slice(&mut self, value: &'a [u8]) {
        self.value = Some(Bytes::from_slice(value));
    }

    #[inline]
    pub fn set_name_from_slice(&mut self, name: &'a [u8]) {
        self.name = Some(Bytes::from_slice(name));
    }

    #[inline]
    pub fn set_name_bytes(&mut self, name: Bytes<'a>) {
        self.name.replace(name);
    }

    #[inline]
    pub fn set_name(&mut self, name: &'a str) {
        self.name = Some(Bytes::from_str(name, rc::Rc::clone(&self.encoding)));
    }

    #[inline]
    pub fn set_value_bytes(&mut self, value: Bytes<'a>) {
        self.value.replace(value);
    }

    #[inline]
    pub fn set_value(&mut self, value: &'a str) {
        self.value = Some(Bytes::from_str(value, rc::Rc::clone(&self.encoding)));
    }
}

type NodePool<'a> = memory::SharedArenaPool<Node<'a>>;

pub struct NodeGenerator<'a> {
    encoding: encoding::SharedEncoding,
    attributes: AttributePool<'a>,
}

impl<'a> NodeGenerator<'a> {
    pub fn new(encoding: encoding::SharedEncoding, attributes: AttributePool<'a>) -> Self {
        Self {
            encoding,
            attributes,
        }
    }
}

impl<'a> memory::PoolGenerator<Node<'a>> for NodeGenerator<'a> {
    fn generate(&self, pool: &mut memory::ArenaPool<Node<'a>>) -> Node<'a> {
        Node::empty(
            rc::Rc::clone(&self.encoding),
            rc::Rc::clone(&self.attributes),
            rc::Rc::new(cell::RefCell::new(pool.clone())),
        )
    }
}

#[derive(Clone)]
pub enum Markup<'a> {
    // HTML represents the standard html tag that can have children (containng other markup).
    HTML(Option<Node<'a>>),

    // Self closing HTML tags that do not have children
    NoChildHTML(Option<Node<'a>>),

    Island {
        children: Option<Vec<Option<Markup<'a>>>>,
    },

    LocalIsland {
        alias: &'a str,
        attributes: Option<Vec<Option<Attribute<'a>>>>,
    },

    RemoteIsland {
        endpoint: &'a str,
        attributes: Option<Vec<Option<Attribute<'a>>>>,
    },

    Component {
        name: &'a str,
        node: Option<Node<'a>>,
    },
}

fn deallocate_nodes<'a>(
    mut list: Option<Vec<Option<Node<'a>>>>,
    node_pool: NodePool<'a>,
) -> ElementResult<()> {
    match list.take() {
        None => Ok(()),
        Some(mut nodes) => {
            for container in nodes.iter_mut() {
                match container.take() {
                    None => continue,
                    Some(node) => {
                        node_pool.borrow_mut().deallocate(node);
                    }
                }
            }
            Ok(())
        }
    }
}

fn deallocate_attributes<'a>(
    mut attributes: Option<Vec<Option<Attribute<'a>>>>,
    attribute_pool: AttributePool<'a>,
) -> ElementResult<()> {
    match attributes.take() {
        None => Ok(()),
        Some(mut attrs) => {
            for container in attrs.iter_mut() {
                match container.take() {
                    None => continue,
                    Some(attr) => {
                        attribute_pool.borrow_mut().deallocate(attr);
                    }
                }
            }
            Ok(())
        }
    }
}

fn deallocate_markup_list<'a>(
    mut list: Option<Vec<Option<Markup<'a>>>>,
    node_pool: NodePool<'a>,
    attribute_pool: AttributePool<'a>,
) -> ElementResult<()> {
    match list.take() {
        None => Ok(()),
        Some(mut nodes) => {
            for container in nodes.iter_mut() {
                match deallocate_markup(container.take(), node_pool.clone(), attribute_pool.clone())
                {
                    Err(err) => return Err(err),
                    Ok(_) => continue,
                }
            }
            Ok(())
        }
    }
}

fn deallocate_markup<'a>(
    mut markup: Option<Markup<'a>>,
    node_pool: NodePool<'a>,
    attribute_pool: AttributePool<'a>,
) -> ElementResult<()> {
    match markup.take() {
        None => Ok(()),
        Some(mut node) => match node {
            Markup::NoChildHTML(mut container) => match container.take() {
                None => Ok(()),
                Some(node) => {
                    node_pool.borrow_mut().deallocate(node);
                    Ok(())
                }
            },
            Markup::HTML(mut container) => match container.take() {
                None => Ok(()),
                Some(node) => {
                    node_pool.borrow_mut().deallocate(node);
                    Ok(())
                }
            },
            Markup::RemoteIsland {
                endpoint,
                mut attributes,
            } => deallocate_attributes(attributes.take(), attribute_pool.clone()),
            Markup::LocalIsland {
                alias,
                mut attributes,
            } => deallocate_attributes(attributes.take(), attribute_pool.clone()),
            Markup::Component { name, mut node } => match node.take() {
                None => Ok(()),
                Some(item) => {
                    node_pool.borrow_mut().deallocate(item);
                    Ok(())
                }
            },
            Markup::Island { mut children } => {
                deallocate_markup_list(children.take(), node_pool.clone(), attribute_pool.clone())
            }
        },
    }
}

impl<'a> Markup<'a> {
    pub fn after_node_size(&self) -> ElementResult<usize> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(n) => Ok(n.after_node_size()),
            },
        }
    }

    pub fn before_node_size(&self) -> ElementResult<usize> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(n) => Ok(n.before_node_size()),
            },
        }
    }

    pub fn children_size(&self) -> ElementResult<usize> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(n) => Ok(n.children_size()),
            },
        }
    }

    pub fn get_node_mut(&mut self, index: usize) -> ElementResult<&Option<Node<'a>>> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => Ok(container),
        }
    }

    pub fn get_node(&self, index: usize) -> ElementResult<&Option<Node<'a>>> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => Ok(container),
        }
    }

    pub fn get_before(&mut self, index: usize) -> ElementResult<Option<&Option<Markup<'a>>>> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_before(index)),
            },
        }
    }

    pub fn get_before_mut(
        &'a mut self,
        index: usize,
    ) -> ElementResult<Option<&'a mut Option<Markup<'a>>>> {
        match self {
            _ => Err(ElementError::NotUsable),
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_before_mut(index)),
            },
        }
    }

    pub fn name(&mut self, name: &'a str) -> ElementResult<()> {
        match self {
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.name(name)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn add_before(&mut self, elem: Markup<'a>) -> ElementResult<usize> {
        match self {
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.add_before(elem)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_after(&mut self, index: usize) -> ElementResult<Option<&Option<Markup<'a>>>> {
        match self {
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_after(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_after_mut(
        &'a mut self,
        index: usize,
    ) -> ElementResult<Option<&'a mut Option<Markup<'a>>>> {
        match self {
            Markup::NoChildHTML(container) | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_after_mut(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn add_after(&mut self, elem: Markup<'a>) -> ElementResult<usize> {
        match self {
            Markup::NoChildHTML(container)
            | Markup::NoChildHTML(container)
            | Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.add_after(elem)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_child(&mut self, index: usize) -> ElementResult<Option<&Option<Markup<'a>>>> {
        match self {
            Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_child(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_child_mut(
        &'a mut self,
        index: usize,
    ) -> ElementResult<Option<&'a mut Option<Markup<'a>>>> {
        match self {
            Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_child_mut(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn add_child(&mut self, elem: Markup<'a>) -> ElementResult<usize> {
        match self {
            Markup::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.add_child(elem)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }
}

#[derive(Clone)]
pub struct Node<'a> {
    name: Option<Bytes<'a>>,

    // the attributes related to the node.
    attributes: Vec<Option<Attribute<'a>>>,

    // after nodes are special in that they are in the order of
    // precedence, where the first item is the most farthest element
    // after this `Node` and the last the closest after this element.
    after: Vec<Option<Markup<'a>>>,

    // before nodes are special in that they are in the order of
    // precedence, where the first item is the most farthest element
    // before this `Node` and the last the closest before this element.
    before: Vec<Option<Markup<'a>>>,

    // Content works different, content that is appended as normal
    // follow the order in the `Vec` with the first being the element
    // and the last the last element.
    //
    // Performing a `Node::before_content` must instead of adjusting the
    // `Vec` simply get the first element and uses it's before list.
    content: Vec<Option<Markup<'a>>>,

    // internal private fields that should never change.
    encoding: encoding::SharedEncoding,
    attribute_pool: AttributePool<'a>,
    node_pool: NodePool<'a>,
}

impl<'a> Node<'a> {
    pub fn empty(
        encoding: encoding::SharedEncoding,
        attribute_pool: AttributePool<'a>,
        node_pool: NodePool<'a>,
    ) -> Self {
        Self {
            attribute_pool,
            attributes: vec![],
            content: vec![],
            before: vec![],
            after: vec![],
            name: None,
            node_pool,
            encoding,
        }
    }

    pub fn no_child(name: &'a str, node_pool: NodePool<'a>) -> ElementResult<Markup<'a>> {
        let mut node = node_pool
            .borrow_mut()
            .allocate()
            .expect("generate new node");
        node.name(name);
        Ok(Markup::NoChildHTML(Some(node)))
    }

    pub fn with_children(name: &'a str, node_pool: NodePool<'a>) -> ElementResult<Markup<'a>> {
        let mut node = node_pool
            .borrow_mut()
            .allocate()
            .expect("generate new node");
        node.name(name);
        Ok(Markup::HTML(Some(node)))
    }

    pub fn after_node_size(&self) -> usize {
        self.after.len()
    }

    pub fn before_node_size(&self) -> usize {
        self.before.len()
    }

    pub fn children_size(&self) -> usize {
        self.content.len()
    }

    pub(crate) fn next_child_index(&self) -> usize {
        self.content.len()
    }

    pub fn name(&mut self, name: &'a str) {
        self.name = Some(Bytes::from_str(name, rc::Rc::clone(&self.encoding)));
    }

    pub fn add_attribute(&mut self, name: &'a str, value: &'a str) {
        self.update_attribute(
            Bytes::from_str(name, self.encoding.clone()),
            Bytes::from_str(value, self.encoding.clone()),
        )
    }

    pub fn has_attr(&mut self, name: &'a str) -> bool {
        let encoded_str = Bytes::from_str(name, self.encoding.clone());
        return self
            .attributes
            .iter_mut()
            .find(|attr_container| {
                println!("Checking attribute in list");
                if let Some(attr) = attr_container {
                    println!(
                        "Matching: {:?} against {:?}",
                        attr.name_bytes().unwrap(),
                        encoded_str
                    );
                    return attr.name_bytes().unwrap() == encoded_str;
                }
                false
            })
            .is_some();
    }

    pub fn attr_value(&mut self, name: &'a str) -> Option<Bytes<'a>> {
        let encoded_str = Bytes::from_str(name, self.encoding.clone());
        return match self.attributes.iter_mut().find(|attr_container| {
            if let Some(attr) = attr_container {
                return attr.name_bytes().unwrap() == encoded_str;
            }
            false
        }) {
            Some(attr_container) => {
                if let Some(mut attr) = attr_container.clone() {
                    return attr.value.clone();
                }
                None
            }
            None => None,
        };
    }

    pub fn update_attribute(&mut self, name: Bytes<'a>, value: Bytes<'a>) {
        let find_attr = self.attributes.iter_mut().find(|attr_container| {
            if let Some(attr) = attr_container {
                return attr.name_bytes().unwrap() == name;
            }
            false
        });

        match find_attr {
            Some(attr_container) => {
                let mut attr = attr_container.clone().unwrap();
                attr.set_value_bytes(value);
            }
            None => {
                self.attributes.push(Some(Attribute::new(
                    name,
                    value,
                    rc::Rc::clone(&self.encoding),
                )));
            }
        };
    }

    pub fn remove_child_at(&mut self, index: usize) -> ElementResult<()> {
        let child_size = self.content.len();
        if index >= child_size {
            return Err(ElementError::OutofBoundIndex(index));
        }

        deallocate_markup(
            self.content.remove(index),
            self.node_pool.clone(),
            self.attribute_pool.clone(),
        )
    }

    pub fn get_before(&mut self, index: usize) -> Option<&Option<Markup<'a>>> {
        self.before.get(index)
    }

    pub fn get_before_mut(&'a mut self, index: usize) -> Option<&'a mut Option<Markup<'a>>> {
        self.before.get_mut(index)
    }

    pub fn add_before(&mut self, elem: Markup<'a>) -> usize {
        let index = self.before.len();
        self.before.push(Some(elem));
        index
    }

    pub fn get_after(&mut self, index: usize) -> Option<&Option<Markup<'a>>> {
        self.after.get(index)
    }

    pub fn get_after_mut(&'a mut self, index: usize) -> Option<&'a mut Option<Markup<'a>>> {
        self.after.get_mut(index)
    }

    pub fn add_after(&mut self, elem: Markup<'a>) -> usize {
        let index = self.after.len();
        self.after.push(Some(elem));
        index
    }

    pub fn get_child(&mut self, index: usize) -> Option<&Option<Markup<'a>>> {
        self.content.get(index)
    }

    pub fn get_child_mut(&'a mut self, index: usize) -> Option<&'a mut Option<Markup<'a>>> {
        self.content.get_mut(index)
    }

    pub fn add_child(&mut self, elem: Markup<'a>) -> usize {
        let next_index = self.content.len();
        self.content.push(Some(elem));
        next_index
    }
}

impl<'a> memory::Resetable for Node<'a> {
    fn reset(&mut self) {
        self.name.take();

        for attribute in self.attributes.iter_mut() {
            if let Some(attr) = attribute.take() {
                self.attribute_pool.borrow_mut().deallocate(attr);
            }
        }

        self.attributes.clear();

        for elem in self.content.iter_mut() {
            deallocate_markup(
                elem.take(),
                self.node_pool.clone(),
                self.attribute_pool.clone(),
            )
            .expect("dellocate node");
        }

        self.content.clear();

        for elem in self.before.iter_mut() {
            deallocate_markup(
                elem.take(),
                self.node_pool.clone(),
                self.attribute_pool.clone(),
            )
            .expect("dellocate node");
        }

        self.before.clear();

        for elem in self.after.iter_mut() {
            deallocate_markup(
                elem.take(),
                self.node_pool.clone(),
                self.attribute_pool.clone(),
            )
            .expect("dellocate node");
        }

        self.after.clear();
    }
}

#[cfg(test)]
mod markup_tests {

    use lazy_static::lazy_static;
    use std::sync::{Arc, Mutex};

    use ewe_mem::{encoding, memory::Resetable};

    use super::*;

    #[test]
    fn node_can_reset() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: NodePool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = Markup::HTML(Some(node_allocator.borrow_mut().allocate().unwrap()));
        child.name("section").expect("should set name");

        assert_eq!(node.add_child(child), 0);

        assert_eq!(node.children_size(), 1);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 0);

        node.reset();

        assert_eq!(node.children_size(), 0);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 0);
    }

    #[test]
    fn can_detach_child_from_parent() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: NodePool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_child(Markup::NoChildHTML(Some(child))), 0);

        assert_eq!(node.children_size(), 1);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 0);

        assert!(matches!(node.remove_child_at(0), ElementResult::Ok(_)));

        assert_eq!(node.children_size(), 0);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 0);

        node_allocator.borrow_mut().deallocate(node);
    }

    #[test]
    fn can_add_node_into_after_list() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: NodePool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_after(Markup::NoChildHTML(Some(child))), 0);

        assert_eq!(node.children_size(), 0);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 1);

        node_allocator.borrow_mut().deallocate(node);
    }

    #[test]
    fn can_add_node_into_before_list() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: NodePool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_before(Markup::NoChildHTML(Some(child))), 0);

        assert_eq!(node.children_size(), 0);
        assert_eq!(node.before_node_size(), 1);
        assert_eq!(node.after_node_size(), 0);

        node_allocator.borrow_mut().deallocate(node);
    }

    #[test]
    fn can_create_markup_with_children() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: NodePool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_child(Markup::NoChildHTML(Some(child))), 0);

        assert_eq!(node.children_size(), 1);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 0);

        node_allocator.borrow_mut().deallocate(node);
    }

    #[test]
    fn can_create_markup_with_attributes() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: NodePool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        assert_eq!(
            node.name.clone().unwrap(),
            Bytes::from_str("div", shared_encoding.clone())
        );

        node.add_attribute("width", "400px");
        node.add_attribute("height", "400px");

        assert_eq!(node.has_attr("width"), true);
        assert_eq!(node.has_attr("height"), true);

        assert!(matches!(node.attr_value("width"), Some(_)));
        assert!(matches!(node.attr_value("height"), Some(_)));

        assert_eq!(
            node.attr_value("width").unwrap(),
            Bytes::from_str("400px", shared_encoding.clone())
        );

        assert_eq!(
            node.attr_value("height").unwrap(),
            Bytes::from_str("400px", shared_encoding.clone())
        );

        node_allocator.borrow_mut().deallocate(node);
    }
}
