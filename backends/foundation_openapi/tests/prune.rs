//! Tests for the closure pruner (spec-56 feature 00 §2).
//!
//! Two invariants, and they pull in opposite directions:
//!
//! - **nothing missing** — every `$ref` left in the output resolves, or the
//!   generated code does not compile. `dangling_refs` is how that is asserted,
//!   not by eyeballing.
//! - **nothing extra** — an unselected type is *absent*. Easy to lose: a pruner
//!   that keeps everything satisfies the first invariant perfectly.

use foundation_openapi::{
    component_count, dangling_refs, prune_to_selection, unreachable_components, Selection,
};
use serde_json::{json, Value};

/// Two endpoints, a shared error envelope, a chain (`Server` → `Datacenter` →
/// `Location`), a recursive type, and components nothing touches.
fn spec() -> Value {
    json!({
        "openapi": "3.0.3",
        "servers": [{"url": "https://api.example.com"}],
        "paths": {
            "/servers": {
                "get": {
                    "operationId": "list-servers",
                    "tags": ["Servers"],
                    "parameters": [{ "$ref": "#/components/parameters/Page" }],
                    "responses": {
                        "200": { "content": { "application/json": {
                            "schema": { "$ref": "#/components/schemas/Server" } } } },
                        "429": { "$ref": "#/components/responses/RateLimited" }
                    }
                }
            },
            "/kubernetes": {
                "get": {
                    "operationId": "list-clusters",
                    "tags": ["Kubernetes"],
                    "responses": { "200": { "content": { "application/json": {
                        "schema": { "$ref": "#/components/schemas/Cluster" } } } } }
                }
            }
        },
        "components": {
            "schemas": {
                "Server":     { "type": "object", "properties": {
                                  "dc": { "$ref": "#/components/schemas/Datacenter" } } },
                "Datacenter": { "type": "object", "properties": {
                                  "loc": { "$ref": "#/components/schemas/Location" } } },
                "Location":   { "type": "object", "properties": { "city": { "type": "string" } } },
                "Tree":       { "type": "object", "properties": {
                                  "child": { "$ref": "#/components/schemas/Tree" } } },
                "Cluster":    { "type": "object", "properties": { "name": { "type": "string" } } },
                "Unused":     { "type": "object", "properties": { "x": { "type": "string" } } },
                "Error":      { "type": "object", "properties": { "msg": { "type": "string" } } }
            },
            "responses": {
                "RateLimited": { "description": "429", "content": { "application/json": {
                    "schema": { "$ref": "#/components/schemas/Error" } } } },
                "NotUsed": { "description": "nobody refs this" }
            },
            "parameters": {
                "Page":   { "name": "page", "in": "query", "schema": { "type": "integer" } },
                "Unused": { "name": "nope", "in": "query", "schema": { "type": "string" } }
            }
        }
    })
}

