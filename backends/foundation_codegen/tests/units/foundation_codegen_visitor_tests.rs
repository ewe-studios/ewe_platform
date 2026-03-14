//! Unit tests for the `MacroFinder` AST visitor in `foundation_codegen`.
//!
//! Validates that the visitor finds items annotated with a target attribute
//! across all supported item kinds, tracks inline module nesting,
//! and correctly parses attribute arguments.

use std::path::PathBuf;

use foundation_codegen::visitor::MacroFinder;
use foundation_codegen::ItemKind;
use syn::visit::Visit;

/// Helper: parse source and run the visitor, returning found items.
fn scan_source(source: &str, target_attr: &str) -> Vec<foundation_codegen::FoundItem> {
    let ast = syn::parse_file(source).expect("test source should be valid Rust");
    let mut finder = MacroFinder::new(target_attr, &PathBuf::from("test.rs"));
    finder.visit_file(&ast);
    finder.found
}

// -- Valid input: finds annotated struct --

#[test]
fn finds_annotated_struct() {
    let items = scan_source(
        r#"
        #[module]
        pub struct AuthHandler;
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "AuthHandler");
    assert_eq!(items[0].item_kind, ItemKind::Struct);
    assert_eq!(items[0].macro_name, "module");
}

#[test]
fn finds_annotated_enum() {
    let items = scan_source(
        r#"
        #[module]
        pub enum Status { Active, Inactive }
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "Status");
    assert_eq!(items[0].item_kind, ItemKind::Enum);
}

#[test]
fn finds_annotated_trait() {
    let items = scan_source(
        r#"
        #[module]
        pub trait Handler {}
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "Handler");
    assert_eq!(items[0].item_kind, ItemKind::Trait);
}

#[test]
fn finds_annotated_function() {
    let items = scan_source(
        r#"
        #[module]
        pub fn handle_request() {}
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "handle_request");
    assert_eq!(items[0].item_kind, ItemKind::Function);
}

#[test]
fn finds_annotated_type_alias() {
    let items = scan_source(
        r#"
        #[module]
        pub type MyResult = Result<(), String>;
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "MyResult");
    assert_eq!(items[0].item_kind, ItemKind::TypeAlias);
}

// -- Valid input: ignores items without the target attribute --

#[test]
fn ignores_unannotated_items() {
    let items = scan_source(
        r#"
        pub struct Plain;
        #[derive(Debug)]
        pub struct WithDerive;
        #[other_attr]
        pub struct WithOther;
        "#,
        "module",
    );

    assert!(items.is_empty(), "should not match non-target attributes");
}

// -- Valid input: finds multiple annotated items --

#[test]
fn finds_multiple_items_in_one_file() {
    let items = scan_source(
        r#"
        #[module]
        pub struct First;

        pub struct Skipped;

        #[module]
        pub enum Second { A }

        #[module]
        fn third() {}
        "#,
        "module",
    );

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].item_name, "First");
    assert_eq!(items[1].item_name, "Second");
    assert_eq!(items[2].item_name, "third");
}

// -- Inline module tracking --

#[test]
fn tracks_inline_module_nesting() {
    let items = scan_source(
        r#"
        mod outer {
            mod inner {
                #[module]
                pub struct Nested;
            }
        }
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "Nested");
    assert_eq!(items[0].inline_module_path, vec!["outer", "inner"]);
}

#[test]
fn top_level_item_has_empty_module_path() {
    let items = scan_source(
        r#"
        #[module]
        pub struct TopLevel;
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert!(items[0].inline_module_path.is_empty());
}

// -- Attribute argument parsing --

#[test]
fn parses_string_attribute_value() {
    let items = scan_source(
        r#"
        #[module(name = "auth")]
        pub struct Handler;
        "#,
        "module",
    );

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].attributes.get("name"),
        Some(&foundation_codegen::AttributeValue::String("auth".to_string())),
    );
}

#[test]
fn parses_bool_attribute_value() {
    let items = scan_source(
        r#"
        #[module(export = true)]
        pub struct Handler;
        "#,
        "module",
    );

    assert_eq!(
        items[0].attributes.get("export"),
        Some(&foundation_codegen::AttributeValue::Bool(true)),
    );
}

#[test]
fn parses_int_attribute_value() {
    let items = scan_source(
        r#"
        #[module(priority = 5)]
        pub struct Handler;
        "#,
        "module",
    );

    assert_eq!(
        items[0].attributes.get("priority"),
        Some(&foundation_codegen::AttributeValue::Int(5)),
    );
}

#[test]
fn parses_ident_attribute_value() {
    let items = scan_source(
        r#"
        #[module(kind = Handler)]
        pub struct MyHandler;
        "#,
        "module",
    );

    assert_eq!(
        items[0].attributes.get("kind"),
        Some(&foundation_codegen::AttributeValue::Ident("Handler".to_string())),
    );
}

#[test]
fn parses_bare_flag_attribute() {
    let items = scan_source(
        r#"
        #[module(export)]
        pub struct Handler;
        "#,
        "module",
    );

    assert_eq!(
        items[0].attributes.get("export"),
        Some(&foundation_codegen::AttributeValue::Flag),
    );
}

#[test]
fn parses_multiple_attribute_args() {
    let items = scan_source(
        r#"
        #[module(name = "auth", export = true, priority = 5)]
        pub struct Handler;
        "#,
        "module",
    );

    assert_eq!(items[0].attributes.len(), 3);
    assert_eq!(
        items[0].attributes.get("name"),
        Some(&foundation_codegen::AttributeValue::String("auth".to_string())),
    );
}

#[test]
fn parses_nested_list_attribute() {
    let items = scan_source(
        r#"
        #[module(depends_on(auth, api))]
        pub struct Handler;
        "#,
        "module",
    );

    let deps = items[0].attributes.get("depends_on");
    assert!(
        matches!(deps, Some(foundation_codegen::AttributeValue::List(v)) if v.len() == 2),
        "should parse nested list with 2 items"
    );
}

#[test]
fn empty_attribute_produces_empty_map() {
    let items = scan_source(
        r#"
        #[module]
        pub struct Handler;
        "#,
        "module",
    );

    assert!(items[0].attributes.is_empty());
}
