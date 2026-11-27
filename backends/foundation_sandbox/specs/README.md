# Vendored API specifications

These files are copies of vendor specifications. The crate never edits them by
hand. The generator reads the OpenAPI 3 form of each file (feature 04).

## Firecracker

| Item | Value |
|---|---|
| File | `firecracker-v1.19.yaml` |
| Source | `https://github.com/firecracker-microvm/firecracker/blob/778a26c0594c80025ed798088a66c2511ba618de/src/firecracker/swagger/firecracker.yaml` |
| Commit | `778a26c0594c80025ed798088a66c2511ba618de` (2026-11-10) |
| API version | `1.19.0-dev` |
| Format | Swagger 2.0 |
| Vendored | 2026-11-26 |
| SHA-256 | `49d5bb9a0030bbd3c604b0e2b5dbf294d34afda095bc866c6dbae7648a1e705d` |

The OpenAPI 3 form is `artefacts/cloud_providers/firecracker/openapi.json`. Make
it from the repository root with the shared converter:

```bash
python3 scripts/openapi/swagger2openapi.py \
  --src backends/foundation_sandbox/specs/firecracker-v1.19.yaml \
  --out artefacts/cloud_providers/firecracker/openapi.json
```

To move to a newer Firecracker release, replace the YAML file, update this table,
convert again and regenerate the code. The build script fails when the generated
tree no longer matches the specification (feature 04, invariant I5).
