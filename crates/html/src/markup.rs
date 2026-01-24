use foundation_core::io::mem::{
    encoding::{self},
    memory::{self},
    primitives::Bytes,
};

use thiserror::Error;

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

    #[error("unable to render not due to : {0}")]
    CantRender(String),
}

type AttributePool<'a> = memory::SharedArenaPool<Attribute<'a>>;

#[derive(Clone)]
pub struct Attribute<'a> {
    name: Option<Bytes<'a>>,
    value: Option<Bytes<'a>>,

    encoding: encoding::SharedEncoding,
}

impl memory::Resettable for Attribute<'_> {
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
    #[must_use] 
    pub fn name_lower(&self) -> Option<String> {
        if let Some(name) = self.name.clone() {
            return Some(name.as_lower(rc::Rc::clone(&self.encoding)));
        }
        None
    }

    #[inline]
    #[must_use] 
    pub fn value_bytes(&self) -> Option<Bytes<'a>> {
        self.value.clone()
    }

    #[inline]
    #[must_use] 
    pub fn name_bytes(&self) -> Option<Bytes<'a>> {
        self.name.clone()
    }

    #[inline]
    #[must_use] 
    pub fn name(&self) -> Option<String> {
        if let Some(name) = self.name.clone() {
            return Some(name.to_string(rc::Rc::clone(&self.encoding)));
        }
        None
    }

    #[inline]
    #[must_use] 
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

type FragmentPool<'a> = memory::SharedArenaPool<Fragment<'a>>;

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

impl<'a> memory::PoolGenerator<Fragment<'a>> for NodeGenerator<'a> {
    fn generate(&self, pool: &mut memory::ArenaPool<Fragment<'a>>) -> Fragment<'a> {
        Fragment::empty(
            rc::Rc::clone(&self.encoding),
            rc::Rc::clone(&self.attributes),
            rc::Rc::new(cell::RefCell::new(pool.clone())),
        )
    }
}

#[derive(Clone)]
pub enum FragmentDef<'a> {
    // HTML represents the standard html tag that can have children (containng other markup).
    HTML(Option<Fragment<'a>>),

    // Self closing HTML tags that do not have children
    NoChildHTML(Option<Fragment<'a>>),

    // Components are the definition of interactive, operation or content
    // generating fragments
    Component(&'a dyn Island<'a>),
}

#[allow(unused)]
#[cfg_attr(
    feature = "debug_trace",
    tracing::instrument(level = "trace", skip_all)
)]
fn deallocate_nodes<'a>(
    mut list: Option<Vec<Option<Fragment<'a>>>>,
    node_pool: FragmentPool<'a>,
) -> ElementResult<()> {
    match list.take() {
        None => Ok(()),
        Some(mut nodes) => {
            for container in &mut nodes {
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

#[allow(unused)]
#[cfg_attr(
    feature = "debug_trace",
    tracing::instrument(level = "trace", skip_all)
)]
fn deallocate_attributes<'a>(
    mut attributes: Option<Vec<Option<Attribute<'a>>>>,
    attribute_pool: AttributePool<'a>,
) -> ElementResult<()> {
    match attributes.take() {
        None => Ok(()),
        Some(mut attrs) => {
            for container in &mut attrs {
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

#[allow(unused)]
#[cfg_attr(
    feature = "debug_trace",
    tracing::instrument(level = "trace", skip_all)
)]
fn deallocate_markup_list<'a>(
    mut list: Option<Vec<Option<FragmentDef<'a>>>>,
    node_pool: FragmentPool<'a>,
    attribute_pool: AttributePool<'a>,
) -> ElementResult<()> {
    match list.take() {
        None => Ok(()),
        Some(mut nodes) => {
            for container in &mut nodes {
                match deallocate_markup(container.take(), node_pool.clone(), attribute_pool.clone())
                {
                    Err(err) => return Err(err),
                    Ok(()) => continue,
                }
            }
            Ok(())
        }
    }
}

#[cfg_attr(
    feature = "debug_trace",
    tracing::instrument(level = "trace", skip_all)
)]
fn deallocate_markup<'a>(
    mut markup: Option<FragmentDef<'a>>,
    node_pool: FragmentPool<'a>,
    _attribute_pool: AttributePool<'a>,
) -> ElementResult<()> {
    match markup.take() {
        None => Ok(()),
        Some(node) => match node {
            FragmentDef::NoChildHTML(mut container) => match container.take() {
                None => Ok(()),
                Some(node) => {
                    node_pool.borrow_mut().deallocate(node);
                    Ok(())
                }
            },
            FragmentDef::HTML(mut container) => match container.take() {
                None => Ok(()),
                Some(node) => {
                    node_pool.borrow_mut().deallocate(node);
                    Ok(())
                }
            },
            FragmentDef::Component(_t) => Ok(()),
        },
    }
}

