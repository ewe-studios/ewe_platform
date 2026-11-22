# Manifest fixtures

These files are the canonical fixtures of the manifest contract (spec 78). The installer
(spec 79) and the registrar sidecar (spec 82) copy the files they use into their own
`tests/fixtures/manifest/` directory. `scripts/ci/manifest_contract.py --check-copies`
compares each copy with the file here by SHA-256.

## Valid fixtures

| File | Shows |
|---|---|
| `valid/complete.json` | The complete example of section 5.1. It has one entry for each deploy kind. |
| `valid/empty.json` | `entries` is an empty array. Both readers accept it and do nothing. |
| `valid/unknown-field.json` | Unknown fields at the top level and on one entry. Both readers ignore them. |
| `valid/marks.json` | One `agent-rpc` entry. The installer writes `EWE_ENTRY=agent-rpc` and `EWE_VERSION=0.4.0`; the registrar maps the marks to this entry. |
| `valid/deploy-systemd.json` | One entry with a `systemd` deploy target. |
| `valid/deploy-docker.json` | One entry with a `docker` deploy target, two ports, a volume and a command. |
| `valid/deploy-none.json` | One entry with the `none` deploy target. |

## Invalid fixtures

Each file breaks one rule. The file name names the rule. The check reports the JSON path.

| File | Rule | JSON path | Reader error |
|---|---|---|---|
| `invalid/v3-version-2.json` | V3: `manifest_version` is not 1 | `/manifest_version` | `UnsupportedManifestVersion` |
| `invalid/v5-duplicate-name.json` | V5: two entries have the same name | `/entries/1/name` | `Validation` |
| `invalid/v6-unknown-kind.json` | V6: `source.kind` is not known | `/entries/0/source/kind` | `UnknownKind` |
| `invalid/v7-http-url.json` | V7: a `url` source is not HTTPS | `/entries/0/source/url` | `Validation` |
| `invalid/v7-bad-sha256.json` | V7: `sha256` is not 64 lowercase hex characters | `/entries/0/source/sha256` | `Validation` |

A standard JSON Schema validator accepts `invalid/v5-duplicate-name.json`. Draft 2020-12 has no
keyword for uniqueness by one property. V5 is a semantic rule: the readers and
`scripts/ci/manifest_contract.py --check-schema` enforce it.

## Marks

A mark links an installed service to its entry. The installer writes the mark; the registrar
reads it.

| Deploy kind | Mark | Value for `valid/marks.json` |
|---|---|---|
| systemd | `Environment=EWE_ENTRY=<name>` | `Environment=EWE_ENTRY=agent-rpc` |
| systemd | `Environment=EWE_VERSION=<version>` | `Environment=EWE_VERSION=0.4.0` |
| docker | label `dev.ewe.entry=<name>` | `dev.ewe.entry=agent-rpc` |
| docker | label `dev.ewe.version=<version>` | `dev.ewe.version=0.4.0` |

A mark that does not match an entry is ignored, with a warning. The manifest stays the source
of truth.
