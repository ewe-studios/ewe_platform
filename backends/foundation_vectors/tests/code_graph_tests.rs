use foundation_vectors::code_graph::*;

const SAMPLE_RUST: &str = r#"
use std::collections::HashMap;

// WHY: core data structure for the parser
pub struct Parser {
    config: Config,
    buffer: Vec<u8>,
}

pub enum Token {
    Ident(String),
    Number(i64),
}

pub trait Tokenizer {
    fn tokenize(&self, input: &str) -> Vec<Token>;
}

impl Tokenizer for Parser {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        Vec::new()
    }
}

impl Parser {
    pub fn new(config: Config) -> Self {
        let map = HashMap::new();
        Self { config, buffer: Vec::new() }
    }

    pub fn parse(&self, input: &str) -> Result<(), String> {
        let tokens = self.tokenize(input);
        process_tokens(&tokens);
        Ok(())
    }
}

fn process_tokens(tokens: &[Token]) -> usize {
    tokens.len()
}

pub const MAX_BUFFER: usize = 4096;

pub struct Config {
    pub max_depth: usize,
}
"#;

#[test]
fn extract_rust_file_nodes() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();

    let node_labels: Vec<&str> = ext.nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(node_labels.contains(&"Parser"), "should find struct Parser");
    assert!(node_labels.contains(&"Token"), "should find enum Token");
    assert!(node_labels.contains(&"Tokenizer"), "should find trait Tokenizer");
    assert!(node_labels.contains(&"process_tokens"), "should find fn process_tokens");
    assert!(node_labels.contains(&"MAX_BUFFER"), "should find const MAX_BUFFER");
    assert!(node_labels.contains(&"Config"), "should find struct Config");
}

#[test]
fn extract_rust_file_node_kinds() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();

    let find = |label: &str| ext.nodes.iter().find(|n| n.label == label).unwrap();
    assert_eq!(find("Parser").kind, NodeKind::Struct);
    assert_eq!(find("Token").kind, NodeKind::Enum);
    assert_eq!(find("Tokenizer").kind, NodeKind::Trait);
    assert_eq!(find("process_tokens").kind, NodeKind::Function);
    assert_eq!(find("MAX_BUFFER").kind, NodeKind::Constant);
}

#[test]
fn extract_rust_file_has_file_node() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let file_nodes: Vec<_> = ext.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(file_nodes.len(), 1);
    assert_eq!(file_nodes[0].source_file, "src/parser.rs");
}

#[test]
fn extract_contains_edges() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let contains_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|(_, _, e)| e.relation == Relation::Contains)
        .collect();
    assert!(contains_edges.len() >= 5, "should have contains edges for structs/fns/etc");
}

#[test]
fn extract_imports_edges() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let import_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|(_, _, e)| e.relation == Relation::Imports)
        .collect();
    assert!(!import_edges.is_empty(), "should have import edges for `use std::collections::HashMap`");
}

#[test]
fn extract_impl_trait_creates_implements_edge() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let impl_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|(_, _, e)| e.relation == Relation::Implements)
        .collect();
    assert!(!impl_edges.is_empty(), "should have implements edge for `impl Tokenizer for Parser`");
}

#[test]
fn extract_raw_calls() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let call_names: Vec<&str> = ext.raw_calls.iter().map(|c| c.callee_name.as_str()).collect();
    assert!(call_names.contains(&"new"), "should find call to HashMap::new");
    assert!(call_names.contains(&"process_tokens"), "should find call to process_tokens");
}

#[test]
fn extract_member_calls_flagged() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let tokenize_call = ext
        .raw_calls
        .iter()
        .find(|c| c.callee_name == "tokenize");
    assert!(tokenize_call.is_some(), "should find self.tokenize call");
    assert!(tokenize_call.unwrap().is_member_call, "self.tokenize should be member call");
}

#[test]
fn extract_rationale() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let parser_node = ext.nodes.iter().find(|n| n.label == "Parser").unwrap();
    assert!(parser_node.rationale.is_some(), "Parser should have rationale from WHY comment");
    assert!(
        parser_node.rationale.as_ref().unwrap().contains("core data structure"),
        "rationale should contain the WHY text"
    );
}

#[test]
fn build_graph_from_single_file() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let graph = CodeGraph::build(vec![ext]);

    assert!(graph.node_count() > 5, "graph should have nodes");
    assert!(graph.edge_count() > 3, "graph should have edges");

    let parser_nodes = graph.find_entity("Parser");
    assert!(!parser_nodes.is_empty(), "should find Parser by name");
    assert_eq!(parser_nodes[0].kind, NodeKind::Struct);
}

#[test]
fn cross_file_call_resolution() {
    let file_a = r#"
pub fn helper() -> i32 { 42 }
"#;
    let file_b = r#"
pub fn caller() {
    helper();
}
"#;
    let ext_a = rust_walker::extract_rust_file("src/a.rs", file_a).unwrap();
    let ext_b = rust_walker::extract_rust_file("src/b.rs", file_b).unwrap();
    let graph = CodeGraph::build(vec![ext_a, ext_b]);

    let caller_node = graph.find_entity("caller");
    assert!(!caller_node.is_empty());

    let helper_node = graph.find_entity("helper");
    assert!(!helper_node.is_empty());

    let callers = graph.callers_of(&helper_node[0].id);
    let caller_ids: Vec<&str> = callers.iter().map(|n| n.label.as_str()).collect();
    assert!(caller_ids.contains(&"caller"), "helper should have caller as a caller (cross-file INFERRED)");
}

