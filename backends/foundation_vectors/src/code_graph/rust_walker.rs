use tree_sitter::{Node, Parser};

use super::types::{
    Confidence, FileExtraction, GraphEdge, GraphNode, NodeKind, RawCall, Relation,
    file_stem_qualified, make_id,
};

pub fn extract_rust_file(path: &str, source: &str) -> Result<FileExtraction, String> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
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
            "struct_item" => {
                extract_type_item(&child, source, file_path, stem, parent_id, extraction, NodeKind::Struct);
            }
            "enum_item" => {
                extract_type_item(&child, source, file_path, stem, parent_id, extraction, NodeKind::Enum);
            }
            "trait_item" => {
                extract_trait(&child, source, file_path, stem, parent_id, extraction);
            }
            "function_item" => {
                extract_function(&child, source, file_path, stem, parent_id, extraction, NodeKind::Function);
            }
            "impl_item" => {
                extract_impl(&child, source, file_path, stem, parent_id, extraction);
            }
            "use_declaration" => {
                extract_use(&child, source, file_path, stem, parent_id, extraction);
            }
            "const_item" => {
                extract_simple_item(&child, source, file_path, stem, parent_id, extraction, NodeKind::Constant);
            }
            "static_item" => {
                extract_simple_item(&child, source, file_path, stem, parent_id, extraction, NodeKind::Static);
            }
            "type_item" => {
                extract_simple_item(&child, source, file_path, stem, parent_id, extraction, NodeKind::TypeAlias);
            }
            "macro_definition" => {
                extract_simple_item(&child, source, file_path, stem, parent_id, extraction, NodeKind::Macro);
            }
            "mod_item" => {
                extract_mod(&child, source, file_path, stem, parent_id, extraction);
            }
            _ => {
                walk_node(child, source, file_path, stem, parent_id, extraction);
            }
        }
    }
}

fn node_name(node: &Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or("").to_string())
}

fn extract_type_item(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    kind: NodeKind,
) {
    let Some(name) = node_name(node, source) else {
        return;
    };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) {
        return;
    }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;
    let rationale = extract_rationale_above(node, source);

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name,
        kind,
        source_file: file_path.to_string(),
        source_line: line,
        rationale,
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
        walk_node(body, source, file_path, stem, &id, extraction);
    }
}

fn extract_trait(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let Some(name) = node_name(node, source) else {
        return;
    };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) {
        return;
    }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;
    let rationale = extract_rationale_above(node, source);

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name,
        kind: NodeKind::Trait,
        source_file: file_path.to_string(),
        source_line: line,
        rationale,
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

    // Supertrait bounds: trait Foo: Bar + Baz
    if let Some(bounds) = node.child_by_field_name("bounds") {
        let mut bc = bounds.walk();
        for bound_child in bounds.children(&mut bc) {
            if bound_child.kind() == "type_identifier"
                || bound_child.kind() == "scoped_type_identifier"
            {
                let super_name = bound_child
                    .utf8_text(source.as_bytes())
                    .unwrap_or("")
                    .to_string();
                let super_label = super_name.split("::").last().unwrap_or(&super_name);
                let super_id = make_id(stem, super_label);
                extraction.edges.push((
                    id.clone(),
                    super_id,
                    GraphEdge {
                        relation: Relation::Inherits,
                        confidence: Confidence::Extracted,
                        source_file: file_path.to_string(),
                        source_line: line,
                        weight: 1.0,
                    },
                ));
            }
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        extract_trait_body(body, source, file_path, stem, &id, extraction);
    }
}

