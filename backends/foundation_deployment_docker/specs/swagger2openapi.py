#!/usr/bin/env python3
"""Convert the Docker Engine API Swagger 2.0 spec to OpenAPI 3.0 JSON.

WHY: Docker's spec is Swagger 2.0, and the generator reads OpenAPI 3.0. The
shared conversion lives in `scripts/openapi/swagger2openapi.py` so the Docker and
Firecracker crates do not keep two copies. This file holds only the Docker-
specific declaration: the source, the output and the JSON-lines operations.

WHAT: Reads the vendored `docker-engine-v1.53.yaml` and writes
`artefacts/cloud_providers/docker/openapi.json`.

HOW: Calls `convert` from the shared script. `STREAM_ITEMS` names the operations
whose `200` body is a JSON-lines stream of progress objects, and the spec
definition of one line. Upstream documents these responses only as "no error";
dockerd writes one `jsonmessage.JSONMessage` per line, which is what these
definitions describe (`/images/load` streams the same messages as `/build`, so
it shares `BuildInfo`).

Run from the repo root:

    python3 backends/foundation_deployment_docker/specs/swagger2openapi.py
"""

import json
import os
import sys

try:
    import yaml
except ImportError:
    sys.exit("PyYAML required: pip install pyyaml")

# The shared converter sits two directories above this file, under scripts/.
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
sys.path.insert(0, os.path.join(ROOT, "scripts", "openapi"))

from swagger2openapi import convert  # noqa: E402

SRC = "backends/foundation_deployment_docker/specs/docker-engine-v1.53.yaml"
OUT = "artefacts/cloud_providers/docker/openapi.json"

# Operations whose `200` body is a JSON-lines stream of progress objects, and
# the spec definition of one line.
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


def main():
    with open(SRC) as f:
        source = yaml.safe_load(f)
    spec3 = convert(source, STREAM_ITEMS)
    with open(OUT, "w") as f:
        json.dump(spec3, f)
    print(
        f"converted -> {OUT}: {len(spec3['components']['schemas'])} schemas, "
        f"{len(spec3['paths'])} paths"
    )


if __name__ == "__main__":
    main()
