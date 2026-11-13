//! F27b/F27c tests: JS/TS/Python/Go extraction + clustering + analysis.

use foundation_vectors::code_graph::{
    extract_go_file, extract_js_file, extract_py_file, extract_ts_file, NodeKind, Relation,
};

const JS_SOURCE: &str = r"
import { helper, formatName } from './utils';
import Config from './config';

class Database {
    constructor(connectionString) {
        this.connectionString = connectionString;
    }
    async query(sql) {
        const results = await this.execute(sql);
        return results.map(r => formatName(r));
    }
    async execute(sql) { return []; }
}

const processData = (data) => {
    const parsed = JSON.parse(data);
    helper(parsed);
    return parsed;
};

export default Database;
";

#[test]
fn js_extracts_classes() {
    let ext = extract_js_file("src/database.js", JS_SOURCE).unwrap();
    let class_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_ne!(
        class_nodes,
        [] as [&foundation_vectors::code_graph::GraphNode; 0]
    );
    let db = class_nodes.iter().find(|n| n.label == "Database").unwrap();
    assert_eq!(db.kind, NodeKind::Struct);
}

#[test]
fn js_extracts_functions_and_methods() {
    let ext = extract_js_file("src/database.js", JS_SOURCE).unwrap();
    let func_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_ne!(
        func_nodes,
        [] as [&foundation_vectors::code_graph::GraphNode; 0]
    );
    let names: Vec<_> = func_nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(names.contains(&"processData"));
}

