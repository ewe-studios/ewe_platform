#!/usr/bin/env python3
"""Check the manifest contract (spec 78).

    python3 -I scripts/ci/manifest_contract.py --check-schema
    python3 -I scripts/ci/manifest_contract.py --check-copies

`--check-schema` validates the schema document and every fixture. It uses a
small JSON Schema subset validator, so it needs no third-party package and no
network. `--check-copies` compares each consumer copy with the canonical file
by SHA-256. A one-byte change on either side fails the check and names the
drifted file.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC_DIR = ROOT / "specifications" / "completed" / "78-manifest-contract"

# The JSON Schema draft that the contract uses.
DRAFT = "https://json-schema.org/draft/2020-12/schema"
SCHEMA_TYPES = {"object", "array", "string", "integer", "number", "boolean", "null"}

# Each valid fixture must fail at exactly this JSON path. The file name names
# the rule. `fixtures/README.md` gives the same table for a human reader.
EXPECTED_INVALID = {
    "v3-version-2.json": ("/manifest_version", "UnsupportedManifestVersion"),
    "v5-duplicate-name.json": ("/entries/1/name", "Validation"),
    "v6-unknown-kind.json": ("/entries/0/source/kind", "UnknownKind"),
    "v7-http-url.json": ("/entries/0/source/url", "Validation"),
    "v7-bad-sha256.json": ("/entries/0/source/sha256", "Validation"),
}

# The directories where the two readers keep their copies. A copy mirrors the
# spec's `fixtures/` directory, so `valid/complete.json` compares with
# `fixtures/valid/complete.json`. A copy of `manifest.schema.json` at the root
# of the copy directory compares with the schema at the spec root.
CONSUMERS = (
    ("installer", "backends/foundation_installer/tests/fixtures/manifest"),
    ("registrar", "backends/foundation_registrar/tests/fixtures/manifest"),
)

SOURCE_KINDS = {"github-release", "url", "cargo", "apt", "mise", "docker-image"}
DEPLOY_KINDS = {"systemd", "docker", "none"}


@dataclass(frozen=True)
class Fault:
    """One contract fault: the JSON path and a plain description."""

    path: str
    message: str

    def show(self) -> str:
        return f"{self.path or '/'}: {self.message}"


def join(path: str, *parts: object) -> str:
    """Append JSON Pointer parts to a path. The root path is the empty string."""
    out = path
    for part in parts:
        out = f"{out}/{part}"
    return out


def report_error(message: str) -> None:
    """Print one error. GitHub shows an annotation; a local run shows a line."""
    if os.environ.get("GITHUB_ACTIONS"):
        print(f"::error::{message}")
    else:
        print(f"error: {message}", file=sys.stderr)


def json_type(value: object) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    return type(value).__name__


def json_equal(left: object, right: object) -> bool:
    """JSON equality. It keeps `true` apart from `1`, as JSON Schema does."""
    if isinstance(left, bool) or isinstance(right, bool):
        return isinstance(left, bool) and isinstance(right, bool) and left == right
    if isinstance(left, dict) and isinstance(right, dict):
        return left.keys() == right.keys() and all(json_equal(left[key], right[key]) for key in left)
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and all(json_equal(a, b) for a, b in zip(left, right))
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return left == right
    return type(left) is type(right) and left == right


def type_matches(value: object, name: str) -> bool:
    """True when `value` matches one JSON Schema type name."""
    if name == "object":
        return isinstance(value, dict)
    if name == "array":
        return isinstance(value, list)
    if name == "string":
        return isinstance(value, str)
    if name == "boolean":
        return isinstance(value, bool)
    if name == "null":
        return value is None
    if name == "integer":
        if isinstance(value, bool):
            return False
        if isinstance(value, int):
            return True
        # JSON Schema counts 1.0 as an integer.
        return isinstance(value, float) and value.is_integer()
    if name == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    raise ValueError(f"unknown JSON Schema type {name!r}")


def resolve_ref(root: dict, ref: str) -> dict:
    """Resolve a local `#/...` JSON Pointer reference in the schema document."""
    if ref == "#":
        return root
    if not ref.startswith("#/"):
        raise ValueError(f"unsupported reference {ref!r}; only local '#/...' references are allowed")
    node: object = root
    for part in ref[2:].split("/"):
        part = part.replace("~1", "/").replace("~0", "~")
        if not isinstance(node, dict) or part not in node:
            raise KeyError(f"reference {ref!r} does not resolve")
        node = node[part]
    if not isinstance(node, dict):
        raise ValueError(f"reference {ref!r} does not name a schema object")
    return node


def matches(instance: object, schema: dict, root: dict) -> bool:
    """True when the instance matches the schema."""
    return not validate(instance, schema, root, "", [])


def discriminated_branch(instance: object, branches: list, root: dict) -> dict | None:
    """The `oneOf` branch whose `kind` constant equals the instance's kind.

    It gives the precise fault path for a known kind with a bad field, and it
    returns None for an unknown kind.
    """
    if not isinstance(instance, dict) or "kind" not in instance:
        return None
    kind = instance["kind"]
    for branch in branches:
        target = resolve_ref(root, branch["$ref"]) if "$ref" in branch else branch
        if not isinstance(target, dict):
            continue
        kind_schema = target.get("properties", {}).get("kind")
        if isinstance(kind_schema, dict) and kind_schema.get("const") == kind:
            return target
    return None


def validate(instance: object, schema: dict, root: dict, path: str = "", faults: list | None = None) -> list[Fault]:
    """Validate one instance against one schema. Append faults and return them.

    It supports the JSON Schema keywords that the manifest schema uses:
    type, const, enum, required, properties, additionalProperties, items,
    minimum, maximum, minItems, maxItems, uniqueItems, minLength, maxLength,
    pattern, $ref, allOf, anyOf, oneOf and not.
    """
    if faults is None:
        faults = []
    if not isinstance(schema, dict):
        faults.append(Fault(path, "the schema is not an object"))
        return faults
    if "$ref" in schema:
        try:
            target = resolve_ref(root, schema["$ref"])
        except (KeyError, ValueError) as err:
            faults.append(Fault(join(path, "$ref"), str(err)))
            return faults
        validate(instance, target, root, path, faults)
    if "type" in schema:
        expected = schema["type"]
        names = expected if isinstance(expected, list) else [expected]
        try:
            ok = any(type_matches(instance, name) for name in names)
        except ValueError as err:
            faults.append(Fault(join(path, "type"), str(err)))
            return faults
        if not ok:
            faults.append(Fault(path, f"expected type {expected!r}, got {json_type(instance)}"))
            return faults
    if "const" in schema and not json_equal(instance, schema["const"]):
        faults.append(Fault(path, f"expected {schema['const']!r}, got {instance!r}"))
    if "enum" in schema and not any(json_equal(instance, value) for value in schema["enum"]):
        faults.append(Fault(path, f"expected one of {schema['enum']!r}, got {instance!r}"))
    if isinstance(instance, dict):
        for key in schema.get("required", []):
            if key not in instance:
                faults.append(Fault(join(path, key), "required property is missing"))
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        for key, value in instance.items():
            if key in properties:
                validate(value, properties[key], root, join(path, key), faults)
            elif isinstance(additional, dict):
                validate(value, additional, root, join(path, key), faults)
            elif additional is False:
                faults.append(Fault(join(path, key), "additional property is not allowed"))
    if isinstance(instance, list):
        if "minItems" in schema and len(instance) < schema["minItems"]:
            faults.append(Fault(path, f"expected at least {schema['minItems']} items, got {len(instance)}"))
        if "maxItems" in schema and len(instance) > schema["maxItems"]:
            faults.append(Fault(path, f"expected at most {schema['maxItems']} items, got {len(instance)}"))
        if schema.get("uniqueItems"):
            seen: dict[str, int] = {}
            for index, value in enumerate(instance):
                key = json.dumps(value, sort_keys=True, separators=(",", ":"))
                if key in seen:
                    faults.append(
                        Fault(join(path, index), f"duplicate item; the first is at {join(path, seen[key]) or '/'}")
                    )
                else:
                    seen[key] = index
        items = schema.get("items")
        if isinstance(items, dict):
            for index, value in enumerate(instance):
                validate(value, items, root, join(path, index), faults)
    if isinstance(instance, str):
        if "minLength" in schema and len(instance) < schema["minLength"]:
            faults.append(Fault(path, f"expected at least {schema['minLength']} characters"))
        if "maxLength" in schema and len(instance) > schema["maxLength"]:
            faults.append(Fault(path, f"expected at most {schema['maxLength']} characters"))
        pattern = schema.get("pattern")
        if pattern is not None:
            try:
                found = re.search(pattern, instance)
            except re.error as err:
                faults.append(Fault(join(path, "pattern"), f"invalid regular expression: {err}"))
            else:
                if found is None:
                    faults.append(Fault(path, f"does not match the pattern {pattern!r}"))
    if isinstance(instance, (int, float)) and not isinstance(instance, bool):
        if "minimum" in schema and instance < schema["minimum"]:
            faults.append(Fault(path, f"expected at least {schema['minimum']}, got {instance!r}"))
        if "maximum" in schema and instance > schema["maximum"]:
            faults.append(Fault(path, f"expected at most {schema['maximum']}, got {instance!r}"))
    for sub in schema.get("allOf", []):
        validate(instance, sub, root, path, faults)
    if "anyOf" in schema:
        if not any(matches(instance, sub, root) for sub in schema["anyOf"]):
            faults.append(Fault(path, "does not match any alternative"))
    if "oneOf" in schema:
        branches = schema["oneOf"]
        matched = [sub for sub in branches if matches(instance, sub, root)]
        if len(matched) > 1:
            faults.append(Fault(path, "matches more than one variant"))
        elif not matched:
            branch = discriminated_branch(instance, branches, root)
            if branch is None:
                faults.append(Fault(path, "does not match any variant"))
            else:
                # Report the fault of the variant that the instance names.
                validate(instance, branch, root, path, faults)
    if "not" in schema and matches(instance, schema["not"], root):
        faults.append(Fault(path, "must not match the excluded schema"))
    return faults


def walk_schema(node: object, root: dict, path: str, faults: list[Fault]) -> None:
    """Check the structure of one schema object and recurse into its subschemas."""
    if not isinstance(node, dict):
        faults.append(Fault(path, "a schema must be a JSON object"))
        return
    if "$ref" in node:
        ref = node["$ref"]
        if not isinstance(ref, str) or not ref.startswith("#/"):
            faults.append(Fault(join(path, "$ref"), "only local '#/...' references are allowed"))
        else:
            try:
                resolve_ref(root, ref)
            except (KeyError, ValueError) as err:
                faults.append(Fault(join(path, "$ref"), str(err)))
    if "type" in node:
        types = node["type"] if isinstance(node["type"], list) else [node["type"]]
        for name in types:
            if name not in SCHEMA_TYPES:
                faults.append(Fault(join(path, "type"), f"unknown type {name!r}"))
    if "required" in node:
        required = node["required"]
        if not isinstance(required, list) or not all(isinstance(key, str) for key in required):
            faults.append(Fault(join(path, "required"), "required must be an array of strings"))
        elif isinstance(node.get("properties"), dict):
            for key in required:
                if key not in node["properties"]:
                    faults.append(Fault(join(path, "required"), f"{key!r} is required but not in properties"))
    if "pattern" in node:
        try:
            re.compile(node["pattern"])
        except (re.error, TypeError) as err:
            faults.append(Fault(join(path, "pattern"), f"invalid regular expression: {err}"))
    for key in ("minimum", "maximum"):
        if key in node and (not isinstance(node[key], (int, float)) or isinstance(node[key], bool)):
            faults.append(Fault(join(path, key), "must be a number"))
    properties = node.get("properties")
    if isinstance(properties, dict):
        for key, sub in properties.items():
            walk_schema(sub, root, join(path, "properties", key), faults)
    if isinstance(node.get("items"), dict):
        walk_schema(node["items"], root, join(path, "items"), faults)
    if isinstance(node.get("additionalProperties"), dict):
        walk_schema(node["additionalProperties"], root, join(path, "additionalProperties"), faults)
    for key in ("oneOf", "anyOf", "allOf"):
        subs = node.get(key)
        if isinstance(subs, list):
            for index, sub in enumerate(subs):
                walk_schema(sub, root, join(path, key, index), faults)
    if isinstance(node.get("not"), dict):
        walk_schema(node["not"], root, join(path, "not"), faults)
    defs = node.get("$defs")
    if isinstance(defs, dict):
        for key, sub in defs.items():
            walk_schema(sub, root, join(path, "$defs", key), faults)


def schema_faults(schema: object) -> list[Fault]:
    """Check that the schema document is a draft 2020-12 object schema of this contract."""
    if not isinstance(schema, dict):
        return [Fault("", "the schema is not a JSON object")]
    faults: list[Fault] = []
    if schema.get("$schema") != DRAFT:
        faults.append(Fault("/$schema", f"expected {DRAFT!r}"))
    if schema.get("type") != "object":
        faults.append(Fault("/type", "the top level must be an object"))
    required = schema.get("required")
    if not isinstance(required, list):
        faults.append(Fault("/required", "the top level must list the required fields"))
    else:
        for key in ("manifest_version", "entries"):
            if key not in required:
                faults.append(Fault("/required", f"the top level must require {key!r}"))
    if not isinstance(schema.get("$defs"), dict) or not schema["$defs"]:
        faults.append(Fault("/$defs", "the shared subschemas must live in $defs"))
    walk_schema(schema, schema, "", faults)
    return faults


def semantic_faults(doc: object) -> list[Fault]:
    """The contract rules that JSON Schema cannot express: V5 and V6.

    The readers enforce unique names (V5) and known kinds (V6). A schema
    `oneOf` rejects an unknown kind too, but it cannot name the kind path
    precisely; this pass gives the reader's fault path and code.
    """
    faults: list[Fault] = []
    if not isinstance(doc, dict):
        return faults
    entries = doc.get("entries")
    if not isinstance(entries, list):
        return faults
    seen: dict[str, int] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            continue
        name = entry.get("name")
        if isinstance(name, str):
            if name in seen:
                faults.append(
                    Fault(
                        f"/entries/{index}/name",
                        f"duplicate name {name!r}; the first is at /entries/{seen[name]}/name",
                    )
                )
            else:
                seen[name] = index
        source = entry.get("source")
        if isinstance(source, dict) and "kind" in source and source["kind"] not in SOURCE_KINDS:
            faults.append(Fault(f"/entries/{index}/source/kind", f"unknown source kind {source['kind']!r}"))
        deploy = entry.get("deploy")
        if isinstance(deploy, dict) and "kind" in deploy and deploy["kind"] not in DEPLOY_KINDS:
            faults.append(Fault(f"/entries/{index}/deploy/kind", f"unknown deploy kind {deploy['kind']!r}"))
    return faults


def manifest_faults(doc: object, schema: dict) -> list[Fault]:
    """Every fault of one manifest: the schema faults, then the semantic faults."""
    return validate(doc, schema, schema) + semantic_faults(doc)


def load_json(path: Path) -> object:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def check_schema(spec_dir: Path = SPEC_DIR, out=print) -> int:
    """Validate the schema document and every fixture. Return 0 when all are correct."""
    path = spec_dir / "manifest.schema.json"
    problems: list[str] = []
    try:
        schema = load_json(path)
    except (OSError, json.JSONDecodeError) as err:
        report_error(f"{path}: cannot read the schema: {err}")
        return 1
    document_faults = schema_faults(schema)
    if document_faults:
        problems.extend(f"manifest.schema.json: {fault.show()}" for fault in document_faults)
    else:
        out("manifest.schema.json: ok")
    valid_dir = spec_dir / "fixtures" / "valid"
    for fixture in sorted(valid_dir.glob("*.json")):
        relative = fixture.relative_to(spec_dir)
        try:
            doc = load_json(fixture)
        except (OSError, json.JSONDecodeError) as err:
            problems.append(f"{relative}: cannot read JSON: {err}")
            continue
        faults = manifest_faults(doc, schema)
        if faults:
            problems.extend(f"{relative}: {fault.show()}" for fault in faults)
        else:
            out(f"{relative}: ok")
    invalid_dir = spec_dir / "fixtures" / "invalid"
    for fixture in sorted(invalid_dir.glob("*.json")):
        relative = fixture.relative_to(spec_dir)
        try:
            doc = load_json(fixture)
        except (OSError, json.JSONDecodeError) as err:
            problems.append(f"{relative}: cannot read JSON: {err}")
            continue
        faults = manifest_faults(doc, schema)
        expected = EXPECTED_INVALID.get(fixture.name)
        if expected is None:
            if faults:
                problems.append(
                    f"{relative}: rejected at {faults[0].path or '/'}, "
                    "but EXPECTED_INVALID has no path for it"
                )
            else:
                problems.append(f"{relative}: accepted, but it is in the invalid set")
            continue
        expected_path, code = expected
        matching = [fault for fault in faults if fault.path == expected_path]
        if matching:
            out(f"{relative}: rejected at {expected_path} ({code})")
        elif not faults:
            problems.append(f"{relative}: accepted, but the contract rejects it ({code} at {expected_path})")
        else:
            observed = ", ".join(sorted({fault.path or "/" for fault in faults}))
            problems.append(
                f"{relative}: rejected at {observed}, but the expected path is {expected_path} ({code})"
            )
    for problem in problems:
        report_error(problem)
    if problems:
        out(f"{len(problems)} manifest contract problem(s)")
        return 1
    out("all manifests match the contract")
    return 0


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_faults(root: Path = ROOT, spec_dir: Path = SPEC_DIR) -> list[str]:
    """Compare each consumer copy with its canonical file. Return one line per fault."""
    faults: list[str] = []
    for label, relative in CONSUMERS:
        consumer_dir = root / relative
        if not consumer_dir.is_dir():
            continue
        for copy in sorted(path for path in consumer_dir.rglob("*") if path.is_file()):
            relative = copy.relative_to(consumer_dir)
            candidates = (spec_dir / relative, spec_dir / "fixtures" / relative)
            canonical = next((path for path in candidates if path.is_file()), None)
            if canonical is None:
                looked_for = " and ".join(str(path.relative_to(root)) for path in candidates)
                faults.append(
                    f"{label}: {copy.relative_to(root)} has no canonical file (looked for {looked_for})"
                )
                continue
            copy_hash = sha256_file(copy)
            canonical_hash = sha256_file(canonical)
            if copy_hash != canonical_hash:
                faults.append(
                    f"{label}: {copy.relative_to(root)} drifted from {canonical.relative_to(root)} "
                    f"(sha256 {copy_hash} != {canonical_hash})"
                )
    return faults


def check_copies(root: Path = ROOT, spec_dir: Path = SPEC_DIR, out=print) -> int:
    """Check that each consumer copy matches the canonical file by SHA-256."""
    for label, relative in CONSUMERS:
        if not (root / relative).is_dir():
            out(f"{label}: no copies yet ({relative})")
    faults = copy_faults(root, spec_dir)
    for fault in faults:
        report_error(fault)
    if faults:
        return 1
    out("every consumer copy matches the canonical file")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--check-schema", action="store_true", help="validate the schema and every fixture")
    parser.add_argument(
        "--check-copies", action="store_true", help="compare each consumer copy with the canonical file"
    )
    args = parser.parse_args(argv)
    if not (args.check_schema or args.check_copies):
        parser.error("give --check-schema, --check-copies, or both")
    failed = False
    if args.check_schema:
        failed |= check_schema() != 0
    if args.check_copies:
        failed |= check_copies() != 0
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
