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

HOW: Rewrites `$ref` paths, moves `in: body` params into `requestBody`, moves
response `schema` under `content["application/json"]`, and derives `servers` from
`schemes` + `basePath`.

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
            kept, body_schema = [], None
            for pm in op.get("parameters", []) or []:
                if pm.get("in") == "body":
                    body_schema = pm.get("schema")
                elif pm.get("in") == "formData":
                    body_schema = body_schema or {"type": "object"}
                else:
                    kept.append(pm)
            if kept:
                new_op["parameters"] = kept
            if body_schema is not None:
                new_op["requestBody"] = {
                    "content": {"application/json": {"schema": body_schema}},
                    "required": True,
                }
            new_resp = {}
            for code, resp in (op.get("responses", {}) or {}).items():
                resp = dict(resp)
                sch = resp.pop("schema", None)
                if sch is not None:
                    resp["content"] = {"application/json": {"schema": sch}}
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
