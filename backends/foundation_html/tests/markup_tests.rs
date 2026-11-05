//! Markup node tests moved out of `src/markup.rs` (spec 70 feature 59,
//! rule 7).
//!
//! The tests use public markup API only. `AttributePool` and `FragmentPool`
//! are private type aliases, so the local bindings let type inference pick
//! the concrete `memory::SharedArenaPool` type. `Fragment` has no public
//! name field, so the moved test reads it through the new `get_name`
//! getter.

use foundation_core::io::mem::{
    encoding,
    memory::{self, Resettable},
    primitives::Bytes,
};
use foundation_html::markup::*;

fn pools() -> (
    memory::SharedArenaPool<Attribute<'static>>,
    memory::SharedArenaPool<Fragment<'static>>,
) {
    let shared_encoding = encoding::UTF8Encoding::shared();
    let memory_limit = memory::MemoryLimiter::create_shared(10 * 10234 * 1024);

    let attribute_pool = memory::ArenaPool::create_shared(
        &memory_limit,
        memory::FnGenerator::new(|_| Attribute::empty(encoding::UTF8Encoding::shared())),
    );

    let node_allocator = memory::ArenaPool::create_shared(
        &memory_limit,
        NodeGenerator::new(shared_encoding, attribute_pool.clone()),
    );

    (attribute_pool, node_allocator)
}

#[test]
fn node_can_reset() {
    let (_attribute_pool, node_allocator) = pools();

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
    let (_attribute_pool, node_allocator) = pools();

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
    let (_attribute_pool, node_allocator) = pools();

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
    let (_attribute_pool, node_allocator) = pools();

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
    let (_attribute_pool, node_allocator) = pools();

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
    let (_attribute_pool, node_allocator) = pools();
    let shared_encoding = encoding::UTF8Encoding::shared();

    let mut node = node_allocator.borrow_mut().allocate().unwrap();
    node.name("div");

    assert_eq!(
        node.get_name().cloned().unwrap(),
        Bytes::from_str("div", &shared_encoding)
    );

    node.add_attribute("width", "400px");
    node.add_attribute("height", "400px");

    assert!(node.has_attr("width"));
    assert!(node.has_attr("height"));

    assert!(node.attr_value("width").is_some());
    assert!(node.attr_value("height").is_some());

    assert_eq!(
        node.attr_value("width").unwrap(),
        Bytes::from_str("400px", &shared_encoding)
    );

    assert_eq!(
        node.attr_value("height").unwrap(),
        Bytes::from_str("400px", &shared_encoding)
    );

    node_allocator.borrow_mut().deallocate(node);
}
