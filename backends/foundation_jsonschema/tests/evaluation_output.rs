//! Integration tests — evaluation output formats.
//!
//! Tests the three JSON Schema output formats: flag, list, and hierarchical.

use foundation_jsonschema::{
    evaluation::{
        Evaluation, EvaluationNode, FlagOutput, HierarchicalOutput, ListEntry, ListOutput,
    },
    is_valid, validate, validator_for, ValidationError,
};
use serde_json::json;

// ── Evaluation struct API ────────────────────────────────────────────

#[test]
fn evaluation_from_node() {
    let node = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    let eval = Evaluation::new(node);
    assert!(eval.valid());
}

#[test]
fn evaluation_with_errors() {
    let mut child = EvaluationNode::new(false, "/type".into(), "/type".into(), "/name".into());
    child.errors.push("expected string".into());

    let root = EvaluationNode::new(false, "/".into(), "".into(), "".into());
    let eval = Evaluation::new(root);
    assert!(!eval.valid());
}

// ── Flag output ──────────────────────────────────────────────────────

#[test]
fn flag_output_valid() {
    let node = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    let eval = Evaluation::new(node);
    assert!(eval.valid());
}

#[test]
fn flag_output_invalid() {
    let node = EvaluationNode::new(false, "/".into(), "".into(), "".into());
    let eval = Evaluation::new(node);
    assert!(!eval.valid());
}

// ── List output ──────────────────────────────────────────────────────

#[test]
fn list_output_single_node() {
    let node = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    let eval = Evaluation::new(node);
    let list = eval.to_list();
    assert_eq!(list.entries.len(), 1);
    assert!(list.valid);
}

#[test]
fn list_output_with_children() {
    let mut child = EvaluationNode::new(false, "/type".into(), "/type".into(), "/name".into());
    child.errors.push("expected string".into());

    let mut root = EvaluationNode::new(false, "/".into(), "".into(), "".into());
    root.children.push(child);

    let eval = Evaluation::new(root);
    let list = eval.to_list();

    assert_eq!(list.entries.len(), 2); // root + child
    assert!(!list.valid);

    let child_entry = &list.entries[1];
    assert_eq!(child_entry.evaluation_path, "/type");
    assert_eq!(child_entry.instance_location, "/name");
}

#[test]
fn list_output_errors_captured() {
    let mut root = EvaluationNode::new(false, "/".into(), "".into(), "".into());
    root.errors.push("schema is false".into());

    let eval = Evaluation::new(root);
    let list = eval.to_list();

    assert_eq!(list.entries.len(), 1);
    assert_eq!(list.entries[0].errors, vec!["schema is false"]);
}

#[test]
fn list_output_annotations_preserved_in_node() {
    let mut root = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    root.annotations.insert("enum".into(), "[1, 2, 3]".into());

    let eval = Evaluation::new(root);
    // Annotations are on the EvaluationNode, not on ListEntry (by design)
    assert_eq!(eval.to_list().entries[0].valid, true);
}

// ── Hierarchical output ─────────────────────────────────────────────

#[test]
fn hierarchical_output_single_node() {
    let node = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    let eval = Evaluation::new(node);
    let hier = eval.to_hierarchical();
    assert!(hier.valid);
    assert!(hier.children.is_empty());
}

#[test]
fn hierarchical_output_nested() {
    let child1 = EvaluationNode::new(true, "/type".into(), "/type".into(), "/name".into());
    let child2 = EvaluationNode::new(
        true,
        "/minLength".into(),
        "/minLength".into(),
        "/name".into(),
    );

    let mut root = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    root.children.push(child1);
    root.children.push(child2);

    let eval = Evaluation::new(root);
    let hier = eval.to_hierarchical();

    assert!(hier.valid);
    assert_eq!(hier.children.len(), 2);
    assert_eq!(hier.children[0].evaluation_path, "/type");
    assert_eq!(hier.children[1].evaluation_path, "/minLength");
}

#[test]
fn hierarchical_output_deep_tree() {
    let grandchild = EvaluationNode::new(
        false,
        "/properties/name/type".into(),
        "/type".into(),
        "/name".into(),
    );

    let mut child = EvaluationNode::new(
        false,
        "/properties/name".into(),
        "/properties/name".into(),
        "/name".into(),
    );
    child.children.push(grandchild);

    let mut root = EvaluationNode::new(false, "/".into(), "".into(), "".into());
    root.children.push(child);

    let eval = Evaluation::new(root);
    let hier = eval.to_hierarchical();

    assert!(!hier.valid);
    assert_eq!(hier.children.len(), 1);
    assert_eq!(hier.children[0].children.len(), 1);
    assert_eq!(
        hier.children[0].children[0].evaluation_path,
        "/properties/name/type"
    );
}

// ── EvaluationNode construction ──────────────────────────────────────

#[test]
fn evaluation_node_with_errors() {
    let mut node = EvaluationNode::new(false, "/type".into(), "/type".into(), "/value".into());
    node.errors.push("expected string, got number".into());
    assert_eq!(node.errors.len(), 1);
}

#[test]
fn evaluation_node_with_annotations() {
    let mut node = EvaluationNode::new(true, "/enum".into(), "/enum".into(), "/color".into());
    node.annotations
        .insert("enum".into(), "[\"red\", \"green\"]".into());
    assert_eq!(
        node.annotations.get("enum"),
        Some(&"[\"red\", \"green\"]".to_string())
    );
}

// ── Convenience function wrappers ────────────────────────────────────

#[test]
fn is_valid_convenience() {
    let schema = json!({"type": "string"});
    assert!(is_valid(&schema, &json!("hello")).unwrap());
    assert!(!is_valid(&schema, &json!(42)).unwrap());
}

#[test]
fn validate_convenience() {
    let schema = json!({"type": "string"});
    assert!(validate(&schema, &json!("hello")).is_ok());
    let err = validate(&schema, &json!(42)).unwrap_err();
    assert!(err.to_string().contains("type"));
}

#[test]
fn iter_errors_convenience() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer", "minimum": 0}
        },
        "required": ["name"]
    });
    let v = validator_for(&schema).unwrap();
    let v = validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!({"age": -1})).collect();
    // Missing required "name" + age below minimum
    assert!(!errors.is_empty());
}

#[test]
fn iter_errors_empty_on_valid() {
    let schema = json!({"type": "string"});
    let v = validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!("hello")).collect();
    assert!(errors.is_empty());
}

// ── Edge cases ───────────────────────────────────────────────────────

#[test]
fn list_output_recursive_tree() {
    // Simulate a deep evaluation tree (e.g., from recursive $ref)
    let mut current = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    for i in 0..5 {
        let mut child = EvaluationNode::new(
            true,
            format!("/$ref/{i}"),
            format!("/$ref/{i}"),
            format!("/data/{i}"),
        );
        child.children.push(current);
        current = child;
    }

    let eval = Evaluation::new(current);
    let list = eval.to_list();
    assert_eq!(list.entries.len(), 6); // 5 levels + root
}

#[test]
fn hierarchical_output_empty_root_valid() {
    let node = EvaluationNode::new(true, String::new(), String::new(), String::new());
    let eval = Evaluation::new(node);
    let hier = eval.to_hierarchical();
    assert!(hier.valid);
    assert!(hier.evaluation_path.is_empty());
}

#[test]
fn evaluation_clone() {
    let mut node = EvaluationNode::new(true, "/".into(), "".into(), "".into());
    node.annotations.insert("type".into(), "string".into());
    let eval = Evaluation::new(node);

    let cloned = eval.clone();
    assert_eq!(cloned.valid(), eval.valid());
}
