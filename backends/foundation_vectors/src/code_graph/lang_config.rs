//! Generic `LanguageConfig` walker for JS/TS/Python (F27b).

use tree_sitter::{Node, Parser};

use super::types::{
    Confidence, FileExtraction, GraphEdge, GraphNode, NodeKind, RawCall, Relation,
    file_stem_qualified, make_id,
};

// ---------------------------------------------------------------------------
// LanguageConfig
// ---------------------------------------------------------------------------

pub struct LanguageConfig {
    pub language: fn() -> tree_sitter::Language,
    pub class_types: &'static [&'static str],
    pub function_types: &'static [&'static str],
    pub import_types: &'static [&'static str],
    pub call_types: &'static [&'static str],
    pub accessor_types: &'static [&'static str],
    pub inheritance_fields: &'static [&'static str],
    pub name_field: &'static str,
    pub body_field: &'static str,
    pub call_function_field: &'static str,
    pub call_accessor_field: &'static str,
    pub extra_walk_fn: Option<fn(Node, &str, &str, &str, &str, &mut FileExtraction)>,
}

// ---------------------------------------------------------------------------
// Language configs
// ---------------------------------------------------------------------------

pub fn javascript_config() -> LanguageConfig {
    LanguageConfig {
        language: || tree_sitter_javascript::LANGUAGE.into(),
        class_types: &["class_declaration", "class"],
        function_types: &["function_declaration", "method_definition", "generator_function_declaration"],
        import_types: &["import_statement", "import_declaration"],
        call_types: &["call_expression"],
        accessor_types: &["member_expression"],
        inheritance_fields: &["class_heritage"],
        name_field: "name",
        body_field: "body",
        call_function_field: "function",
        call_accessor_field: "property",
        extra_walk_fn: Some(js_extra_walk),
    }
}

pub fn typescript_config() -> LanguageConfig {
    LanguageConfig {
        language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        class_types: &["class_declaration", "abstract_class_declaration", "class"],
        function_types: &["function_declaration", "method_definition", "generator_function_declaration", "abstract_method_signature"],
        import_types: &["import_statement", "import_declaration"],
        call_types: &["call_expression"],
        accessor_types: &["member_expression"],
        inheritance_fields: &["class_heritage"],
        name_field: "name",
        body_field: "body",
        call_function_field: "function",
        call_accessor_field: "property",
        extra_walk_fn: Some(js_extra_walk),
    }
}

pub fn python_config() -> LanguageConfig {
    LanguageConfig {
        language: || tree_sitter_python::LANGUAGE.into(),
        class_types: &["class_definition"],
        function_types: &["function_definition"],
        import_types: &["import_statement", "import_from_statement"],
        call_types: &["call"],
        accessor_types: &["attribute"],
        inheritance_fields: &["superclasses"],
        name_field: "name",
        body_field: "body",
        call_function_field: "function",
        call_accessor_field: "attribute",
        extra_walk_fn: None,
    }
}

// ---------------------------------------------------------------------------
// Extra walks
// ---------------------------------------------------------------------------

fn js_extra_walk(
    node: Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
) {
    if node.kind() != "variable_declaration" && node.kind() != "lexical_declaration" {
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }
        let name = child
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .unwrap_or("anonymous");

        let mut vc = child.walk();
        let mut func_node: Option<Node> = None;
        for vc_child in child.children(&mut vc) {
            if vc_child.kind() == "arrow_function" || vc_child.kind() == "function_expression" {
                func_node = Some(vc_child);
                break;
            }
        }
        let Some(func) = func_node else { continue };

        let id = make_id(stem, name);
        if extraction.seen_ids.contains_key(&id) {
            continue;
        }
        extraction.seen_ids.insert(id.clone(), true);
        let line = child.start_position().row + 1;
        extraction.nodes.push(GraphNode {
            id: id.clone(),
            label: name.to_string(),
            kind: NodeKind::Function,
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
        if let Some(body) = func.child_by_field_name("body") {
            extract_calls(&body, source, file_path, &id, extraction, &javascript_config());
        }
    }
}

// ---------------------------------------------------------------------------
// Public extract entry points
// ---------------------------------------------------------------------------

pub fn extract_js_file(path: &str, source: &str) -> Result<FileExtraction, String> {
    extract_with_config(path, source, &javascript_config())
}

pub fn extract_ts_file(path: &str, source: &str) -> Result<FileExtraction, String> {
    extract_with_config(path, source, &typescript_config())
}

pub fn extract_py_file(path: &str, source: &str) -> Result<FileExtraction, String> {
    extract_with_config(path, source, &python_config())
}

fn extract_with_config(
    path: &str,
    source: &str,
    config: &LanguageConfig,
) -> Result<FileExtraction, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&(config.language)())
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

    walk_node(tree.root_node(), source, path, &stem, &file_id, &mut extraction, config);

    Ok(extraction)
}

// ---------------------------------------------------------------------------
// Generic walker
// ---------------------------------------------------------------------------