fn extract_trait_body(
    body: Node,
    source: &str,
    file_path: &str,
    stem: &str,
    trait_id: &str,
    extraction: &mut FileExtraction,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "function_item" || child.kind() == "function_signature_item" {
            extract_function(
                &child,
                source,
                file_path,
                stem,
                trait_id,
                extraction,
                NodeKind::Method,
            );
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
    let Some(name) = node_name(node, source) else {
        return;
    };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) {
        return;
    }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;
    let rationale = extract_rationale_above(node, source);

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name,
        kind,
        source_file: file_path.to_string(),
        source_line: line,
        rationale,
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

fn extract_impl(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let line = node.start_position().row + 1;

    let type_name = node
        .child_by_field_name("type")
        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or("").to_string());
    let trait_name = node
        .child_by_field_name("trait")
        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or("").to_string());

    let impl_label = match (&trait_name, &type_name) {
        (Some(tr), Some(ty)) => format!("impl_{tr}_for_{ty}"),
        (None, Some(ty)) => format!("impl_{ty}"),
        _ => return,
    };
    let impl_id = make_id(stem, &impl_label);

    if !extraction.seen_ids.contains_key(&impl_id) {
        extraction.seen_ids.insert(impl_id.clone(), true);
        extraction.nodes.push(GraphNode {
            id: impl_id.clone(),
            label: impl_label,
            kind: NodeKind::Impl,
            source_file: file_path.to_string(),
            source_line: line,
            rationale: None,
        });
        extraction.edges.push((
            parent_id.to_string(),
            impl_id.clone(),
            GraphEdge {
                relation: Relation::Contains,
                confidence: Confidence::Extracted,
                source_file: file_path.to_string(),
                source_line: line,
                weight: 1.0,
            },
        ));
    }

    if let Some(trait_n) = &trait_name {
        let trait_short = trait_n.split("::").last().unwrap_or(trait_n);
        let trait_id = make_id(stem, trait_short);
        if let Some(type_n) = &type_name {
            let type_short = type_n.split('<').next().unwrap_or(type_n);
            let type_id = make_id(stem, type_short);
            extraction.edges.push((
                type_id,
                trait_id,
                GraphEdge {
                    relation: Relation::Implements,
                    confidence: Confidence::Extracted,
                    source_file: file_path.to_string(),
                    source_line: line,
                    weight: 1.0,
                },
            ));
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_item" {
                extract_function(
                    &child,
                    source,
                    file_path,
                    stem,
                    &impl_id,
                    extraction,
                    NodeKind::Method,
                );
            }
        }
    }
}

fn extract_use(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let text = node
        .utf8_text(source.as_bytes())
        .unwrap_or("")
        .to_string();
    let line = node.start_position().row + 1;

    let path = text
        .trim_start_matches("use ")
        .trim_end_matches(';')
        .trim();

    let segments: Vec<&str> = path.split("::").collect();
    if segments.is_empty() {
        return;
    }

    let import_target = segments.last().unwrap_or(&"");
    let target_id = make_id(stem, import_target);

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

fn extract_simple_item(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    kind: NodeKind,
) {
    let Some(name) = node_name(node, source) else {
        return;
    };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) {
        return;
    }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;
    let rationale = extract_rationale_above(node, source);

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name,
        kind,
        source_file: file_path.to_string(),
        source_line: line,
        rationale,
    });

    extraction.edges.push((
        parent_id.to_string(),
        id,
        GraphEdge {
            relation: Relation::Contains,
            confidence: Confidence::Extracted,
            source_file: file_path.to_string(),
            source_line: line,
            weight: 1.0,
        },
    ));
}

fn extract_mod(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    let Some(name) = node_name(node, source) else {
        return;
    };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) {
        return;
    }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name,
        kind: NodeKind::Module,
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
        walk_node(body, source, file_path, stem, &id, extraction);
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
                let is_member_call = func.kind() == "field_expression";
                let callee_name = if is_member_call {
                    func.child_by_field_name("field")
                        .map(|f| {
                            f.utf8_text(source.as_bytes())
                                .unwrap_or("")
                                .to_string()
                        })
                        .unwrap_or_default()
                } else {
                    let text = func
                        .utf8_text(source.as_bytes())
                        .unwrap_or("")
                        .to_string();
                    text.split("::").last().unwrap_or(&text).to_string()
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

fn extract_rationale_above(node: &Node, source: &str) -> Option<String> {
    let mut rationale_parts = Vec::new();
    let mut prev = node.prev_sibling();

    while let Some(sib) = prev {
        if sib.kind() == "line_comment" || sib.kind() == "block_comment" {
            let text = sib
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_string();
            let trimmed = text.trim_start_matches("//").trim_start_matches("/*").trim_end_matches("*/").trim();
            let prefixes = [
                "NOTE:", "IMPORTANT:", "HACK:", "WHY:", "RATIONALE:", "TODO:", "FIXME:",
                "SAFETY:", "INVARIANT:",
            ];
            if prefixes.iter().any(|p| trimmed.starts_with(p)) {
                rationale_parts.push(trimmed.to_string());
            }
        } else if sib.kind() == "attribute_item" || sib.kind() == "inner_attribute_item" {
            // skip attributes, keep looking
        } else {
            break;
        }
        prev = sib.prev_sibling();
    }

    if rationale_parts.is_empty() {
        None
    } else {
        rationale_parts.reverse();
        Some(rationale_parts.join("\n"))
    }
}