#[test]
fn js_extracts_imports() {
    let ext = extract_js_file("src/database.js", JS_SOURCE).unwrap();
    let import_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Imports)
        .collect();
    assert_ne!(
        import_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

#[test]
fn js_extracts_inheritance() {
    let ext = extract_js_file(
        "src/logger.js",
        r"
class Logger extends EventEmitter {
    log(msg) { this.emit('log', msg); }
}
",
    )
    .unwrap();
    let inherit_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Inherits)
        .collect();
    assert_ne!(
        inherit_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

#[test]
fn js_raw_calls_extraction() {
    let ext = extract_js_file("src/app.js", JS_SOURCE).unwrap();
    assert!(!ext.raw_calls.is_empty());
    let callee_names: Vec<_> = ext
        .raw_calls
        .iter()
        .map(|c| c.callee_name.as_str())
        .collect();
    assert!(callee_names.contains(&"formatName"));
}

const TS_SOURCE: &str = r"
import { User, UserService } from './user';
import { Repository } from './repo';

abstract class BaseController {
    abstract handle(req: Request): Promise<Response>;
}

class UserController extends BaseController {
    private repo: Repository;
    async handle(req: Request): Promise<Response> {
        const users = await this.repo.findAll();
        return Response.json(users);
    }
}

export { UserController };
";

#[test]
fn ts_extracts_classes() {
    let ext = extract_ts_file("src/controller.ts", TS_SOURCE).unwrap();
    let class_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert!(class_nodes.len() >= 2);
}

#[test]
fn ts_extracts_methods() {
    let ext = extract_ts_file("src/controller.ts", TS_SOURCE).unwrap();
    let method_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_ne!(
        method_nodes,
        [] as [&foundation_vectors::code_graph::GraphNode; 0]
    );
    let names: Vec<_> = method_nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(names.contains(&"handle"));
}

#[test]
fn ts_extracts_inheritance() {
    let ext = extract_ts_file("src/controller.ts", TS_SOURCE).unwrap();
    let inherit_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Inherits)
        .collect();
    assert_ne!(
        inherit_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

#[test]
fn ts_raw_calls_extraction() {
    // `BaseController.handle` and `UserController.handle` share a name: each
    // method keeps its own node, and the body of the second one is walked.
    let ext = extract_ts_file("src/controller.ts", TS_SOURCE).unwrap();
    let callee_names: Vec<_> = ext
        .raw_calls
        .iter()
        .map(|c| c.callee_name.as_str())
        .collect();
    assert!(
        callee_names.contains(&"findAll"),
        "expected a raw call to `findAll`, got {callee_names:?}"
    );
    assert!(callee_names.contains(&"json"));
}

#[test]
fn methods_with_the_same_name_in_two_classes_have_two_ids() {
    let ext = extract_ts_file("src/controller.ts", TS_SOURCE).unwrap();
    let handles: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method && n.label == "handle")
        .collect();
    assert_eq!(handles.len(), 2, "one node per class: {handles:?}");
    assert_ne!(handles[0].id, handles[1].id);
    let ids: std::collections::HashSet<_> = ext.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids.len(), ext.nodes.len(), "node ids are unique");
}

const PY_SOURCE: &str = r#"
from typing import List, Optional
from database import Connection, query

class UserRepository:
    def __init__(self, conn: Connection):
        self.conn = conn
    def find_by_id(self, id: int) -> Optional[dict]:
        return query(self.conn, "SELECT * FROM users WHERE id = ?", id)
    def find_all(self) -> List[dict]:
        return query(self.conn, "SELECT * FROM users")

class CachedUserRepository(UserRepository):
    def __init__(self, conn: Connection, cache: dict):
        super().__init__(conn)
        self.cache = cache
    def find_by_id(self, id: int) -> Optional[dict]:
        if id in self.cache:
            return self.cache[id]
        return super().find_by_id(id)
"#;

#[test]
fn py_extracts_classes() {
    let ext = extract_py_file("src/repo.py", PY_SOURCE).unwrap();
    let class_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert!(class_nodes.len() >= 2);
}

#[test]
fn py_extracts_imports() {
    let ext = extract_py_file("src/repo.py", PY_SOURCE).unwrap();
    let import_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Imports)
        .collect();
    assert_ne!(
        import_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

#[test]
fn py_extracts_inheritance() {
    let ext = extract_py_file("src/repo.py", PY_SOURCE).unwrap();
    let inherit_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Inherits)
        .collect();
    assert_ne!(
        inherit_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

const GO_SOURCE: &str = r#"
package main

import (
    "fmt"
    "net/http"
    "github.com/gin-gonic/gin"
)

type User struct {
    ID   int
    Name string
}

type Admin struct {
    User
    Level int
}

func (u *User) GetID() int {
    return u.ID
}

func main() {
    fmt.Println("hello")
    http.Get("https://example.com")
}
"#;

#[test]
fn go_extracts_structs() {
    let ext = extract_go_file("main.go", GO_SOURCE).unwrap();
    let struct_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert!(struct_nodes.len() >= 2);
}

#[test]
fn go_extracts_functions() {
    let ext = extract_go_file("main.go", GO_SOURCE).unwrap();
    let func_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    let names: Vec<_> = func_nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(names.contains(&"main"));
}

#[test]
fn go_extracts_methods() {
    let ext = extract_go_file("main.go", GO_SOURCE).unwrap();
    let method_nodes: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    let names: Vec<_> = method_nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(names.contains(&"GetID"));
}

#[test]
fn go_extracts_imports() {
    let ext = extract_go_file("main.go", GO_SOURCE).unwrap();
    let import_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Imports)
        .collect();
    assert_ne!(
        import_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

#[test]
fn go_extracts_inheritance() {
    let ext = extract_go_file("main.go", GO_SOURCE).unwrap();
    let inherit_edges: Vec<_> = ext
        .edges
        .iter()
        .filter(|e| e.2.relation == Relation::Inherits)
        .collect();
    assert_ne!(
        inherit_edges,
        [] as [&(
            std::string::String,
            std::string::String,
            foundation_vectors::code_graph::GraphEdge
        ); 0]
    );
}

// ---------------------------------------------------------------------------
// Clustering and analysis (F27c)
// ---------------------------------------------------------------------------

use foundation_vectors::code_graph::{CodeGraph, FileExtraction};

#[test]
fn clustering_returns_communities() {
    let graph = make_test_graph();
    let result = graph.cluster();
    assert!(result.num_communities >= 1);
    assert!(result.num_communities <= 6);
}

#[test]
fn god_nodes_returns_top_degree_nodes() {
    let graph = make_god_graph();
    let gods = graph.god_nodes(1);
    assert_eq!(gods.len(), 1);
    assert_eq!(gods[0].label, "central");
}

#[test]
fn surprising_connections_finds_cross_community_edges() {
    let graph = make_test_graph();
    let result = graph.cluster();
    let surprises = graph.surprising_connections(Some(&result.communities));
    assert!(!surprises.is_empty() || result.num_communities <= 1);
}

fn make_test_graph() -> CodeGraph {
    let mut extraction = FileExtraction {
        file_path: "test.go".to_string(),
        ..Default::default()
    };
    for (i, name) in ["a", "b", "c", "x", "y", "z"].iter().enumerate() {
        extraction
            .nodes
            .push(foundation_vectors::code_graph::GraphNode {
                id: format!("test_{name}"),
                label: name.to_string(),
                kind: NodeKind::Function,
                source_file: "test.go".to_string(),
                source_line: i + 1,
                rationale: None,
            });
        extraction.seen_ids.insert(format!("test_{name}"), true);
    }
    for (src, tgt) in [
        ("a", "b"),
        ("b", "c"),
        ("a", "c"),
        ("x", "y"),
        ("y", "z"),
        ("x", "z"),
    ] {
        extraction.edges.push((
            format!("test_{src}"),
            format!("test_{tgt}"),
            foundation_vectors::code_graph::GraphEdge {
                relation: Relation::Calls,
                confidence: foundation_vectors::code_graph::Confidence::Extracted,
                source_file: "test.go".to_string(),
                source_line: 1,
                weight: 1.0,
            },
        ));
    }
    extraction.edges.push((
        "test_c".to_string(),
        "test_x".to_string(),
        foundation_vectors::code_graph::GraphEdge {
            relation: Relation::Calls,
            confidence: foundation_vectors::code_graph::Confidence::Extracted,
            source_file: "test.go".to_string(),
            source_line: 1,
            weight: 0.1,
        },
    ));
    CodeGraph::build(vec![extraction])
}

fn make_god_graph() -> CodeGraph {
    let mut extraction = FileExtraction {
        file_path: "test.go".to_string(),
        ..Default::default()
    };
    for name in ["central", "a", "b", "c", "d"] {
        extraction
            .nodes
            .push(foundation_vectors::code_graph::GraphNode {
                id: format!("test_{name}"),
                label: name.to_string(),
                kind: NodeKind::Function,
                source_file: "test.go".to_string(),
                source_line: 1,
                rationale: None,
            });
        extraction.seen_ids.insert(format!("test_{name}"), true);
    }
    for other in ["a", "b", "c", "d"] {
        extraction.edges.push((
            format!("test_{other}"),
            "test_central".to_string(),
            foundation_vectors::code_graph::GraphEdge {
                relation: Relation::Calls,
                confidence: foundation_vectors::code_graph::Confidence::Extracted,
                source_file: "test.go".to_string(),
                source_line: 1,
                weight: 1.0,
            },
        ));
    }
    CodeGraph::build(vec![extraction])
}

#[test]
fn rust_methods_with_the_same_name_in_two_impls_have_two_ids() {
    let source = "struct A; struct B; \
                  impl A { fn run(&self) { helper_a(); } } \
                  impl B { fn run(&self) { helper_b(); } }";
    let ext =
        foundation_vectors::code_graph::rust_walker::extract_rust_file("src/m.rs", source).unwrap();
    let runs: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method && n.label == "run")
        .collect();
    assert_eq!(runs.len(), 2, "{runs:?}");
    let callees: Vec<_> = ext
        .raw_calls
        .iter()
        .map(|c| c.callee_name.as_str())
        .collect();
    assert!(
        callees.contains(&"helper_a") && callees.contains(&"helper_b"),
        "{callees:?}"
    );
}

#[test]
fn go_methods_with_the_same_name_on_two_types_have_two_ids() {
    let source = "package m\ntype A struct{}\ntype B struct{}\n\
                  func (a *A) Run() { helperA() }\nfunc (b *B) Run() { helperB() }\n";
    let ext = extract_go_file("m.go", source).unwrap();
    let runs: Vec<_> = ext
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method && n.label == "Run")
        .collect();
    assert_eq!(runs.len(), 2, "{runs:?}");
}
