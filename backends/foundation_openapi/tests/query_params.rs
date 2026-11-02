//! What the generator turns query parameters into: `Option<bool>` for
//! booleans (serialised `true`/`false`) and repeated keys for arrays.
//!
//! WHY: The generator once typed every query parameter as `Option<String>`.
//! A `type: boolean` parameter (`stream`, `one_shot`) came out as a string the
//! caller had to stringify by hand, and a `type: array` parameter that Docker
//! expects repeated (`t`) could not be sent at all.
//!
//! WHAT: Generates one small spec through the public [`UnifiedGenerator`] and
//! checks the emitted args struct and the emitted request function.

use foundation_openapi::unified::{AnalysisOptions, UnifiedGenerator};

/// Generate `spec` and return every emitted module, concatenated.
fn generate(tag: &str, spec: &serde_json::Value) -> String {
    let out = std::env::temp_dir().join(format!(
        "foundation_openapi_query_{tag}_{}_{}",
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

/// A spec whose one endpoint has a boolean and an array query parameter.
fn query_spec() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.0.0",
        "info": {"title": "query", "version": "1"},
        "servers": [{"url": "http://localhost"}],
        "paths": {
            "/things": {"get": {
                "operationId": "ThingsGet",
                "parameters": [
                    {"name": "verbose", "in": "query", "schema": {"type": "boolean"}},
                    {"name": "names", "in": "query", "schema": {"type": "array", "items": {"type": "string"}}},
                    {"name": "limit", "in": "query", "schema": {"type": "string"}}
                ],
                "responses": {"200": {"description": "ok"}}
            }}
        }
    })
}

#[test]
fn boolean_query_param_is_bool() {
    let all = generate("query_bool", &query_spec());
    // The args struct carries a real bool, not an `Option<String>`.
    assert!(
        all.contains("pub verbose: Option<bool>,"),
        "boolean parameter must generate as Option<bool>:\n{all}"
    );
    // And it is serialised as `true`/`false`, not a string.
    assert!(
        all.contains("args.verbose.map(|b| if b { \"true\" } else { \"false\" })"),
        "boolean parameter must serialise to true/false:\n{all}"
    );
}

#[test]
fn array_query_param_repeats_key() {
    let all = generate("query_array", &query_spec());
    // The args struct carries a Vec, not an `Option<String>`.
    assert!(
        all.contains("pub names: Vec<String>,"),
        "array parameter must generate as Vec<String>:\n{all}"
    );
    // The request function sends one `names=` pair per value.
    assert!(
        all.contains("for value in &args.names {"),
        "array parameter must be sent as repeated keys:\n{all}"
    );
    // A plain string parameter is unchanged.
    assert!(
        all.contains("pub limit: Option<String>,"),
        "string parameter must stay Option<String>:\n{all}"
    );
}
