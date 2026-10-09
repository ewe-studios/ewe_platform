//! Go language bespoke extractor (F27b).
//!
//! Go has a hand-written walker (not LanguageConfig-based) because its AST
//! patterns — package declarations, import groups, method receivers, interface
//! definitions — require Go-specific handling.

use tree_sitter::{Node, Parser};

use super::types::{
    Confidence, FileExtraction, GraphEdge, GraphNode, NodeKind, RawCall, Relation,
    file_stem_qualified, make_id,
};

pub fn extract_go_file(path: &str, source: &str) -> Result<FileExtraction, String> {
    let mut parser = Parser::new();
    let language = tree_sitter_go::LANGUAGE;
    parser
        .set_language(&language.into())
        .map_err(|e| format!("set language: {e}"))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "parse failed".to_string())?;

    let stem = file_stem_qualified(path);
    let file_id = make_id(&stem, "");
    let file_id = file_id.trim_end_matches('_').to_string();

    let mut extraction = FileExtraction {
        file_path: path.to_string(),
        ..Default::default()
    };

    extraction.nodes.push(GraphNode {
        id: file_id.clone(),
        label: path.to_string(),
        kind: NodeKind::File,
        source_file: path.to_string(),
        source_line: 0,
        rationale: None,
    });
    extraction.seen_ids.insert(file_id.clone(), true);

    walk_node(
        tree.root_node(),
        source,
        path,
        &stem,
        &file_id,
        &mut extraction,
    );

    Ok(extraction)
}

fn walk_node(
    node: Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_declaration" => {
                extract_type_declaration(&child, source, file_path, stem, parent_id, extraction);
            }
            "function_declaration" => {
                extract_function(&child, source, file_path, stem, parent_id, extraction, NodeKind::Function);
            }
            "method_declaration" => {
                extract_method(&child, source, file_path, stem, parent_id, extraction);
            }
            "import_declaration" => {
                extract_import(&child, source, file_path, stem, parent_id, extraction);
            }
            "var_declaration" | "const_declaration" => {
                extract_const_or_var(&child, source, file_path, stem, parent_id, extraction);
            }
            _ => {
                walk_node(child, source, file_path, stem, parent_id, extraction);
            }
        }
    }
}

