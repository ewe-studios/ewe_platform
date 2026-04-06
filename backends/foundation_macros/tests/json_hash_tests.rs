//! Unit tests for the JsonHash derive macro.

use foundation_macros::JsonHash;
use serde::Serialize;

#[derive(JsonHash, Serialize)]
struct TestResource {
    name: String,
    value: i32,
}

#[test]
fn test_struct_hash_deterministic() {
    let a = TestResource {
        name: "test".into(),
        value: 42,
    };
    let b = TestResource {
        name: "test".into(),
        value: 42,
    };
    assert_eq!(a.struct_hash(), b.struct_hash());
}

#[test]
fn test_struct_hash_different_values() {
    let a = TestResource {
        name: "test".into(),
        value: 42,
    };
    let b = TestResource {
        name: "test".into(),
        value: 43,
    };
    assert_ne!(a.struct_hash(), b.struct_hash());
}

#[test]
fn test_struct_hash_different_types_same_content() {
    #[derive(JsonHash, Serialize)]
    struct TypeA {
        value: i32,
    }

    #[derive(JsonHash, Serialize)]
    struct TypeB {
        value: i32,
    }

    let a = TypeA { value: 42 };
    let b = TypeB { value: 42 };
    // Different struct names = different hashes
    assert_ne!(a.struct_hash(), b.struct_hash());
}

#[test]
fn test_struct_hash_with_nested_struct() {
    #[derive(JsonHash, Serialize, Clone)]
    struct Inner {
        x: i32,
    }

    #[derive(JsonHash, Serialize)]
    struct Outer {
        inner: Inner,
    }

    let inner = Inner { x: 10 };
    let outer = Outer { inner: inner.clone() };

    // Both should hash without panic
    let inner_hash = inner.struct_hash();
    let outer_hash = outer.struct_hash();

    // Hashes should be different (different struct names)
    assert_ne!(inner_hash, outer_hash);

    // Same inner struct should produce same hash
    let inner2 = Inner { x: 10 };
    assert_eq!(inner_hash, inner2.struct_hash());
}

#[test]
fn test_struct_hash_with_option_and_vec() {
    #[derive(JsonHash, Serialize)]
    struct ComplexResource {
        name: String,
        tags: Vec<String>,
        metadata: Option<String>,
    }

    let a = ComplexResource {
        name: "test".into(),
        tags: vec!["a".into(), "b".into()],
        metadata: Some("meta".into()),
    };

    let b = ComplexResource {
        name: "test".into(),
        tags: vec!["a".into(), "b".into()],
        metadata: Some("meta".into()),
    };

    assert_eq!(a.struct_hash(), b.struct_hash());

    // Different tags = different hash
    let c = ComplexResource {
        name: "test".into(),
        tags: vec!["a".into(), "c".into()],
        metadata: Some("meta".into()),
    };

    assert_ne!(a.struct_hash(), c.struct_hash());
}

#[test]
fn test_struct_hash_with_none_option() {
    #[derive(JsonHash, Serialize)]
    struct WithOption {
        name: String,
        value: Option<i32>,
    }

    let a = WithOption {
        name: "test".into(),
        value: None,
    };

    let b = WithOption {
        name: "test".into(),
        value: None,
    };

    assert_eq!(a.struct_hash(), b.struct_hash());

    let c = WithOption {
        name: "test".into(),
        value: Some(42),
    };

    assert_ne!(a.struct_hash(), c.struct_hash());
}

#[test]
fn test_struct_hash_with_empty_vec() {
    #[derive(JsonHash, Serialize)]
    struct WithVec {
        name: String,
        items: Vec<i32>,
    }

    let a = WithVec {
        name: "test".into(),
        items: vec![],
    };

    let b = WithVec {
        name: "test".into(),
        items: vec![],
    };

    assert_eq!(a.struct_hash(), b.struct_hash());

    let c = WithVec {
        name: "test".into(),
        items: vec![1, 2, 3],
    };

    assert_ne!(a.struct_hash(), c.struct_hash());
}

#[test]
fn test_struct_hash_is_stable_across_runs() {
    #[derive(JsonHash, Serialize)]
    struct StableResource {
        id: u64,
        data: String,
    }

    let resource = StableResource {
        id: 12345,
        data: "hello world".into(),
    };

    let hash1 = resource.struct_hash();
    let hash2 = resource.struct_hash();
    let hash3 = resource.struct_hash();

    // Same instance should always produce same hash
    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);

    // Hash should be non-empty
    assert!(!hash1.is_empty());
}

#[test]
fn test_struct_hash_with_bool() {
    #[derive(JsonHash, Serialize)]
    struct WithBool {
        name: String,
        enabled: bool,
    }

    let a = WithBool {
        name: "test".into(),
        enabled: true,
    };

    let b = WithBool {
        name: "test".into(),
        enabled: false,
    };

    assert_ne!(a.struct_hash(), b.struct_hash());

    let c = WithBool {
        name: "test".into(),
        enabled: true,
    };

    assert_eq!(a.struct_hash(), c.struct_hash());
}
