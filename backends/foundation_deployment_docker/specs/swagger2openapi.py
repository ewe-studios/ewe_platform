#!/usr/bin/env python3
"""Convert the Docker Engine API Swagger 2.0 spec to the OpenAPI 3.0 subset that
`foundation_openapi`'s `UnifiedGenerator` consumes.

WHY: The generator's parser (`foundation_openapi::process_spec`) understands
OpenAPI 3.x (`components.schemas`, `$ref: #/components/schemas/...`, `requestBody`,
`responses[*].content["application/json"].schema`) and GCP Discovery — but not
Swagger 2.0 (`definitions`, `$ref: #/definitions/...`, `in: body` params,
`responses[*].schema`). Docker's spec is Swagger 2.0.

WHAT: Reads the vendored `docker-engine-v1.53.yaml` and writes
`artefacts/cloud_providers/docker/openapi.json` in the OpenAPI 3.0 shape.

HOW: Rewrites `$ref` paths, moves `in: body` params into `requestBody`, moves a
parameter's `type`/`items`/`format`/`enum`/`default` into its `schema` (so an
array query parameter is seen as one, and sent repeated), moves
response `schema` under `content`, and derives `servers` from `schemes` +
`basePath`.

Media types come from the operation's `produces` / `consumes` (falling back to
the spec-wide lists), not a blanket `application/json`: Swagger 2 says what a
body *is* there, and the generator needs it to tell a JSON document from a tar
archive, a multiplexed log stream or a JSON-lines progress feed. A `2xx` with
no schema whose operation produces a binary stream (`/containers/{id}/export`,
`/exec/{id}/start`, ...) still has a body, so it is given a
`{type: string, format: binary}` schema under that media type.

One addition of ours, `STREAM_ITEMS`: the spec defines the objects that
`/images/create`, `/images/{name}/push`, `/build` and `/images/load` stream
(`CreateImageInfo`, `PushImageInfo`, `BuildInfo`) but no response references
them, so a generator never sees them; `/containers/{id}/stats` references
`ContainerStatsResponse` but declares it a single JSON document, though by
default dockerd streams it. Each of those operations' `200` is declared here
as `application/jsonl` of its item type -- what dockerd sends.

Run from the repo root:
    python3 backends/foundation_deployment_docker/specs/swagger2openapi.py
"""

import json
import sys

try:
    import yaml
except ImportError:
    sys.exit("PyYAML required: pip install pyyaml")

SRC = "backends/foundation_deployment_docker/specs/docker-engine-v1.53.yaml"
OUT = "artefacts/cloud_providers/docker/openapi.json"


# Operations whose `200` body is a JSON-lines stream of progress objects, and
# the spec definition of one line. Upstream documents these responses only as
# "no error"; dockerd writes one `jsonmessage.JSONMessage` per line, which is
# what these definitions describe (`/images/load` streams the same messages as
# `/build`, so it shares `BuildInfo`).
STREAM_ITEMS = {
    "ImageCreate": "CreateImageInfo",
    "ImagePush": "PushImageInfo",
    "ImageBuild": "BuildInfo",
    "ImageLoad": "BuildInfo",
    # With `stream=true` (dockerd's default) stats is an endless feed of one
    # `ContainerStatsResponse` per line; with `stream=false` it is a single
    # line. Upstream declares it `application/json`, which a generator reads
    # as one document and would wait on forever for a live feed.
    "ContainerStats": "ContainerStatsResponse",
}

# The media type a JSON-lines stream is declared under.
JSON_LINES = "application/jsonl"

# Media types whose body is not a single document but a byte stream.
JSON_TYPES = ("application/json",)
TEXT_TYPES = ("text/plain",)


def media_type(declared, fallback):
    """The first media type an operation declares, else the spec-wide one."""
    types = declared or fallback or []
    return types[0] if types else "application/json"


# Swagger 2 keeps a non-body parameter's type on the parameter itself; OpenAPI 3
# puts it under `schema`.
PARAM_SCHEMA_KEYS = ("type", "items", "format", "enum", "default", "collectionFormat")


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


def main():
    with open(SRC) as f:
        s = yaml.safe_load(f)

    definitions = rewrite_refs(s.get("definitions", {}))
    global_produces = s.get("produces")
    global_consumes = s.get("consumes")
    base_url = f"http://localhost{s.get('basePath', '')}"

    new_paths = {}
    for path, item in s.get("paths", {}).items():
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
            stream_item = STREAM_ITEMS.get(op.get("operationId"))
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
                    # Error bodies are Docker's JSON `ErrorResponse` whatever the
                    # operation produces on success.
                    resp["content"] = {(produces if success else "application/json"): {"schema": sch}}
                elif success and produces not in JSON_TYPES + TEXT_TYPES:
                    resp["content"] = {produces: {"schema": {"type": "string", "format": "binary"}}}
                new_resp[str(code)] = resp
            new_op["responses"] = new_resp
            new_item[method] = new_op
        new_paths[path] = new_item

    spec3 = {
        "openapi": "3.0.0",
        "info": s.get("info", {"title": "Docker Engine API", "version": "1.53"}),
        "servers": [{"url": base_url}],
        "paths": new_paths,
        "components": {"schemas": definitions},
    }

    with open(OUT, "w") as f:
        json.dump(spec3, f)
    print(f"converted -> {OUT}: {len(definitions)} schemas, {len(new_paths)} paths, base_url={base_url}")


if __name__ == "__main__":
    main()
