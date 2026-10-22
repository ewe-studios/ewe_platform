//! What the generator turns a spec's schemas into: field names, schema
//! lookups, `allOf` members and aliases.
//!
//! WHY: Each of these once broke a real provider. Cloudflare and Linode key
//! properties with names that are not Rust identifiers (`13335`, `*`, `+gt`),
//! which were emitted verbatim. Hetzner names a schema
//! `CreateServerResponseServerPublic_net`, which the generator refers to as
//! `…PublicNet`; the lookup missed and 52 of its 100 types became opaque
//! blobs. An `allOf` member given by `$ref` contributed no fields.
//!
//! WHAT: Generates small specs through the public [`UnifiedGenerator`] and
//! checks the emitted types.

use std::sync::OnceLock;

use foundation_openapi::unified::{AnalysisOptions, UnifiedGenerator};

/// Generate `spec` and return every emitted module, concatenated.
fn generate(tag: &str, spec: &serde_json::Value) -> String {
    let out = std::env::temp_dir().join(format!(
        "foundation_openapi_types_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let crate_src = out.join("src");
    UnifiedGenerator::new(out.clone())
        .with_provider_dir(crate_src.clone())
        .generate(tag, &spec.to_string(), &AnalysisOptions::default())
        .expect("generation succeeds");

    let generated = crate_src.join("generated");
    let mut modules: Vec<_> = std::fs::read_dir(&generated)
        .expect("generated dir")
        .map(|entry| entry.expect("entry").path().join("mod.rs"))
        .filter(|module| module.exists())
        .collect();
    modules.sort();
    let mut all = String::new();
    for module in modules {
        all.push_str(&std::fs::read_to_string(&module).expect("read module"));
    }
    std::fs::remove_dir_all(&out).expect("clean up");
    all
}

/// A spec whose one endpoint returns `schema_ref`, with `schemas` as its
/// components.
fn spec(schema_ref: &str, schemas: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.0.0",
        "info": {"title": "types", "version": "1"},
        "servers": [{"url": "http://localhost"}],
        "paths": {
            "/things": {"get": {
                "operationId": "ThingGet",
                "responses": {"200": {"description": "ok", "content": {"application/json": {
                    "schema": {"$ref": format!("#/components/schemas/{schema_ref}")}
                }}}}
            }}
        },
        "components": {"schemas": schemas}
    })
}

/// The body of the generated `struct name`, through its closing brace.
fn struct_body<'a>(all: &'a str, name: &str) -> &'a str {
    all.split(&format!("pub struct {name} {{"))
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .unwrap_or_else(|| panic!("struct {name} not generated:\n{all}"))
}

/// The generated code for one spec exercising every case below, generated
/// once for all the tests.
fn generated() -> &'static str {
    static ALL: OnceLock<String> = OnceLock::new();
    ALL.get_or_init(|| {
        generate(
            "types",
            &spec(
                "Thing",
                serde_json::json!({
                    "Thing": {"type": "object", "properties": {
                        // Cloudflare keys properties by ASN, and by a bare `*`.
                        "13335": {"type": "string"},
                        "*": {"type": "string"},
                        "$metadata": {"type": "string"},
                        // Linode's filter operators.
                        "+gt": {"type": "string"},
                        // DigitalOcean's Postgres settings.
                        "pg_partman_bgw.interval": {"type": "string"},
                        "type": {"type": "string"},
                        "self": {"type": "string"},
                        "account_id": {"type": "string"},
                        "zone": {"type": "string"},
                        "ApiVersion": {"type": "string"},
                        "public_net": {"$ref": "#/components/schemas/Thing_public_net"},
                        "exact": {"$ref": "#/components/schemas/Exact"},
                        "other": {"$ref": "#/components/schemas/exact_other"},
                        "missing": {"$ref": "#/components/schemas/NoSuchType"},
                        "latitude": {"$ref": "#/components/schemas/latitude"},
                        "messages": {"$ref": "#/components/schemas/messages"},
                        "free_form": {"$ref": "#/components/schemas/free_form"},
                        "either": {"$ref": "#/components/schemas/either"},
                        "blank": {"$ref": "#/components/schemas/blank"},
                        "zone_response": {"$ref": "#/components/schemas/zone_response"}
                    }},
                    // Hetzner's shape: the hoisted component carries the
                    // property's underscore.
                    "Thing_public_net": {"type": "object", "properties": {"ipv4": {"type": "string"}}},
                    "Exact": {"type": "object", "properties": {"a": {"type": "string"}}},
                    "exact_other": {"type": "object", "properties": {"b": {"type": "string"}}},
                    "latitude": {"type": "number"},
                    "messages": {"type": "array", "items": {"type": "string"}},
                    "free_form": {"type": "object"},
                    "either": {"oneOf": [{"type": "string"}, {"type": "integer"}]},
                    "blank": {"type": "object", "properties": {}},
                    "api_response_common": {
                        "type": "object",
                        "required": ["success", "errors"],
                        "properties": {
                            "success": {"type": "boolean"},
                            "errors": {"type": "array", "items": {"type": "string"}}
                        }
                    },
                    "zone_response": {"allOf": [
                        {"$ref": "#/components/schemas/api_response_common"},
                        {"properties": {"result": {"type": "string"}}}
                    ]}
                }),
            ),
        )
    })
}

