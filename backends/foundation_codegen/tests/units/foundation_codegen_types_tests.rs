//! Unit tests for core types in `foundation_codegen`.
//!
//! Validates Display, equality, hashing, and clone for
//! `ItemKind`, `Location`, and `AttributeValue`.

use std::collections::HashSet;
use std::path::PathBuf;

use foundation_codegen::{AttributeValue, ItemKind, Location};

// -- ItemKind Display --

#[test]
fn item_kind_display_produces_rust_keywords() {
    assert_eq!(ItemKind::Struct.to_string(), "struct");
    assert_eq!(ItemKind::Enum.to_string(), "enum");
    assert_eq!(ItemKind::Trait.to_string(), "trait");
    assert_eq!(ItemKind::Function.to_string(), "fn");
    assert_eq!(ItemKind::Impl.to_string(), "impl");
    assert_eq!(ItemKind::TypeAlias.to_string(), "type");
}

// -- ItemKind equality and hashing --

#[test]
fn item_kind_same_variant_is_equal() {
    assert_eq!(ItemKind::Struct, ItemKind::Struct);
}

#[test]
fn item_kind_different_variants_are_not_equal() {
    assert_ne!(ItemKind::Struct, ItemKind::Enum);
}

#[test]
fn item_kind_deduplicates_in_hashset() {
    let mut set = HashSet::new();
    set.insert(ItemKind::Struct);
    set.insert(ItemKind::Struct);
    assert_eq!(set.len(), 1);
}

// -- Location Display --

#[test]
fn location_display_formats_as_path_line_column() {
    let loc = Location {
        file_path: PathBuf::from("/src/main.rs"),
        line: 42,
        column: 5,
    };
    assert_eq!(loc.to_string(), "/src/main.rs:42:5");
}

// -- Location equality and clone --

#[test]
fn location_clone_produces_equal_copy() {
    let loc = Location {
        file_path: PathBuf::from("/src/lib.rs"),
        line: 1,
        column: 1,
    };
    let cloned = loc.clone();
    assert_eq!(loc, cloned);
}

// -- AttributeValue equality --

#[test]
fn attribute_value_string_equality() {
    assert_eq!(
        AttributeValue::String("hello".to_string()),
        AttributeValue::String("hello".to_string()),
    );
}

#[test]
fn attribute_value_bool_equality() {
    assert_eq!(AttributeValue::Bool(true), AttributeValue::Bool(true));
    assert_ne!(AttributeValue::Bool(true), AttributeValue::Bool(false));
}

#[test]
fn attribute_value_int_equality() {
    assert_eq!(AttributeValue::Int(42), AttributeValue::Int(42));
}

#[test]
fn attribute_value_ident_equality() {
    assert_eq!(
        AttributeValue::Ident("Foo".to_string()),
        AttributeValue::Ident("Foo".to_string()),
    );
}

#[test]
fn attribute_value_flag_equality() {
    assert_eq!(AttributeValue::Flag, AttributeValue::Flag);
}

#[test]
fn attribute_value_list_equality() {
    let list = AttributeValue::List(vec![AttributeValue::Flag, AttributeValue::Bool(true)]);
    let same = AttributeValue::List(vec![AttributeValue::Flag, AttributeValue::Bool(true)]);
    assert_eq!(list, same);
}

#[test]
fn attribute_value_clone_produces_equal_copy() {
    let val = AttributeValue::String("test".to_string());
    assert_eq!(val.clone(), val);
}