impl<'a> FragmentDef<'a> {
    pub fn after_node_size(&self) -> ElementResult<usize> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(n) => Ok(n.after_node_size()),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn before_node_size(&self) -> ElementResult<usize> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(n) => Ok(n.before_node_size()),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn children_size(&self) -> ElementResult<usize> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(n) => Ok(n.children_size()),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_node_mut(&mut self, _index: usize) -> ElementResult<&Option<Fragment<'a>>> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => Ok(container),
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_node(&self, _index: usize) -> ElementResult<&Option<Fragment<'a>>> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => Ok(container),
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_before(&mut self, index: usize) -> ElementResult<Option<&Option<FragmentDef<'a>>>> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_before(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_before_mut(
        &'a mut self,
        index: usize,
    ) -> ElementResult<Option<&'a mut Option<FragmentDef<'a>>>> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_before_mut(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn name(&mut self, name: &'a str) -> ElementResult<()> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => {
                    node.name(name);
                    Ok(())
                }
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn add_before(&mut self, elem: FragmentDef<'a>) -> ElementResult<usize> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.add_before(elem)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_after(&mut self, index: usize) -> ElementResult<Option<&Option<FragmentDef<'a>>>> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_after(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_after_mut(
        &'a mut self,
        index: usize,
    ) -> ElementResult<Option<&'a mut Option<FragmentDef<'a>>>> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.get_after_mut(index)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn add_after(&mut self, elem: FragmentDef<'a>) -> ElementResult<usize> {
        match self {
            FragmentDef::NoChildHTML(container) | FragmentDef::HTML(container) => match container {
                None => Err(ElementError::NotUsable),
                Some(node) => Ok(node.add_after(elem)),
            },
            _ => Err(ElementError::NotUsable),
        }
    }

    pub fn get_child(&mut self, index: usize) -> ElementResult<Option<&Option<FragmentDef<'a>>>> {
        if let FragmentDef::HTML(Some(node)) = self {
            return Ok(node.get_child(index));
        }
        Err(ElementError::NotUsable)
    }

    pub fn get_child_mut(
        &'a mut self,
        index: usize,
    ) -> ElementResult<Option<&'a mut Option<FragmentDef<'a>>>> {
        if let FragmentDef::HTML(Some(node)) = self {
            return Ok(node.get_child_mut(index));
        }
        Err(ElementError::NotUsable)
    }

    pub fn add_child(&mut self, elem: FragmentDef<'a>) -> ElementResult<usize> {
        if let FragmentDef::HTML(Some(node)) = self {
            return Ok(node.add_child(elem));
        }
        Err(ElementError::NotUsable)
    }
}

#[derive(Clone)]
pub struct Fragment<'a> {
    name: Option<Bytes<'a>>,

    // the attributes related to the node.
    attributes: Vec<Option<Attribute<'a>>>,

    // after nodes are special in that they are in the order of
    // precedence, where the first item is the most farthest element
    // after this `Node` and the last the closest after this element.
    after: Vec<Option<FragmentDef<'a>>>,

    // before nodes are special in that they are in the order of
    // precedence, where the first item is the most farthest element
    // before this `Node` and the last the closest before this element.
    before: Vec<Option<FragmentDef<'a>>>,

    // Content works different, content that is appended as normal
    // follow the order in the `Vec` with the first being the element
    // and the last the last element.
    //
    // Performing a `Node::before_content` must instead of adjusting the
    // `Vec` simply get the first element and uses it's before list.
    content: Vec<Option<FragmentDef<'a>>>,

    // internal private fields that should never change.
    encoding: encoding::SharedEncoding,

    attribute_pool: AttributePool<'a>,

    node_pool: FragmentPool<'a>,
}

impl<'a> Fragment<'a> {
    pub fn empty(
        encoding: encoding::SharedEncoding,
        attribute_pool: AttributePool<'a>,
        node_pool: FragmentPool<'a>,
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

    pub fn render_into(&mut self) -> ElementResult<Self> {
        let fragment_attributes = self.attributes.clone();
        match self.node_pool.borrow_mut().allocate() {
            Ok(mut fragment) => {
                fragment.attributes = fragment_attributes;

                for before_element in &self.before {
                    match before_element {
                        Some(_item) => {}
                        None => continue,
                    }
                }

                Ok(fragment)
            }
            Err(_err) => Err(ElementError::CantRender(String::from(
                "memory limit exceeded",
            ))),
        }
    }

    pub fn no_child(name: &'a str, node_pool: FragmentPool<'a>) -> ElementResult<FragmentDef<'a>> {
        let mut node = node_pool
            .borrow_mut()
            .allocate()
            .expect("generate new node");
        node.name(name);
        Ok(FragmentDef::NoChildHTML(Some(node)))
    }