/// Every input below is a real property key from a vendor's spec, and each
/// one used to be emitted verbatim into a struct definition.
#[test]
fn a_property_key_becomes_a_field_name_rust_accepts() {
    let thing = struct_body(generated(), "Thing");
    for (wire, field) in [
        ("13335", "field_13335"),
        // Punctuation sanitises to nothing.
        ("*", "field"),
        // Leading punctuation is dropped; the wire name is kept by the rename.
        ("$metadata", "metadata"),
        ("+gt", "gt"),
        ("pg_partman_bgw.interval", "pg_partman_bgw_interval"),
    ] {
        let expected = format!("#[serde(rename = \"{wire}\")]\n    pub {field}: Option<String>,");
        assert!(
            thing.contains(&expected),
            "{expected} missing from:\n{thing}"
        );
    }
    // Keywords take the raw-identifier form the generator has always used.
    assert!(thing.contains("pub r#type: Option<String>,"), "{thing}");
    assert!(thing.contains("pub _self: Option<String>,"), "{thing}");
}

/// The guarantee that keeps every provider's committed output from moving:
/// only names that were already broken change.
#[test]
fn a_valid_key_is_left_exactly_alone() {
    let thing = struct_body(generated(), "Thing");
    for name in ["account_id", "zone"] {
        assert!(
            thing.contains(&format!("pub {name}: Option<String>,")),
            "{thing}"
        );
        assert!(
            !thing.contains(&format!("#[serde(rename = \"{name}\")]")),
            "{name} needs no rename:\n{thing}"
        );
    }
    // Casing conversion is unchanged.
    assert!(
        thing.contains("#[serde(rename = \"ApiVersion\")]\n    pub api_version: Option<String>,"),
        "{thing}"
    );
}

/// The reference site pascal-cases `Thing_public_net` to `ThingPublicNet`,
/// which no amount of snake-casing turns back into the key: the lookup must
/// match in the pascal-cased space, or the type is emitted as an opaque blob.
#[test]
fn a_schema_key_resolves_even_when_pascal_casing_is_not_invertible() {
    let all = generated();
    assert!(
        struct_body(all, "ThingPublicNet").contains("pub ipv4: Option<String>,"),
        "{all}"
    );
    assert!(
        struct_body(all, "Thing").contains("pub public_net: Option<ThingPublicNet>,"),
        "{all}"
    );
    // A name nothing declares must still miss — the fallback is a real signal.
    assert!(
        all.contains("/// `NoSuchType` type (no schema in the spec).\npub type NoSuchType = serde_json::Value;"),
        "{all}"
    );
}

#[test]
fn a_direct_key_match_still_wins() {
    let all = generated();
    assert!(
        struct_body(all, "Exact").contains("pub a: Option<String>,"),
        "{all}"
    );
    // Cloudflare's snake_case keys, via the existing rule.
    assert!(
        struct_body(all, "ExactOther").contains("pub b: Option<String>,"),
        "{all}"
    );
}

#[test]
fn an_all_of_member_given_by_ref_contributes_its_fields() {
    let zone = struct_body(generated(), "ZoneResponse");
    assert!(zone.contains("pub success: bool,"), "{zone}");
    assert!(zone.contains("pub errors: Vec<String>,"), "{zone}");
    assert!(zone.contains("pub result: Option<String>,"), "{zone}");
}

#[test]
fn schemas_without_properties_alias_what_they_are() {
    let all = generated();
    assert!(all.contains("pub type Latitude = f64;"), "{all}");
    assert!(all.contains("pub type Messages = Vec<String>;"), "{all}");
    assert!(
        all.contains("pub type FreeForm = serde_json::Value;"),
        "{all}"
    );
    assert!(
        all.contains("pub type Either = serde_json::Value;"),
        "{all}"
    );
    // An object with no fields is a struct, not an alias.
    assert!(!all.contains("pub type Blank "), "{all}");
    assert!(all.contains("pub struct Blank"), "{all}");
}
