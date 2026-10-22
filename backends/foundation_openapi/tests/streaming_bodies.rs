//! The generator hands streamed bodies back unread, and sends raw request
//! bodies as-is.
//!
//! WHY: A response that is a JSON-lines feed or a tar archive used to come out
//! as `ApiResponse<()>` — the body dropped after the headers, so a client could
//! neither read a pull's progress nor wait for it to finish — or as a JSON
//! parse of a tar. A non-JSON request body (a build context) was not sent at
//! all. Error bodies were read whole.
//!
//! WHAT: Generates a small spec covering each case and checks the emitted
//! functions and types.

use foundation_openapi::unified::{AnalysisOptions, UnifiedGenerator};
use foundation_openapi::{process_spec, ResponseType};

const SPEC: &str = r##"{
  "openapi": "3.0.0",
  "info": {"title": "streams", "version": "1"},
  "servers": [{"url": "http://localhost"}],
  "paths": {
    "/images/create": {"post": {
      "operationId": "ImageCreate",
      "parameters": [{"name": "fromImage", "in": "query", "schema": {"type": "string"}}],
      "requestBody": {"required": false, "content": {"text/plain": {"schema": {"type": "string"}}}},
      "responses": {"200": {"description": "ok", "content": {"application/jsonl": {"schema": {"$ref": "#/components/schemas/CreateImageInfo"}}}}}
    }},
    "/images/{name}/get": {"get": {
      "operationId": "ImageGet",
      "parameters": [{"name": "name", "in": "path", "required": true, "schema": {"type": "string"}}],
      "responses": {"200": {"description": "ok", "content": {"application/x-tar": {"schema": {"type": "string", "format": "binary"}}}}}
    }},
    "/images/get": {"get": {
      "operationId": "ImageGetAll",
      "parameters": [{"name": "names", "in": "query", "schema": {"type": "array", "items": {"type": "string"}}}],
      "responses": {"200": {"description": "ok", "content": {"application/x-tar": {"schema": {"type": "string", "format": "binary"}}}}}
    }},
    "/containers/{id}/archive": {"put": {
      "operationId": "PutContainerArchive",
      "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string"}}],
      "requestBody": {"required": true, "content": {"application/x-tar": {"schema": {"type": "string", "format": "binary"}}}},
      "responses": {"200": {"description": "ok"}}
    }},
    "/containers/create": {"post": {
      "operationId": "ContainerCreate",
      "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ContainerCreateRequest"}}}},
      "responses": {"201": {"description": "ok", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ImageInspect"}}}}}
    }},
    "/images/{name}/json": {"get": {
      "operationId": "ImageInspect",
      "parameters": [{"name": "name", "in": "path", "required": true, "schema": {"type": "string"}}],
      "responses": {"200": {"description": "ok", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ImageInspect"}}}}}
    }}
  },
  "components": {"schemas": {
    "CreateImageInfo": {"type": "object", "properties": {
      "status": {"type": "string"},
      "errorDetail": {"$ref": "#/components/schemas/ErrorDetail"}
    }},
    "ErrorDetail": {"type": "object", "properties": {"message": {"type": "string"}}},
    "ImageInspect": {"type": "object", "properties": {"Id": {"type": "string"}}},
    "ContainerConfig": {"type": "object", "properties": {"Image": {"type": "string"}, "Cmd": {"type": "array", "items": {"type": "string"}}}},
    "ContainerCreateRequest": {"allOf": [
      {"$ref": "#/components/schemas/ContainerConfig"},
      {"type": "object", "properties": {"HostConfig": {"type": "object", "properties": {"Init": {"type": "boolean"}}}}}
    ]}
  }}
}"##;

#[test]
fn streamed_responses_and_raw_request_bodies_are_classified() {
    let processor = process_spec(SPEC).expect("spec parses");
    let endpoints = processor.endpoints();
    let find = |id: &str| {
        endpoints
            .iter()
            .find(|e| e.operation_id == id)
            .unwrap_or_else(|| panic!("{id} extracted"))
    };

    assert_eq!(
        find("ImageCreate").response_type,
        Some(ResponseType::JsonLines("CreateImageInfo".to_string()))
    );
    assert_eq!(find("ImageGet").response_type, Some(ResponseType::Binary));
    assert_eq!(
        find("ImageInspect").response_type,
        Some(ResponseType::Generated("ImageInspect".to_string()))
    );

    let raw = find("PutContainerArchive")
        .raw_request_body
        .as_ref()
        .expect("raw body");
    assert_eq!(raw.media_type, "application/x-tar");
    assert!(raw.required);
    let optional = find("ImageCreate")
        .raw_request_body
        .as_ref()
        .expect("raw body");
    assert_eq!(optional.media_type, "text/plain");
    assert!(!optional.required);
    assert!(find("ImageInspect").raw_request_body.is_none());

    assert_eq!(
        find("ImageGetAll").array_query_params,
        vec!["names".to_string()]
    );
    assert_eq!(find("ImageCreate").array_query_params, Vec::<String>::new());
}