fn schemas(spec: &Value) -> Vec<String> {
    spec.pointer("/components/schemas")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn keeps_the_transitive_chain_and_drops_the_rest() {
    let mut spec = spec();
    let stats = prune_to_selection(&mut spec, &Selection::all().paths(["/servers"]));

    let kept = schemas(&spec);
    // Reached through Server -> Datacenter -> Location.
    assert!(kept.contains(&"Server".to_string()));
    assert!(kept.contains(&"Datacenter".to_string()), "one hop");
    assert!(kept.contains(&"Location".to_string()), "two hops — the closure is transitive");
    // Reached through the shared 429 response envelope.
    assert!(kept.contains(&"Error".to_string()), "reached via components/responses");

    // Nothing extra: this is the half a keep-everything pruner would fail.
    assert!(!kept.contains(&"Cluster".to_string()), "belongs to the unselected path");
    assert!(!kept.contains(&"Unused".to_string()), "referenced by nothing");
    assert!(!kept.contains(&"Tree".to_string()), "recursive, but unreachable");
    assert!(stats.components_removed > 0);
}

#[test]
fn follows_refs_into_responses_and_parameters_not_just_schemas() {
    // DigitalOcean's shape: 3758 refs into components/responses, 833 into
    // parameters. Only `schemas` is modelled in the typed spec, which is why the
    // pruner walks raw JSON.
    let mut spec = spec();
    prune_to_selection(&mut spec, &Selection::all().paths(["/servers"]));

    assert!(spec.pointer("/components/responses/RateLimited").is_some(), "kept: the 429 refs it");
    assert!(spec.pointer("/components/parameters/Page").is_some(), "kept: the operation refs it");
    assert!(spec.pointer("/components/responses/NotUsed").is_none(), "dropped");
    assert!(spec.pointer("/components/parameters/Unused").is_none(), "dropped");
}

#[test]
fn leaves_no_dangling_refs() {
    // The invariant that keeps the output compiling.
    let mut spec = spec();
    prune_to_selection(&mut spec, &Selection::all().paths(["/servers"]));
    assert_eq!(dangling_refs(&spec), Vec::<String>::new());
}

#[test]
fn a_recursive_type_terminates_and_is_kept_when_reached() {
    let mut spec = spec();
    spec["paths"]["/servers"]["get"]["responses"]["200"]["content"]["application/json"]["schema"] =
        json!({ "$ref": "#/components/schemas/Tree" });

    // If the walk did not guard against cycles this would hang rather than fail.
    prune_to_selection(&mut spec, &Selection::all().paths(["/servers"]));

    assert!(schemas(&spec).contains(&"Tree".to_string()), "self-referential, still reachable");
    assert_eq!(dangling_refs(&spec), Vec::<String>::new());
}

#[test]
fn follows_a_discriminator_mapping() {
    // Hetzner's *only* refs live in discriminator mappings (35 of them). A walk
    // that only follows `$ref` would drop the mapped schema and leave a
    // discriminator pointing at nothing.
    let mut spec = spec();
    spec["components"]["schemas"]["Service"] = json!({
        "oneOf": [{ "$ref": "#/components/schemas/ServiceTCP" }],
        "discriminator": { "propertyName": "protocol",
                           "mapping": { "tcp": "#/components/schemas/ServiceTCP" } }
    });
    spec["components"]["schemas"]["ServiceTCP"] = json!({ "type": "object" });
    spec["paths"]["/servers"]["get"]["responses"]["200"]["content"]["application/json"]["schema"] =
        json!({ "$ref": "#/components/schemas/Service" });

    prune_to_selection(&mut spec, &Selection::all().paths(["/servers"]));

    assert!(schemas(&spec).contains(&"ServiceTCP".to_string()), "mapping is a ref site");
    assert_eq!(dangling_refs(&spec), Vec::<String>::new());
}

#[test]
fn drops_operations_but_keeps_a_path_that_still_has_one() {
    let mut spec = spec();
    spec["paths"]["/servers"]["post"] = json!({
        "operationId": "create-server",
        "responses": { "200": { "content": { "application/json": {
            "schema": { "$ref": "#/components/schemas/Unused" } } } } }
    });

    let stats = prune_to_selection(&mut spec, &Selection::all().operations(["list-servers"]));

    assert!(spec.pointer("/paths/~1servers/get").is_some(), "the selected operation stays");
    assert!(spec.pointer("/paths/~1servers/post").is_none(), "its sibling goes");
    assert_eq!(stats.operations_removed, 1);
    assert!(
        !schemas(&spec).contains(&"Unused".to_string()),
        "and the type only that sibling reached goes with it"
    );
}

#[test]
fn an_empty_selection_still_drops_orphans() {
    // Linode ships 77 component schemas its spec references nowhere (0 $refs in
    // the whole document). Selecting everything should still not carry them.
    let mut spec = spec();
    prune_to_selection(&mut spec, &Selection::all());

    assert!(spec.pointer("/paths/~1kubernetes").is_some(), "nothing was selected away");
    assert!(!schemas(&spec).contains(&"Unused".to_string()), "but orphans still go");
    assert!(!schemas(&spec).contains(&"Tree".to_string()));
}

#[test]
fn unreachable_components_reports_what_prune_would_drop() {
    let orphans = unreachable_components(&spec());
    assert!(orphans.contains(&"#/components/schemas/Unused".to_string()));
    assert!(orphans.contains(&"#/components/responses/NotUsed".to_string()));
    assert!(!orphans.contains(&"#/components/schemas/Server".to_string()));
}

#[test]
fn dangling_refs_finds_a_broken_pointer() {
    // Proves the invariant check has teeth — if it cannot fail, it proves nothing.
    let mut spec = spec();
    spec["paths"]["/servers"]["get"]["responses"]["200"]["content"]["application/json"]["schema"] =
        json!({ "$ref": "#/components/schemas/DoesNotExist" });

    assert_eq!(dangling_refs(&spec), vec!["#/components/schemas/DoesNotExist".to_string()]);
}

// ── the real specs ───────────────────────────────────────────────────────────

const LINODE: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                      src.linode/linode-api-openapi/openapi.json";

/// The whole feature 00 pipeline on the real 9.3 MB spec:
/// **select → canonicalise → prune**, and the result must be self-contained.
#[test]
fn the_full_pipeline_on_the_real_linode_spec() {
    let Ok(raw) = std::fs::read_to_string(LINODE) else {
        eprintln!("SKIP: Linode spec not present");
        return;
    };
    let mut spec: Value = serde_json::from_str(&raw).expect("parses");

    let paths_before = spec["paths"].as_object().unwrap().len();
    let schemas_before = component_count(&spec, "schemas");

    let selection = Selection::all().paths([
        "/{apiVersion}/linode/instances",
        "/{apiVersion}/linode/instances/{linodeId}",
        "/{apiVersion}/profile/sshkeys",
    ]);

    // 1. select  2. canonicalise (hoists inline schemas the kept paths own)
    foundation_openapi::prune_to_selection(&mut spec, &selection);
    foundation_openapi::ensure_servers(&mut spec, "https://api.linode.com");
    foundation_openapi::canonicalize_operations(&mut spec);
    // 3. prune again: canonicalisation may have made earlier orphans reachable,
    //    and nothing else drops what it did not.
    let stats = foundation_openapi::prune_to_selection(&mut spec, &selection);

    assert_eq!(spec["paths"].as_object().unwrap().len(), 3, "of {paths_before}");
    assert_eq!(
        foundation_openapi::validate_canonical(&spec),
        Ok(()),
        "the output is in the shape the generator needs"
    );
    assert_eq!(dangling_refs(&spec), Vec::<String>::new(), "self-contained");

    let kept = component_count(&spec, "schemas");
    assert!(
        kept < 60,
        "6 endpoints should carry a few dozen schemas, not the spec's {schemas_before} \
         originals plus 1042 hoisted — got {kept}"
    );
    assert_eq!(
        unreachable_components(&spec),
        Vec::<String>::new(),
        "nothing is carried that nothing reaches (Linode ships {schemas_before} orphans): {stats:?}"
    );
}