#[test]
fn member_call_exclusion_from_cross_file() {
    let file_a = r#"
pub struct Logger;
impl Logger {
    pub fn log(&self) {}
}
"#;
    let file_b = r#"
pub fn main() {
    let l = Logger;
    l.log();
}
"#;
    let ext_a = rust_walker::extract_rust_file("src/a.rs", file_a).unwrap();
    let ext_b = rust_walker::extract_rust_file("src/b.rs", file_b).unwrap();
    let graph = CodeGraph::build(vec![ext_a, ext_b]);

    let log_node = graph.find_entity("log");
    if !log_node.is_empty() {
        let callers = graph.callers_of(&log_node[0].id);
        // member calls should NOT create cross-file INFERRED edges
        let has_cross_file_caller = callers.iter().any(|n| n.source_file == "src/b.rs");
        assert!(!has_cross_file_caller, "member call l.log() should NOT create cross-file edge");
    }
}

#[test]
fn find_entity_partial_match() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let graph = CodeGraph::build(vec![ext]);

    let results = graph.find_entity("process");
    assert!(!results.is_empty(), "should find process_tokens via partial match");
}

#[test]
fn neighborhood_budget() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let graph = CodeGraph::build(vec![ext]);

    let parser_nodes = graph.find_entity("Parser");
    assert!(!parser_nodes.is_empty());

    let subgraph = graph.neighborhood(&parser_nodes[0].id, 200).unwrap();
    assert!(!subgraph.nodes.is_empty());
    assert!(subgraph.token_estimate <= 200 || subgraph.nodes.len() == 1);
}

#[test]
fn neighborhood_unknown_node_errors() {
    let graph = CodeGraph::new();
    let result = graph.neighborhood("nonexistent", 100);
    assert!(result.is_err());
}

#[test]
fn shortest_path_exists() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let graph = CodeGraph::build(vec![ext]);

    let file_nodes: Vec<_> = graph.all_nodes().into_iter().filter(|n| n.kind == NodeKind::File).collect();
    let fn_nodes: Vec<_> = graph.all_nodes().into_iter().filter(|n| n.kind == NodeKind::Function).collect();

    if !file_nodes.is_empty() && !fn_nodes.is_empty() {
        let path = graph.shortest_path(&file_nodes[0].id, &fn_nodes[0].id);
        assert!(path.is_some(), "should find path from file to function via contains");
    }
}

#[test]
fn json_round_trip() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let graph = CodeGraph::build(vec![ext]);

    let json = graph.to_json().unwrap();
    let restored = CodeGraph::from_json(&json).unwrap();

    assert_eq!(graph.node_count(), restored.node_count());
    assert_eq!(graph.edge_count(), restored.edge_count());

    let parser_nodes = restored.find_entity("Parser");
    assert!(!parser_nodes.is_empty());
    assert_eq!(parser_nodes[0].kind, NodeKind::Struct);
}

#[test]
fn make_id_normalization() {
    assert_eq!(make_id("parser", "MyStruct"), "parser_mystruct");
    assert_eq!(make_id("", "hello_world"), "hello_world");
    assert_eq!(make_id("mod", "Foo::Bar"), "mod_foo__bar");
}

#[test]
fn file_stem_qualified_paths() {
    assert_eq!(file_stem_qualified("src/parser.rs"), "src_parser");
    assert_eq!(file_stem_qualified("lib.rs"), "lib");
    assert_eq!(file_stem_qualified("deeply/nested/module.rs"), "nested_module");
}

#[test]
fn normalize_label_strips_special_chars() {
    assert_eq!(normalize_label("MyStruct<T>"), "mystructt");
    assert_eq!(normalize_label("foo::bar"), "foobar");
    assert_eq!(normalize_label("hello_world"), "hello_world");
}

#[test]
fn empty_graph_operations() {
    let graph = CodeGraph::new();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert!(graph.find_entity("anything").is_empty());
    assert!(graph.callers_of("nope").is_empty());
    assert!(graph.shortest_path("a", "b").is_none());
}

#[test]
fn nodes_by_file() {
    let ext = rust_walker::extract_rust_file("src/parser.rs", SAMPLE_RUST).unwrap();
    let graph = CodeGraph::build(vec![ext]);
    let nodes = graph.nodes_by_file("src/parser.rs");
    assert!(!nodes.is_empty());
    assert!(nodes.iter().all(|n| n.source_file == "src/parser.rs"));
}

#[test]
fn callees_of() {
    let source = r#"
fn helper() {}
fn caller() {
    helper();
}
"#;
    let ext = rust_walker::extract_rust_file("src/test.rs", source).unwrap();
    let graph = CodeGraph::build(vec![ext]);
    let caller = graph.find_entity("caller");
    assert!(!caller.is_empty());
    let callees = graph.callees_of(&caller[0].id);
    let callee_labels: Vec<&str> = callees.iter().map(|n| n.label.as_str()).collect();
    assert!(callee_labels.contains(&"helper"));
}