#[test]
fn generated_functions_stream_their_bodies() {
    let out = std::env::temp_dir().join(format!(
        "foundation_openapi_streaming_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let crate_src = out.join("src");
    UnifiedGenerator::new(out.clone())
        .with_provider_dir(crate_src.clone())
        .generate("streams", SPEC, &AnalysisOptions::default())
        .expect("generation succeeds");

    let generated = crate_src.join("generated");
    let mut all = String::new();
    for entry in std::fs::read_dir(&generated).expect("generated dir") {
        let module = entry.expect("entry").path().join("mod.rs");
        if module.exists() {
            all.push_str(&std::fs::read_to_string(&module).expect("read module"));
        }
    }
    let shared = std::fs::read_to_string(generated.join("shared").join("mod.rs")).expect("shared");
    std::fs::remove_dir_all(&out).expect("clean up");

    let compact: String = all.split_whitespace().collect::<Vec<_>>().join(" ");

    // The JSON-lines feed: its item type is generated, and the body comes back
    // as a stream of it.
    assert!(compact.contains("pub struct CreateImageInfo"), "{all}");
    assert!(
        compact.contains("Result<ApiResponse<super::shared::JsonLinesStream<CreateImageInfo>>, super::shared::ApiError>"),
        "{all}"
    );
    // The status is checked by the shared `send_json_lines`, which hands the
    // body back unread, its lines capped at the client's `BodyLimits`.
    assert!(
        compact.contains("super::shared::send_json_lines(client.into(), builder.build()).await"),
        "{all}"
    );

    // The tar archive comes back as bytes, unread — not parsed as JSON.
    assert!(
        compact.contains("Result<ApiResponse<super::shared::ByteStream>, super::shared::ApiError>"),
        "{all}"
    );
    assert!(
        compact.contains("super::shared::send_bytes(client.into(), builder.build()).await"),
        "{all}"
    );

    // Raw request bodies: required → a body parameter; optional → an Option.
    assert!(
        compact.contains("body: foundation_netio::shared::http::SendSafeBody,"),
        "{all}"
    );
    assert!(
        compact.contains("body: Option<foundation_netio::shared::http::SendSafeBody>,"),
        "{all}"
    );
    assert!(
        compact.contains("super::shared::with_raw_body(builder, \"application/x-tar\", body)"),
        "{all}"
    );

    // Every response goes through the shared senders, which read an error
    // body bounded (`read_error_body`); no generated function collects a body
    // itself.
    assert!(!all.contains("collect_bytes_from_send_safe"), "{all}");
    assert!(!all.contains("response.take_body()"), "{all}");

    // An array query parameter is a Vec, sent as one pair per value.
    assert!(compact.contains("pub names: Vec<String>,"), "{all}");
    assert!(
        compact.contains("for value in &args.names { builder = builder.query(\"names\", Some(value.as_str())); }"),
        "{all}"
    );
    assert!(compact.contains("pub from_image: Option<String>,"), "{all}");

    // An `allOf` that extends a `$ref`'d type carries that type's fields too.
    let create_request = all
        .split("pub struct ContainerCreateRequest")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("ContainerCreateRequest generated");
    for field in [
        "pub image: Option<String>",
        "pub cmd: Option<Vec<String>>",
        "pub host_config:",
    ] {
        assert!(
            create_request.contains(field),
            "{field} missing from {create_request}"
        );
    }

    // A JSON document is still parsed as one, by the shared `send_json`.
    assert!(
        compact.contains("Result<ApiResponse<ImageInspect>, super::shared::ApiError>"),
        "{all}"
    );
    assert!(
        compact.contains("super::shared::send_json(client.into(), builder.build()).await"),
        "{all}"
    );

    // Every function takes its client as `impl Into<ApiClient>`: a bare
    // `DynNetClient` still works, and an `ApiClient` carries its body limits.
    assert!(
        compact.contains("client: impl Into<super::shared::ApiClient>,"),
        "{all}"
    );
    assert!(!compact.contains("client: DynNetClient,"), "{all}");

    for helper in [
        "send_checked",
        "send_json",
        "send_json_lines",
        "send_bytes",
        "send_without_body",
        "with_raw_body",
        "ApiClient",
        "BodyLimits",
        "ByteStream",
        "JsonLinesStream",
    ] {
        assert!(shared.contains(helper), "{helper} not re-exported: {shared}");
    }
}