fn extract_type_declaration(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_spec" {
            let name = child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let id = make_id(stem, name);
            if extraction.seen_ids.contains_key(&id) {
                continue;
            }
            extraction.seen_ids.insert(id.clone(), true);
            let line = child.start_position().row + 1;

            // Determine kind from the type body
            let kind = if let Some(type_node) = child.child_by_field_name("type") {
                match type_node.kind() {
                    "struct_type" => NodeKind::Struct,
                    "interface_type" => NodeKind::Trait, // Go interfaces ≈ Rust traits
                    _ => NodeKind::Struct,
                }
            } else {
                NodeKind::Struct
            };

            extraction.nodes.push(GraphNode {
                id: id.clone(),
                label: name.to_string(),
                kind,
                source_file: file_path.to_string(),
                source_line: line,
                rationale: None,
            });

            extraction.edges.push((
                parent_id.to_string(),
                id.clone(),
                GraphEdge {
                    relation: Relation::Contains,
                    confidence: Confidence::Extracted,
                    source_file: file_path.to_string(),
                    source_line: line,
                    weight: 1.0,
                },
            ));

            // Walk the type body for embedded types (inheritance) and methods
            if let Some(type_body) = child.child_by_field_name("type") {
                // Embedded fields (struct embedding = inheritance in Go)
                // struct_type → field_declaration_list → field_declaration
                // For interfaces: interface_type → method_spec_list → method_spec
                let mut tc = type_body.walk();
                for tc_child in type_body.children(&mut tc) {
                    // Handle both direct field_declaration and field_declaration_list wrapper
                    if tc_child.kind() == "field_declaration" {
                        process_field_for_inheritance(&tc_child, source, file_path, stem, &id, extraction);
                    } else if tc_child.kind() == "field_declaration_list"
                        || tc_child.kind() == "method_spec_list"
                    {
                        let mut fc = tc_child.walk();
                        for fc_child in tc_child.children(&mut fc) {
                            if fc_child.kind() == "field_declaration" {
                                process_field_for_inheritance(&fc_child, source, file_path, stem, &id, extraction);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Process a field_declaration for embedded type detection (Go embedding = inheritance).
fn process_field_for_inheritance(
    tc_child: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    type_id: &str,
    extraction: &mut FileExtraction,
) {
    let field_name = tc_child.child_by_field_name("name");
    let type_node = tc_child.child_by_field_name("type");
    if let Some(type_n) = type_node {
        if field_name.is_none() {
            // Embedded type → inherits edge
            let embedded_name = type_n
                .utf8_text(source.as_bytes())
                .unwrap_or("");
            let super_label = embedded_name.split('.').last().unwrap_or(embedded_name);
            let super_id = make_id(stem, super_label);
            extraction.edges.push((
                type_id.to_string(),
                super_id,
                GraphEdge {
                    relation: Relation::Inherits,
                    confidence: Confidence::Inferred,
                    source_file: file_path.to_string(),
                    source_line: tc_child.start_position().row + 1,
                    weight: 1.0,
                },
            ));
        }
    }
}

fn extract_function(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    kind: NodeKind,
) {
    let Some(name) = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
    else { return };

    let id = make_id(stem, name);
    if extraction.seen_ids.contains_key(&id) { return }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name.to_string(),
        kind,
        source_file: file_path.to_string(),
        source_line: line,
        rationale: None,
    });

    extraction.edges.push((
        parent_id.to_string(),
        id.clone(),
        GraphEdge {
            relation: Relation::Contains,
            confidence: Confidence::Extracted,
            source_file: file_path.to_string(),
            source_line: line,
            weight: 1.0,
        },
    ));

    if let Some(body) = node.child_by_field_name("body") {
        extract_calls(&body, source, file_path, &id, extraction);
    }
}

fn extract_method(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let Some(name) = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
    else { return };

    // Method has a receiver: `func (r *Receiver) MethodName() { ... }`
    let receiver_name = if let Some(recv) = node.child_by_field_name("receiver") {
        recv.child_by_field_name("name")
            .or_else(|| recv.child_by_field_name("type"))
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .map(|s| {
                // Strip pointer prefix: *Receiver → Receiver
                s.strip_prefix('*').unwrap_or(s)
            })
    } else {
        None
    };

    let parent = if let Some(recv_name) = receiver_name {
        make_id(stem, recv_name)
    } else {
        parent_id.to_string()
    };

    let id = make_id(stem, name);
    if extraction.seen_ids.contains_key(&id) { return }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name.to_string(),
        kind: NodeKind::Method,
        source_file: file_path.to_string(),
        source_line: line,
        rationale: None,
    });

    extraction.edges.push((
        parent,
        id.clone(),
        GraphEdge {
            relation: Relation::Contains,
            confidence: Confidence::Extracted,
            source_file: file_path.to_string(),
            source_line: line,
            weight: 1.0,
        },
    ));

    if let Some(body) = node.child_by_field_name("body") {
        extract_calls(&body, source, file_path, &id, extraction);
    }
}

fn extract_import(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let line = node.start_position().row + 1;
    let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();

    // Go imports: `import "pkg/path"` or `import ( "pkg" "pkg2" )`
    // Extract the last path segment as the imported name
    for import_line in text.lines() {
        let trimmed = import_line.trim().trim_matches('"');
        if trimmed.is_empty() || trimmed == "import" || trimmed.starts_with('(') || trimmed == ")" {
            continue;
        }
        // Get the last path segment
        let import_name = trimmed.split('/').last().unwrap_or(trimmed);
        let target_id = make_id(stem, import_name);
        extraction.edges.push((
            parent_id.to_string(),
            target_id,
            GraphEdge {
                relation: Relation::Imports,
                confidence: Confidence::Extracted,
                source_file: file_path.to_string(),
                source_line: line,
                weight: 0.5,
            },
        ));
    }
}

fn extract_const_or_var(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "var_spec" || child.kind() == "const_spec" {
            let name_node = child.child_by_field_name("name");
            if let Some(name) = name_node.and_then(|n| n.utf8_text(source.as_bytes()).ok()) {
                let kind = if child.kind() == "const_spec" {
                    NodeKind::Constant
                } else {
                    NodeKind::Static
                };
                let id = make_id(stem, name);
                if extraction.seen_ids.contains_key(&id) { continue }
                extraction.seen_ids.insert(id.clone(), true);

                extraction.nodes.push(GraphNode {
                    id: id.clone(),
                    label: name.to_string(),
                    kind,
                    source_file: file_path.to_string(),
                    source_line: child.start_position().row + 1,
                    rationale: None,
                });

                extraction.edges.push((
                    parent_id.to_string(),
                    id,
                    GraphEdge {
                        relation: Relation::Contains,
                        confidence: Confidence::Extracted,
                        source_file: file_path.to_string(),
                        source_line: child.start_position().row + 1,
                        weight: 1.0,
                    },
                ));
            }
        }
    }
}

fn extract_calls(
    node: &Node,
    source: &str,
    file_path: &str,
    caller_id: &str,
    extraction: &mut FileExtraction,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_expression" {
            let func_node = child.child_by_field_name("function");
            if let Some(func) = func_node {
                let is_member_call = func.kind() == "selector_expression";
                let callee_name = if is_member_call {
                    // Go selector: `pkg.Func()` or `receiver.Method()`
                    func.child_by_field_name("field")
                        .map(|f| f.utf8_text(source.as_bytes()).unwrap_or("").to_string())
                        .unwrap_or_default()
                } else {
                    let text = func.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    text.split('.').last().unwrap_or(&text).to_string()
                };

                if !callee_name.is_empty() {
                    extraction.raw_calls.push(RawCall {
                        caller_id: caller_id.to_string(),
                        callee_name,
                        is_member_call,
                        source_file: file_path.to_string(),
                        source_line: child.start_position().row + 1,
                    });
                }
            }
        }
        extract_calls(&child, source, file_path, caller_id, extraction);
    }
}