    pub fn with_children(
        name: &'a str,
        node_pool: FragmentPool<'a>,
    ) -> ElementResult<FragmentDef<'a>> {
        let mut node = node_pool
            .borrow_mut()
            .allocate()
            .expect("generate new node");
        node.name(name);
        Ok(FragmentDef::HTML(Some(node)))
    }

    #[must_use] 
    pub fn after_node_size(&self) -> usize {
        self.after.len()
    }

    #[must_use] 
    pub fn before_node_size(&self) -> usize {
        self.before.len()
    }

    #[must_use] 
    pub fn children_size(&self) -> usize {
        self.content.len()
    }

    #[allow(unused)]
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
        );
    }

    pub fn has_attr(&mut self, name: &'a str) -> bool {
        let encoded_str = Bytes::from_str(name, self.encoding.clone());
        self.attributes.iter_mut().any(|attr_container| {
            ewe_trace::debug!("Checking attribute in list");
            if let Some(attr) = attr_container {
                ewe_trace::debug!(
                    "Matching: {:?} against {:?}",
                    attr.name_bytes().unwrap(),
                    encoded_str
                );
                return attr.name_bytes().unwrap() == encoded_str;
            }
            false
        })
    }

    #[cfg_attr(
        feature = "debug_trace",
        tracing::instrument(level = "trace", skip_all)
    )]
    pub fn attr_value(&mut self, name: &'a str) -> Option<Bytes<'a>> {
        let encoded_str = Bytes::from_str(name, self.encoding.clone());
        match self.attributes.iter_mut().find(|attr_container| {
            if let Some(attr) = attr_container {
                return attr.name_bytes().unwrap() == encoded_str;
            }
            false
        }) {
            Some(attr_container) => {
                if let Some(attr) = attr_container.clone() {
                    return attr.value.clone();
                }
                None
            }
            None => None,
        }
    }

    #[cfg_attr(
        feature = "debug_trace",
        tracing::instrument(level = "trace", skip_all)
    )]
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
        }
    }

    #[cfg_attr(
        feature = "debug_trace",
        tracing::instrument(level = "trace", skip_all)
    )]
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

    pub fn get_before(&mut self, index: usize) -> Option<&Option<FragmentDef<'a>>> {
        self.before.get(index)
    }

    pub fn get_before_mut(&'a mut self, index: usize) -> Option<&'a mut Option<FragmentDef<'a>>> {
        self.before.get_mut(index)
    }

    pub fn add_before(&mut self, elem: FragmentDef<'a>) -> usize {
        let index = self.before.len();
        self.before.push(Some(elem));
        index
    }

    pub fn get_after(&mut self, index: usize) -> Option<&Option<FragmentDef<'a>>> {
        self.after.get(index)
    }

    pub fn get_after_mut(&'a mut self, index: usize) -> Option<&'a mut Option<FragmentDef<'a>>> {
        self.after.get_mut(index)
    }

    pub fn add_after(&mut self, elem: FragmentDef<'a>) -> usize {
        let index = self.after.len();
        self.after.push(Some(elem));
        index
    }

    pub fn get_child(&mut self, index: usize) -> Option<&Option<FragmentDef<'a>>> {
        self.content.get(index)
    }

    pub fn get_child_mut(&'a mut self, index: usize) -> Option<&'a mut Option<FragmentDef<'a>>> {
        self.content.get_mut(index)
    }

    pub fn add_child(&mut self, elem: FragmentDef<'a>) -> usize {
        let next_index = self.content.len();
        self.content.push(Some(elem));
        next_index
    }
}

