#!/usr/bin/env python3
"""Convert a Swagger 2.0 document to the OpenAPI 3.0 subset that
`foundation_openapi`'s `UnifiedGenerator` consumes.

WHY: The generator's parser (`foundation_openapi::process_spec`) understands
OpenAPI 3.x (`components.schemas`, `$ref: #/components/schemas/...`,
`requestBody`, `responses[*].content["application/json"].schema`) and GCP
Discovery - but not Swagger 2.0 (`definitions`, `$ref: #/definitions/...`,
`in: body` params, `responses[*].schema`). Two vendor specifications in this
repository are Swagger 2.0: the Docker Engine API and the Firecracker API.

WHAT: The shared converter. `convert` reads a loaded Swagger 2.0 document and
returns the OpenAPI 3.0 shape. A crate script supplies its own source and output
paths and, when its vendor streams JSON lines, the map from operation ID to line
schema (see `stream_items`).

HOW: Rewrites `$ref` paths, moves `in: body` params into `requestBody`, moves a
parameter's `type`/`items`/`format`/`enum`/`default` into its `schema` (so an
array query parameter is seen as one, and sent repeated), moves response
`schema` under `content`, and derives `servers` from `schemes` + `basePath`.

Media types come from the operation's `produces` / `consumes` (falling back to
the spec-wide lists), not a blanket `application/json`: Swagger 2 says what a
body *is* there, and the generator needs it to tell a JSON document from a tar
archive, a multiplexed log stream or a JSON-lines progress feed. A `2xx` with
no schema whose operation produces a binary stream still has a body, so it is
given a `{type: string, format: binary}` schema under that media type.

Run it directly, or import `convert` from a crate script:

    python3 scripts/openapi/swagger2openapi.py --src spec.yaml --out openapi.json

An operation that `stream_items` names has its `200` declared as
`application/jsonl` of that item type - what the server sends line by line.
"""

import argparse
import json
import sys

try:
    import yaml
except ImportError:
    sys.exit("PyYAML required: pip install pyyaml")

# The media type a JSON-lines stream is declared under.
JSON_LINES = "application/jsonl"

# Media types whose body is a single document, not a byte stream.
JSON_TYPES = ("application/json",)
TEXT_TYPES = ("text/plain",)

# Swagger 2 keeps a non-body parameter's type on the parameter itself; OpenAPI 3
# puts it under `schema`.
PARAM_SCHEMA_KEYS = ("type", "items", "format", "enum", "default", "collectionFormat")


def media_type(declared, fallback):
    """The first media type an operation declares, else the spec-wide one."""
    types = declared or fallback or []
    return types[0] if types else "application/json"


def param_to_v3(pm):
    """Move a Swagger 2 parameter's type keywords under `schema`."""
    out = {k: v for k, v in pm.items() if k not in PARAM_SCHEMA_KEYS}
    schema = {k: v for k, v in pm.items() if k in PARAM_SCHEMA_KEYS and k != "collectionFormat"}
    if schema:
        out["schema"] = schema
    return out


def rewrite_refs(obj):
    """Recursively rewrite `#/definitions/X` -> `#/components/schemas/X`."""
    if isinstance(obj, dict):
        out = {}
        for k, v in obj.items():
            if k == "$ref" and isinstance(v, str) and v.startswith("#/definitions/"):
                out[k] = v.replace("#/definitions/", "#/components/schemas/")
            else:
                out[k] = rewrite_refs(v)
        return out
    if isinstance(obj, list):
        return [rewrite_refs(v) for v in obj]
    return obj


def convert(source, stream_items=None):
    """Return the OpenAPI 3.0 form of a loaded Swagger 2.0 document.

    `stream_items` maps an operation ID to the schema name of one line of its
    JSON-lines success body. An operation in the map has its `200` declared as
    `application/jsonl` of that schema.
    """
    stream_items = stream_items or {}
    definitions = rewrite_refs(source.get("definitions", {}))
    global_produces = source.get("produces")
    global_consumes = source.get("consumes")
    base_url = f"http://localhost{source.get('basePath', '')}"

    new_paths = {}
    for path, item in source.get("paths", {}).items():
        new_item = {}
        for method, op in item.items():
            if method not in ("get", "post", "put", "patch", "delete", "options", "head"):
                new_item[method] = rewrite_refs(op)
                continue
            op = rewrite_refs(op)
            new_op = {k: v for k, v in op.items() if k not in ("parameters", "responses")}
            produces = media_type(op.get("produces"), global_produces)
            consumes = media_type(op.get("consumes"), global_consumes)
            kept, body_schema, body_required = [], None, False
            for pm in op.get("parameters", []) or []:
                if pm.get("in") == "body":
                    body_schema = pm.get("schema")
                    # Swagger 2: a parameter is optional unless it says otherwise.
                    body_required = bool(pm.get("required", False))
                elif pm.get("in") == "formData":
                    body_schema = body_schema or {"type": "object"}
                    body_required = body_required or bool(pm.get("required", False))
                else:
                    kept.append(param_to_v3(pm))
            if kept:
                new_op["parameters"] = kept
            if body_schema is not None:
                new_op["requestBody"] = {
                    "content": {consumes: {"schema": body_schema}},
                    "required": body_required,
                }
            stream_item = stream_items.get(op.get("operationId"))
            new_resp = {}
            for code, resp in (op.get("responses", {}) or {}).items():
                resp = dict(resp)
                sch = resp.pop("schema", None)
                success = str(code).startswith("2")
                if success and stream_item is not None:
                    resp["content"] = {
                        JSON_LINES: {"schema": {"$ref": f"#/components/schemas/{stream_item}"}}
                    }
                elif sch is not None:
                    # Error bodies are the vendor's JSON error whatever the
                    # operation produces on success.
                    resp["content"] = {
                        (produces if success else "application/json"): {"schema": sch}
                    }
                elif success and produces not in JSON_TYPES + TEXT_TYPES:
                    resp["content"] = {produces: {"schema": {"type": "string", "format": "binary"}}}
                new_resp[str(code)] = resp
            new_op["responses"] = new_resp
            new_item[method] = new_op
        new_paths[path] = new_item

    return {
        "openapi": "3.0.0",
        "info": source.get("info", {"title": "API", "version": ""}),
        "servers": [{"url": base_url}],
        "paths": new_paths,
        "components": {"schemas": definitions},
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--src", required=True, help="Swagger 2.0 YAML file")
    parser.add_argument("--out", required=True, help="OpenAPI 3.0 JSON file")
    args = parser.parse_args(argv)

    with open(args.src) as f:
        source = yaml.safe_load(f)
    spec3 = convert(source)
    with open(args.out, "w") as f:
        json.dump(spec3, f)
    print(
        f"converted -> {args.out}: {len(spec3['components']['schemas'])} schemas, "
        f"{len(spec3['paths'])} paths"
    )


if __name__ == "__main__":
    main()
