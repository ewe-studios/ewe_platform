//! Tests for the `emit` shrink passes: envelope hoisting, shared-shape hoisting,
//! and per-module deduplication.

use std::collections::{BTreeSet, HashSet};

use foundation_openapi::unified::emit::{
    dedupe_within_modules, hoist_envelopes, hoist_shared_shapes, Field, ModuleFile, TypeBody,
    TypeDef, ARGS_DERIVES, SCHEMA_DERIVES,
};

fn field(name: &str, ty: &str) -> Field {
    Field {
        doc: format!("`{name}` property."),
        attrs: Vec::new(),
        name: name.to_string(),
        ty: ty.to_string(),
    }
}

fn schema(name: &str, fields: Vec<Field>) -> TypeDef {
    TypeDef::structure(name, &format!("`{name}` type."), SCHEMA_DERIVES, fields)
}

fn module(path: &str, defs: Vec<TypeDef>) -> ModuleFile {
    let mut m = ModuleFile::new(path);
    for d in defs {
        m.push(d);
    }
    m
}

fn find<'a>(files: &'a [ModuleFile], module: usize, name: &str) -> &'a TypeDef {
    files[module]
        .types()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("{name} not in module {module}"))
}

fn alias_target(def: &TypeDef) -> &str {
    match &def.body {
        TypeBody::Alias { target } => target,
        TypeBody::Struct { .. } => panic!("{} is still a struct", def.name),
    }
}

fn duplicate_names(file: &ModuleFile) -> BTreeSet<String> {
    let mut seen = HashSet::new();
    file.types()
        .filter(|t| !seen.insert(t.name.clone()))
        .map(|t| t.name.clone())
        .collect()
}

#[test]
fn envelopes_that_differ_only_in_result_share_one_generic() {
    let envelope = |name: &str, result: &str| {
        schema(
            name,
            vec![
                field("errors", "Vec<serde_json::Value>"),
                field("messages", "Vec<serde_json::Value>"),
                field("result", result),
                field("success", "bool"),
            ],
        )
    };
    let mut files = vec![
        module("shared/mod.rs", vec![]),
        module(
            "zones/mod.rs",
            vec![
                envelope("ZoneResponse", "Option<Zone>"),
                envelope("ZoneList", "Vec<Zone>"),
            ],
        ),
    ];
    hoist_envelopes(&mut files, 0);

    assert_eq!(
        alias_target(find(&files, 1, "ZoneResponse")),
        "super::shared::ApiEnvelope<Option<Zone>>"
    );
    assert_eq!(
        alias_target(find(&files, 1, "ZoneList")),
        "super::shared::ApiEnvelope<Vec<Zone>>"
    );
    let TypeBody::Struct {
        generic, fields, ..
    } = &find(&files, 0, "ApiEnvelope").body
    else {
        panic!("ApiEnvelope is not a struct");
    };
    assert_eq!(generic.as_deref(), Some("T"));
    assert!(fields.iter().any(|f| f.name == "result" && f.ty == "T"));
}

#[test]
fn a_lone_envelope_shape_is_left_alone() {
    let mut files = vec![
        module("shared/mod.rs", vec![]),
        module(
            "zones/mod.rs",
            vec![schema(
                "Only",
                vec![field("result", "Zone"), field("success", "bool")],
            )],
        ),
    ];
    hoist_envelopes(&mut files, 0);
    assert!(matches!(
        find(&files, 1, "Only").body,
        TypeBody::Struct { .. }
    ));
}

#[test]
fn identical_portable_structs_across_modules_are_defined_once_in_shared() {
    let args = |name: &str| {
        TypeDef::structure(
            name,
            "args",
            ARGS_DERIVES,
            vec![field("account_id", "String")],
        )
    };
    let mut files = vec![
        module("shared/mod.rs", vec![]),
        module("dns/mod.rs", vec![args("ListDnsArgs")]),
        module("zones/mod.rs", vec![args("ListZonesArgs")]),
    ];
    hoist_shared_shapes(&mut files, 0);

    assert_eq!(
        alias_target(find(&files, 1, "ListDnsArgs")),
        "super::shared::AccountIdArgs"
    );
    assert_eq!(
        alias_target(find(&files, 2, "ListZonesArgs")),
        "super::shared::AccountIdArgs"
    );
    assert!(matches!(
        find(&files, 0, "AccountIdArgs").body,
        TypeBody::Struct { .. }
    ));
    assert!(duplicate_names(&files[0]).is_empty());
}

#[test]
fn empty_objects_collapse_into_one_empty_object() {
    let mut files = vec![
        module("shared/mod.rs", vec![schema("SharedEmpty", vec![])]),
        module("dns/mod.rs", vec![schema("DnsEmpty", vec![])]),
    ];
    hoist_shared_shapes(&mut files, 0);
    assert_eq!(alias_target(find(&files, 0, "SharedEmpty")), "EmptyObject");
    assert_eq!(
        alias_target(find(&files, 1, "DnsEmpty")),
        "super::shared::EmptyObject"
    );
}

#[test]
fn structs_naming_module_types_are_only_merged_inside_their_module() {
    let with_zone = |name: &str| schema(name, vec![field("zone", "Option<Zone>")]);
    let mut files = vec![
        module("shared/mod.rs", vec![]),
        module("a/mod.rs", vec![with_zone("A1"), with_zone("A2")]),
        module("b/mod.rs", vec![with_zone("B1")]),
    ];
    hoist_shared_shapes(&mut files, 0);
    dedupe_within_modules(&mut files);

    assert!(matches!(
        find(&files, 1, "A1").body,
        TypeBody::Struct { .. }
    ));
    assert_eq!(alias_target(find(&files, 1, "A2")), "A1");
    assert!(matches!(
        find(&files, 2, "B1").body,
        TypeBody::Struct { .. }
    ));
}

#[test]
fn shapes_that_differ_only_in_default_merge_and_keep_default() {
    let mut needs = schema("Needs", vec![field("name", "String")]);
    let mut plain = schema("Plain", vec![field("name", "String")]);
    if let TypeBody::Struct { derives, .. } = &mut plain.body {
        derives.retain(|d| d != "Default");
    }
    // Same module, so the local pass merges them; `Plain` comes first.
    std::mem::swap(&mut needs, &mut plain);
    let mut files = vec![module("zones/mod.rs", vec![needs, plain])];
    dedupe_within_modules(&mut files);

    let canonical = find(&files, 0, "Plain");
    let TypeBody::Struct { derives, .. } = &canonical.body else {
        panic!("Plain should stay the struct");
    };
    assert!(
        derives.iter().any(|d| d == "Default"),
        "Needs relied on Default"
    );
    assert_eq!(alias_target(find(&files, 0, "Needs")), "Plain");
}

#[test]
fn aliases_render_as_pub_type() {
    let mut out = String::new();
    TypeDef::alias("ZoneId", "`ZoneId` type.", "String")
        .render(&mut out)
        .unwrap();
    assert_eq!(out, "/// `ZoneId` type.\npub type ZoneId = String;\n\n");
}