fn walk_node(
    node: Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    config: &LanguageConfig,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();

        if config.class_types.contains(&kind) {
            extract_class(&child, source, file_path, stem, parent_id, extraction, config);
            continue;
        }

        if config.function_types.contains(&kind) {
            extract_function(&child, source, file_path, stem, parent_id, extraction, config, NodeKind::Function);
            continue;
        }

        if config.import_types.contains(&kind) {
            extract_import(&child, source, file_path, stem, parent_id, extraction, config);
            continue;
        }

        if let Some(extra) = config.extra_walk_fn {
            extra(child, source, file_path, stem, parent_id, extraction);
        }

        walk_node(child, source, file_path, stem, parent_id, extraction, config);
    }
}

fn node_name(node: &Node, source: &str, config: &LanguageConfig) -> Option<String> {
    node.child_by_field_name(config.name_field)
        .map(|n| n.utf8_text(source.as_bytes()).unwrap_or("").to_string())
}

fn extract_class(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    config: &LanguageConfig,
) {
    let Some(name) = node_name(node, source, config) else { return };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) { return }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name.clone(),
        kind: NodeKind::Struct,
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

    // Inheritance: try named fields (Python: superclasses) first, then unnamed children (JS/TS: class_heritage)
    let heritage = config.inheritance_fields.iter()
        .find_map(|&f| node.child_by_field_name(f));
    let heritage = heritage.or_else(|| {
        node.children(&mut node.walk())
            .find(|c| c.kind() == "class_heritage")
    });
    if let Some(heritage) = heritage {
        if let Some(super_name) = find_type_identifier(&heritage, source.as_bytes()) {
            let super_label = super_name.split("::").last().unwrap_or(&super_name)
                .split('.').last().unwrap_or(&super_name);
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

    // Walk class body for methods
    if let Some(body) = node.child_by_field_name(config.body_field) {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if config.function_types.contains(&child.kind()) {
                extract_function(&child, source, file_path, stem, &id, extraction, config, NodeKind::Method);
            } else {
                walk_node(child, source, file_path, stem, &id, extraction, config);
            }
        }
    }
}

fn find_type_identifier(node: &Node, source: &[u8]) -> Option<String> {
    let kind = node.kind();
    if kind == "type_identifier" || kind == "identifier" || kind == "scoped_type_identifier" {
        return node.utf8_text(source).ok().map(|s| s.to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(name) = find_type_identifier(&child, source) {
            return Some(name);
        }
    }
    None
}

fn extract_function(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    config: &LanguageConfig,
    kind: NodeKind,
) {
    let Some(name) = node_name(node, source, config) else { return };
    let id = make_id(stem, &name);
    if extraction.seen_ids.contains_key(&id) { return }
    extraction.seen_ids.insert(id.clone(), true);

    let line = node.start_position().row + 1;

    extraction.nodes.push(GraphNode {
        id: id.clone(),
        label: name,
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

    if let Some(body) = node.child_by_field_name(config.body_field) {
        extract_calls(&body, source, file_path, &id, extraction, config);
    }
}

fn extract_import(
    node: &Node,
    source: &str,
    file_path: &str,
    stem: &str,
    parent_id: &str,
    extraction: &mut FileExtraction,
    config: &LanguageConfig,
) {
    let line = node.start_position().row + 1;

    // For JS/TS `import { foo } from 'bar'` and Python `from bar import foo`
    let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
    let target = extract_import_target(&text, config);

    if !target.is_empty() {
        let target_id = make_id(stem, &target);
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

fn extract_import_target(text: &str, config: &LanguageConfig) -> String {
    // Check Python imports FIRST (from bar import foo)
    if config.import_types.contains(&"import_from_statement") {
        if let Some(import_idx) = text.find(" import ") {
            let names = &text[import_idx + 8..];
            return names.split(',').next().unwrap_or("").trim().to_string();
        }
    }
    // JS/TS: import { foo } from 'bar' or import foo from 'bar'
    if let Some(start) = text.find('{') {
        if let Some(end) = text.find('}') {
            let names = &text[start + 1..end];
            return names.split(',').next().unwrap_or("").trim().to_string();
        }
    }
    if let Some(from_idx) = text.find(" from ") {
        let between = text[7..from_idx].trim();
        return between.split(',').next().unwrap_or("").trim().to_string();
    }
    String::new()
}

fn extract_calls(
    node: &Node,
    source: &str,
    file_path: &str,
    caller_id: &str,
    extraction: &mut FileExtraction,
    config: &LanguageConfig,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if config.call_types.contains(&child.kind()) {
            let func_node = child.child_by_field_name(config.call_function_field);
            if let Some(func) = func_node {
                let is_member_call = config.accessor_types.contains(&func.kind());
                let callee_name = if is_member_call {
                    func.child_by_field_name(config.call_accessor_field)
                        .map(|f| f.utf8_text(source.as_bytes()).unwrap_or("").to_string())
                        .unwrap_or_default()
                } else {
                    let text = func.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    text.split('.').last().unwrap_or(&text)
                        .split("::").last().unwrap_or(&text)
                        .to_string()
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
        extract_calls(&child, source, file_path, caller_id, extraction, config);
    }
}