impl memory::Resettable for Fragment<'_> {
    #[cfg_attr(
        feature = "debug_trace",
        tracing::instrument(level = "trace", skip_all)
    )]
    fn reset(&mut self) {
        self.name.take();

        for attribute in &mut self.attributes {
            if let Some(attr) = attribute.take() {
                self.attribute_pool.borrow_mut().deallocate(attr);
            }
        }

        self.attributes.clear();

        for elem in &mut self.content {
            deallocate_markup(
                elem.take(),
                self.node_pool.clone(),
                self.attribute_pool.clone(),
            )
            .expect("dellocate node");
        }

        self.content.clear();

        for elem in &mut self.before {
            deallocate_markup(
                elem.take(),
                self.node_pool.clone(),
                self.attribute_pool.clone(),
            )
            .expect("dellocate node");
        }

        self.before.clear();

        for elem in &mut self.after {
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

#[derive(Error, Debug)]
pub enum IslandError {
    #[error("Could not locate location of target insertion point: {0}")]
    CantFindSlotPoint(String),

    #[error("Fragment failed to complete successfully due to: {0}")]
    FragmentFailed(String),
}

/// Islands are the fundamental means to define unique components and the underlying content
/// they generate. It takes a root into which it pushes the core result into.
pub trait Island<'a> {
    /// `render_into` renders the given fragment as a content of the root.   fn `render_into(&self`, root: Fragment<'a>) -> Result<(), `IslandError`>;
    fn render_into(&self, root: Fragment<'a>) -> Result<(), IslandError>;

    /// `render_before` renders given fragment as a before content of the root.   fn `render_into(&self`, root: Fragment<'a>) -> Result<(), `IslandError`>;
    fn render_before(&self, root: Fragment<'a>) -> Result<(), IslandError>;

    /// `render_after` renders the given fragment as a after content of the root.   fn `render_into(&self`, root: Fragment<'a>) -> Result<(), `IslandError`>;
    fn render_after(&self, root: Fragment<'a>) -> Result<(), IslandError>;
}

impl<'a> Island<'a> for Fragment<'a> {
    fn render_into(&self, _root: Fragment<'a>) -> Result<(), IslandError> {
        todo!()
    }

    fn render_before(&self, _root: Fragment<'a>) -> Result<(), IslandError> {
        todo!()
    }

    fn render_after(&self, _root: Fragment<'a>) -> Result<(), IslandError> {
        todo!()
    }
}

#[derive(Clone)]
pub enum IslandAddr<'a> {
    Local(&'a str),
    Remote(&'a str),
}

/// `FragementRef` provides a type indicating to the Island what type of fragment
/// its dealing with, this is useful as a metadata to how an island might what to
/// deal with the parent.
///
/// An Example: A <swap-out/> operation island
///
/// If its parent is a Page for instance, an operation Island like `swap-out` will
/// probably dig deep into the page and look for it's target location to slot the relevant
/// data into location
///
/// But if its a Partial will simply append the content with its operation wrapper as is to
/// allow the client decide how to handle the swap operation.
pub enum FragmentRef<'a> {
    Page(Fragment<'a>),
    Partial(Fragment<'a>),
}

#[cfg(test)]
mod markup_tests {

    use foundation_core::io::mem::{encoding, memory::Resettable};

    use super::*;

    #[test]
    fn node_can_reset() {
        let shared_encoding = encoding::UTF8Encoding::shared();
        let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

        let attribute_pool: AttributePool = memory::ArenaPool::create_shared(
            rc::Rc::clone(&memory_limit),
            memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
        );

        let node_allocator: FragmentPool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = FragmentDef::HTML(Some(node_allocator.borrow_mut().allocate().unwrap()));
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

        let node_allocator: FragmentPool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_child(FragmentDef::NoChildHTML(Some(child))), 0);

        assert_eq!(node.children_size(), 1);
        assert_eq!(node.before_node_size(), 0);
        assert_eq!(node.after_node_size(), 0);

        assert!(matches!(node.remove_child_at(0), ElementResult::Ok(())));

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

        let node_allocator: FragmentPool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_after(FragmentDef::NoChildHTML(Some(child))), 0);

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

        let node_allocator: FragmentPool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_before(FragmentDef::NoChildHTML(Some(child))), 0);

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

        let node_allocator: FragmentPool = memory::ArenaPool::create_shared(
            memory_limit,
            NodeGenerator::new(shared_encoding.clone(), attribute_pool.clone()),
        );

        let mut node = node_allocator.borrow_mut().allocate().unwrap();
        node.name("div");

        let mut child = node_allocator.borrow_mut().allocate().unwrap();
        child.name("section");

        assert_eq!(node.add_child(FragmentDef::NoChildHTML(Some(child))), 0);

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

        let node_allocator: FragmentPool = memory::ArenaPool::create_shared(
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

        assert!(node.has_attr("width"));
        assert!(node.has_attr("height"));

        assert!(node.attr_value("width").is_some());
        assert!(node.attr_value("height").is_some());

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
