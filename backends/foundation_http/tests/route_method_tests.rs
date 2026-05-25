//! Tests for RouteMethod<S> — generic handler storage per HTTP method.

use foundation_http::shared::router::RouteMethod;
use foundation_netio::simple_http::SimpleMethod;

// -----------------------------------------------------------------------
// Tests

#[test]
fn test_route_method_empty() {
    let rm: RouteMethod<String> = RouteMethod::empty();
    assert!(!rm.has_any());
}

#[test]
fn test_route_method_set_and_get() {
    let mut rm = RouteMethod::empty();
    rm.set_method(SimpleMethod::GET, "get_handler".to_string());
    assert!(rm.has_any());
    assert_eq!(rm.get_method(&SimpleMethod::GET).unwrap(), "get_handler");
}

#[test]
fn test_route_method_all_standard_methods() {
    let mut rm = RouteMethod::empty();
    let methods = [
        SimpleMethod::GET,
        SimpleMethod::POST,
        SimpleMethod::PUT,
        SimpleMethod::DELETE,
        SimpleMethod::PATCH,
        SimpleMethod::HEAD,
        SimpleMethod::OPTIONS,
        SimpleMethod::CONNECT,
        SimpleMethod::TRACE,
    ];

    for (i, method) in methods.iter().enumerate() {
        rm.set_method(method.clone(), format!("handler_{i}"));
    }

    for (i, method) in methods.iter().enumerate() {
        assert_eq!(rm.get_method(method).unwrap(), format!("handler_{i}"));
    }
}

#[test]
fn test_route_method_custom() {
    let mut rm = RouteMethod::empty();
    rm.set_method(SimpleMethod::Custom("PROPFIND".to_string()), "propfind_handler".to_string());
    assert!(rm.has_any());
    assert_eq!(
        rm.get_method(&SimpleMethod::Custom("PROPFIND".to_string())).unwrap(),
        "propfind_handler"
    );
}

#[test]
fn test_route_method_custom_mismatch() {
    let mut rm = RouteMethod::empty();
    rm.set_method(SimpleMethod::Custom("PROPFIND".to_string()), "propfind".to_string());

    // Different custom method should not match
    assert!(rm.get_method(&SimpleMethod::Custom("MKCOL".to_string())).is_err());
}

#[test]
fn test_route_method_get_missing_returns_err() {
    let rm: RouteMethod<String> = RouteMethod::empty();
    let result = rm.get_method(&SimpleMethod::GET);
    assert!(result.is_err());
}

#[test]
fn test_route_method_take() {
    let mut rm1 = RouteMethod::empty();
    rm1.set_method(SimpleMethod::GET, "get_1".to_string());

    let mut rm2 = RouteMethod::empty();
    rm2.set_method(SimpleMethod::POST, "post_2".to_string());

    rm1.take(rm2);

    assert_eq!(rm1.get_method(&SimpleMethod::GET).unwrap(), "get_1");
    assert_eq!(rm1.get_method(&SimpleMethod::POST).unwrap(), "post_2");
}

#[test]
fn test_route_method_take_preserves_existing() {
    let mut rm1 = RouteMethod::empty();
    rm1.set_method(SimpleMethod::GET, "get_1".to_string());
    rm1.set_method(SimpleMethod::PUT, "put_1".to_string());

    let mut rm2 = RouteMethod::empty();
    rm2.set_method(SimpleMethod::GET, "get_2".to_string()); // should NOT overwrite
    rm2.set_method(SimpleMethod::POST, "post_2".to_string());

    rm1.take(rm2);

    assert_eq!(rm1.get_method(&SimpleMethod::GET).unwrap(), "get_1"); // original preserved
    assert_eq!(rm1.get_method(&SimpleMethod::PUT).unwrap(), "put_1");
    assert_eq!(rm1.get_method(&SimpleMethod::POST).unwrap(), "post_2");
}

#[test]
fn test_route_method_take_custom() {
    let mut rm1 = RouteMethod::empty();
    let mut rm2 = RouteMethod::empty();
    rm2.set_method(SimpleMethod::Custom("PROPFIND".to_string()), "propfind".to_string());

    rm1.take(rm2);
    assert_eq!(
        rm1.get_method(&SimpleMethod::Custom("PROPFIND".to_string())).unwrap(),
        "propfind"
    );
}

#[test]
fn test_route_method_clone() {
    let mut rm = RouteMethod::empty();
    rm.set_method(SimpleMethod::GET, "get".to_string());
    rm.set_method(SimpleMethod::POST, "post".to_string());

    let cloned = rm.clone();
    assert_eq!(cloned.get_method(&SimpleMethod::GET).unwrap(), "get");
    assert_eq!(cloned.get_method(&SimpleMethod::POST).unwrap(), "post");
}

#[test]
fn test_route_method_has_any_multiple() {
    let mut rm = RouteMethod::empty();
    assert!(!rm.has_any());

    rm.set_method(SimpleMethod::GET, "get".to_string());
    assert!(rm.has_any());

    rm.set_method(SimpleMethod::POST, "post".to_string());
    assert!(rm.has_any());
}

#[test]
fn test_route_method_has_any_custom() {
    let mut rm = RouteMethod::empty();
    rm.set_method(SimpleMethod::Custom("TEST".to_string()), "test".to_string());
    assert!(rm.has_any());
}
