use foundation_jsonschema::{LazyLocation, Location};

#[test]
fn test_location_empty() {
    let loc = Location::new();
    assert!(loc.as_json_pointer().is_empty());
}

#[test]
fn test_location_push_property() {
    let mut loc = Location::new();
    loc.push_property("foo");
    loc.push_property("bar");
    assert_eq!(loc.as_json_pointer(), "/foo/bar");
}

#[test]
fn test_location_push_index() {
    let mut loc = Location::new();
    loc.push_property("items");
    loc.push_index(3);
    assert_eq!(loc.as_json_pointer(), "/items/3");
}

#[test]
fn test_location_pop() {
    let mut loc = Location::new();
    loc.push_property("foo");
    loc.push_property("bar");
    assert!(loc.pop());
    assert_eq!(loc.as_json_pointer(), "/foo");
}

#[test]
fn test_location_json_pointer_escaping() {
    let mut loc = Location::new();
    loc.push_property("foo/bar");
    loc.push_property("~baz");
    loc.push_index(0);
    // ~0 -> ~, ~1 -> /
    assert_eq!(loc.as_json_pointer(), "/foo~1bar/~0baz/0");
}

#[test]
fn test_location_display() {
    let mut loc = Location::new();
    loc.push_property("foo");
    assert_eq!(format!("{loc}"), "/foo");
}

#[test]
fn test_location_display_root() {
    let loc = Location::new();
    assert_eq!(format!("{loc}"), "/");
}

#[test]
fn test_lazy_location_materialize() {
    let root = LazyLocation::new();
    let a = root.push_property("a");
    let idx = a.push_index(0);
    let b = idx.push_property("b");
    let loc = b.materialize();
    assert_eq!(loc.as_json_pointer(), "/a/0/b");
}

#[test]
fn test_lazy_location_root_materialize() {
    let root = LazyLocation::new();
    let loc = root.materialize();
    assert!(loc.as_json_pointer().is_empty());
}

#[test]
fn test_lazy_location_display() {
    let root = LazyLocation::new();
    let foo = root.push_property("foo");
    assert_eq!(format!("{foo}"), "/foo");
}
