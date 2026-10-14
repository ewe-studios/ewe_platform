//! What the generator emits, as data — and the passes that shrink it.
//!
//! WHY: a provider's generated API is compiled as one crate, and rustc's
//! front-end cost grows with every type it has to check — above all with the
//! `Serialize`/`Deserialize` impls each struct derives. Cloudflare's spec
//! produced ~10k structs, half of them structural duplicates (thousands of
//! `{ account_id: String }` args, `{ result, success }` envelopes, empty
//! objects), and one `rustc` for the crate needed ~12 GB.
//!
//! WHAT: the generator describes every type as a [`TypeDef`] instead of writing
//! text straight away. Before rendering, [`shrink`] rewrites them:
//!
//! 1. Envelopes — structs that differ only in the type of their `result`
//!    field become aliases of one generic `ApiEnvelope…<T>` in `shared`.
//! 2. Shared shapes — identical structs whose fields name no other generated
//!    type are hoisted into `shared` once, wherever they occurred.
//! 3. Local duplicates — identical structs within one module become aliases
//!    of the first.
//!
//! Every original name survives as a `pub type` alias, so the public API —
//! names, field access, struct literals — is unchanged.
//!
//! Derives are not trimmed: `Default` is what callers and test fixtures build
//! request args and response values with (`..Default::default()`), and the
//! other derives are each used. Merging duplicates is where the size was.
//!
//! HOW: the passes compare rendered field lists (attributes, names, types)
//! with doc comments left out. Text comparison is exact for the generator's
//! own output, and it is the same text that is finally written.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

/// Derives for a schema type (request bodies and responses).
pub(crate) const SCHEMA_DERIVES: &[&str] = &[
    "Debug",
    "Clone",
    "Default",
    "Serialize",
    "Deserialize",
    "JsonHash",
];

/// Derives for an endpoint's `*Args` struct. Never deserialized; `Default`
/// lets callers fill only the parameters they need (`..Default::default()`).
pub(crate) const ARGS_DERIVES: &[&str] = &["Debug", "Clone", "Default", "Serialize", "JsonHash"];

/// One field of a generated struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    /// Doc comment text (without `///`), or empty.
    pub doc: String,
    /// Attribute lines, e.g. `#[serde(rename = "Id")]`.
    pub attrs: Vec<String>,
    /// The field identifier.
    pub name: String,
    /// The field's Rust type.
    pub ty: String,
}

/// The body of a generated type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeBody {
    /// `pub struct Name<generic> { fields }`.
    Struct {
        derives: Vec<String>,
        attrs: Vec<String>,
        /// A type parameter (`T`), for the generic envelopes.
        generic: Option<String>,
        fields: Vec<Field>,
    },
    /// `pub type Name = target;`.
    Alias { target: String },
}

/// One generated type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeDef {
    pub name: String,
    /// Doc comment text (without `///`), one entry per line.
    pub doc: Vec<String>,
    pub body: TypeBody,
}

impl TypeDef {
    /// A struct with the given derives and fields.
    pub(crate) fn structure(name: &str, doc: &str, derives: &[&str], fields: Vec<Field>) -> Self {
        Self {
            name: name.to_string(),
            doc: vec![doc.to_string()],
            body: TypeBody::Struct {
                derives: derives.iter().map(|d| (*d).to_string()).collect(),
                attrs: Vec::new(),
                generic: None,
                fields,
            },
        }
    }

    /// `pub type name = target;`.
    pub(crate) fn alias(name: &str, doc: &str, target: &str) -> Self {
        Self {
            name: name.to_string(),
            doc: vec![doc.to_string()],
            body: TypeBody::Alias {
                target: target.to_string(),
            },
        }
    }

    fn render(&self, out: &mut String) -> std::fmt::Result {
        for line in &self.doc {
            if line.is_empty() {
                writeln!(out, "///")?;
            } else {
                writeln!(out, "/// {line}")?;
            }
        }
        match &self.body {
            TypeBody::Alias { target } => writeln!(out, "pub type {} = {};", self.name, target)?,
            TypeBody::Struct {
                derives,
                attrs,
                generic,
                fields,
            } => {
                writeln!(out, "#[derive({})]", derives.join(", "))?;
                for attr in attrs {
                    writeln!(out, "{attr}")?;
                }
                match generic {
                    Some(param) => writeln!(out, "pub struct {}<{param}> {{", self.name)?,
                    None => writeln!(out, "pub struct {} {{", self.name)?,
                }
                for field in fields {
                    if !field.doc.is_empty() {
                        writeln!(out, "    /// {}", field.doc)?;
                    }
                    for attr in &field.attrs {
                        writeln!(out, "    {attr}")?;
                    }
                    writeln!(out, "    pub {}: {},", field.name, field.ty)?;
                }
                writeln!(out, "}}")?;
            }
        }
        writeln!(out)
    }
}

/// A piece of a generated module: verbatim text, or a type.
#[derive(Debug, Clone)]
pub(crate) enum Item {
    Raw(String),
    Type(TypeDef),
}

/// One generated `mod.rs`, before rendering.
#[derive(Debug, Clone)]
pub(crate) struct ModuleFile {
    /// Path relative to `generated/` (`shared/mod.rs`, `zones/mod.rs`).
    pub rel_path: String,
    pub items: Vec<Item>,
}

impl ModuleFile {
    pub(crate) fn new(rel_path: impl Into<String>) -> Self {
        Self {
            rel_path: rel_path.into(),
            items: Vec::new(),
        }
    }

    pub(crate) fn raw(&mut self, text: String) {
        if !text.is_empty() {
            self.items.push(Item::Raw(text));
        }
    }

    pub(crate) fn push(&mut self, def: TypeDef) {
        self.items.push(Item::Type(def));
    }

    /// Names of every type this module defines.
    pub(crate) fn type_names(&self) -> HashSet<String> {
        self.types().map(|t| t.name.clone()).collect()
    }

    fn types(&self) -> impl Iterator<Item = &TypeDef> {
        self.items.iter().filter_map(|i| match i {
            Item::Type(t) => Some(t),
            Item::Raw(_) => None,
        })
    }

    pub(crate) fn render(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();
        for item in &self.items {
            match item {
                Item::Raw(text) => out.push_str(text),
                Item::Type(def) => def.render(&mut out)?,
            }
        }
        Ok(out)
    }
}

/// Identifiers in a Rust type string (`Option<Vec<Foo>>` → Option, Vec, Foo).
fn type_idents(ty: &str) -> impl Iterator<Item = &str> {
    ty.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
}

/// Whether a field type names no generated type — only std and `serde_json`.
///
/// Such a type means the same thing in every module, so a struct made only of
/// them can live in `shared` and be used from anywhere.
pub(crate) fn names_no_generated_type(ty: &str) -> bool {
    const KNOWN: &[&str] = &[
        "String",
        "i64",
        "f64",
        "bool",
        "Vec",
        "Option",
        "Box",
        "std",
        "collections",
        "HashMap",
        "serde_json",
        "Value",
    ];
    type_idents(ty).all(|ident| KNOWN.contains(&ident))
}

fn mentions(ty: &str, name: &str) -> bool {
    type_idents(ty).any(|ident| ident == name)
}

/// The comparable form of a field list: everything but the doc comments.
fn fields_key(fields: &[Field], replace_result: bool) -> String {
    let mut key = String::new();
    for f in fields {
        let ty = if replace_result && f.name == "result" {
            "T"
        } else {
            f.ty.as_str()
        };
        let _ = write!(key, "{}|{}:{};", f.attrs.join(" "), f.name, ty);
    }
    key
}

/// `Default` is left out: a shape that some members need `Default` on and
/// others do not is still one shape — the canonical type keeps `Default`
/// (see [`with_default_if_any`]).
fn struct_key(
    derives: &[String],
    attrs: &[String],
    fields: &[Field],
    replace_result: bool,
) -> String {
    let derives: Vec<&str> = derives
        .iter()
        .map(String::as_str)
        .filter(|d| *d != "Default")
        .collect();
    format!(
        "{}#{}#{}",
        derives.join(","),
        attrs.join(" "),
        fields_key(fields, replace_result)
    )
}

/// Where one type lives: (module index, item index).
type Loc = (usize, usize);

fn type_at(files: &[ModuleFile], (f, i): Loc) -> &TypeDef {
    match &files[f].items[i] {
        Item::Type(t) => t,
        Item::Raw(_) => unreachable!("Loc always points at a type item"),
    }
}

fn type_at_mut(files: &mut [ModuleFile], (f, i): Loc) -> &mut TypeDef {
    match &mut files[f].items[i] {
        Item::Type(t) => t,
        Item::Raw(_) => unreachable!("Loc always points at a type item"),
    }
}

fn struct_locs(files: &[ModuleFile]) -> Vec<Loc> {
    let mut locs = Vec::new();
    for (f, file) in files.iter().enumerate() {
        for (i, item) in file.items.iter().enumerate() {
            if let Item::Type(TypeDef {
                body: TypeBody::Struct { generic: None, .. },
                ..
            }) = item
            {
                locs.push((f, i));
            }
        }
    }
    locs
}

/// Whether the struct at `loc` derives `Default`.
fn derives_default(files: &[ModuleFile], loc: Loc) -> bool {
    matches!(&type_at(files, loc).body, TypeBody::Struct { derives, .. } if derives.iter().any(|d| d == "Default"))
}

/// Give `def` `Default` (in its usual place, after `Clone`) when any of the
/// structs it stands for had it.
fn with_default_if_any(def: &mut TypeDef, any_default: bool) {
    if let TypeBody::Struct { derives, .. } = &mut def.body {
        if any_default && !derives.iter().any(|d| d == "Default") {
            let at = derives
                .iter()
                .position(|d| d == "Clone")
                .map_or(0, |i| i + 1);
            derives.insert(at, "Default".to_string());
        }
    }
}

/// Groups with at least two members, largest first, so the most common shape
/// gets the plain name and rarer ones the numbered variants.
fn by_size(groups: BTreeMap<String, Vec<Loc>>) -> Vec<Vec<Loc>> {
    let mut groups: Vec<(String, Vec<Loc>)> =
        groups.into_iter().filter(|(_, m)| m.len() >= 2).collect();
    groups.sort_by(|(ka, a), (kb, b)| b.len().cmp(&a.len()).then_with(|| ka.cmp(kb)));
    groups.into_iter().map(|(_, m)| m).collect()
}

/// Turn the type at `loc` into an alias of `target`, keeping its docs.
fn make_alias(files: &mut [ModuleFile], loc: Loc, target: String) {
    let def = type_at_mut(files, loc);
    def.doc.push(String::new());
    def.doc.push(format!("Same shape as `{target}`."));
    def.body = TypeBody::Alias { target };
}

/// Run every pass. `shared` is the index of the `shared` module in `files`.
pub(crate) fn shrink(files: &mut [ModuleFile], shared: usize) {
    hoist_envelopes(files, shared);
    hoist_shared_shapes(files, shared);
    dedupe_within_modules(files);
}

/// A name not yet used in `taken`, starting from `base`.
fn unique_name(base: &str, taken: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}{n}");
        n += 1;
    }
    taken.insert(name.clone());
    name
}

fn pascal(field: &str) -> String {
    crate::to_pascal_case(field.trim_start_matches("r#"))
}

/// The prefix that names a `shared` item from module `file`.
fn shared_path(file: usize, shared: usize) -> &'static str {
    if file == shared {
        ""
    } else {
        "super::shared::"
    }
}

/// Structs that differ only in their `result` type become one generic
/// `ApiEnvelope…<T>` in `shared`.
///
/// Vendors wrap every response the same way — Cloudflare's
/// `{ success, errors, messages, result }` — and the spec repeats that wrapper
/// per endpoint. Only `result` varies, so it becomes the type parameter, and
/// carries the field's whole type (`Option<Zone>` or `Zone`), which keeps
/// required-ness exactly as the spec says.
pub(crate) fn hoist_envelopes(files: &mut [ModuleFile], shared: usize) {
    let mut groups: BTreeMap<String, Vec<Loc>> = BTreeMap::new();
    for loc in struct_locs(files) {
        let def = type_at(files, loc);
        let TypeBody::Struct {
            derives,
            attrs,
            fields,
            ..
        } = &def.body
        else {
            continue;
        };
        let Some(result) = fields.iter().find(|f| f.name == "result") else {
            continue;
        };
        let others_portable = fields
            .iter()
            .filter(|f| f.name != "result")
            .all(|f| names_no_generated_type(&f.ty));
        if fields.len() < 2 || !others_portable || mentions(&result.ty, &def.name) {
            continue;
        }
        groups
            .entry(struct_key(derives, attrs, fields, true))
            .or_default()
            .push(loc);
    }

    let mut taken = files[shared].type_names();
    let mut hoisted: Vec<TypeDef> = Vec::new();
    for members in by_size(groups) {
        let any_default = members.iter().any(|&loc| derives_default(files, loc));
        let mut template = type_at(files, members[0]).clone();
        with_default_if_any(&mut template, any_default);
        let TypeBody::Struct {
            derives,
            attrs,
            fields,
            ..
        } = template.body
        else {
            continue;
        };
        let others: Vec<&str> = fields
            .iter()
            .filter(|f| f.name != "result")
            .map(|f| f.name.as_str())
            .collect();
        // Named for what sits beside `result`; a long field list would make
        // an unreadable name, so those take the first member's name instead.
        let base = if others == ["errors", "messages", "success"] {
            "ApiEnvelope".to_string()
        } else if others.len() <= 4 {
            format!(
                "ApiEnvelope{}",
                others.iter().map(|f| pascal(f)).collect::<String>()
            )
        } else {
            format!("{}Envelope", template.name)
        };
        let name = unique_name(&base, &mut taken);
        let generic_fields: Vec<Field> = fields
            .into_iter()
            .map(|mut f| {
                if f.name == "result" {
                    f.ty = "T".to_string();
                    f.doc = "The endpoint's payload.".to_string();
                }
                f
            })
            .collect();
        hoisted.push(TypeDef {
            name: name.clone(),
            doc: vec![format!(
                "Response envelope shared by {} endpoints; `T` is the type of `result`.",
                members.len()
            )],
            body: TypeBody::Struct {
                derives,
                attrs,
                generic: Some("T".to_string()),
                fields: generic_fields,
            },
        });
        for loc in members {
            let result_ty = match &type_at(files, loc).body {
                TypeBody::Struct { fields, .. } => fields
                    .iter()
                    .find(|f| f.name == "result")
                    .map(|f| f.ty.clone())
                    .unwrap_or_default(),
                TypeBody::Alias { .. } => continue,
            };
            let target = format!("{}{name}<{result_ty}>", shared_path(loc.0, shared));
            make_alias(files, loc, target);
        }
    }
    append_shared(files, shared, hoisted, "RESPONSE ENVELOPES");
}

/// Identical structs whose fields name no generated type are defined once in
/// `shared`, under a name that says what they are (`AccountIdArgs`,
/// `EmptyObject`), and every occurrence becomes an alias of it.
pub(crate) fn hoist_shared_shapes(files: &mut [ModuleFile], shared: usize) {
    let mut groups: BTreeMap<String, Vec<Loc>> = BTreeMap::new();
    for loc in struct_locs(files) {
        let TypeBody::Struct {
            derives,
            attrs,
            fields,
            ..
        } = &type_at(files, loc).body
        else {
            continue;
        };
        if fields.iter().all(|f| names_no_generated_type(&f.ty)) {
            groups
                .entry(struct_key(derives, attrs, fields, false))
                .or_default()
                .push(loc);
        }
    }

    let mut taken = files[shared].type_names();
    let mut hoisted: Vec<TypeDef> = Vec::new();
    for members in by_size(groups) {
        let any_default = members.iter().any(|&loc| derives_default(files, loc));
        let mut template = type_at(files, members[0]).clone();
        with_default_if_any(&mut template, any_default);
        let TypeBody::Struct {
            derives, fields, ..
        } = &template.body
        else {
            continue;
        };
        let kind = if derives.iter().any(|d| d == "Deserialize") {
            "Object"
        } else {
            "Args"
        };
        let base = match fields.len() {
            0 => format!("Empty{kind}"),
            1..=3 => format!(
                "{}{kind}",
                fields.iter().map(|f| pascal(&f.name)).collect::<String>()
            ),
            _ => format!("{}Shape", template.name),
        };
        let name = unique_name(&base, &mut taken);
        let mut canonical = template.clone();
        canonical.name.clone_from(&name);
        canonical.doc = vec![format!("A shape {} generated types share.", members.len())];
        hoisted.push(canonical);
        for loc in members {
            let target = format!("{}{name}", shared_path(loc.0, shared));
            make_alias(files, loc, target);
        }
    }
    append_shared(files, shared, hoisted, "SHARED SHAPES");
}

/// Within each module, identical structs become aliases of the first.
pub(crate) fn dedupe_within_modules(files: &mut [ModuleFile]) {
    let locs = struct_locs(files);
    let mut first: HashMap<(usize, String), (Loc, String)> = HashMap::new();
    let mut aliases: Vec<(Loc, String)> = Vec::new();
    let mut needs_default: HashSet<Loc> = HashSet::new();
    for loc in locs {
        let def = type_at(files, loc);
        let TypeBody::Struct {
            derives,
            attrs,
            fields,
            ..
        } = &def.body
        else {
            continue;
        };
        // A struct that names itself (through `Box`) has no twin: the twin
        // would name *itself*.
        let key = (loc.0, struct_key(derives, attrs, fields, false));
        match first.get(&key) {
            Some((canonical_loc, canonical)) => {
                if derives_default(files, loc) {
                    needs_default.insert(*canonical_loc);
                }
                aliases.push((loc, canonical.clone()));
            }
            None => {
                first.insert(key, (loc, def.name.clone()));
            }
        }
    }
    for loc in needs_default {
        with_default_if_any(type_at_mut(files, loc), true);
    }
    for (loc, target) in aliases {
        make_alias(files, loc, target);
    }
}

fn append_shared(files: &mut [ModuleFile], shared: usize, mut defs: Vec<TypeDef>, title: &str) {
    if defs.is_empty() {
        return;
    }
    defs.sort_by(|a, b| a.name.cmp(&b.name));
    let file = &mut files[shared];
    file.raw(format!(
        "// =============================================================================\n\
         // {title}\n\
         // =============================================================================\n\n"
    ));
    for def in defs {
        file.push(def);
    }
}

/// Names defined in more than one place must not collide; checked in tests.
#[cfg(test)]
pub(crate) fn duplicate_names(file: &ModuleFile) -> std::collections::BTreeSet<String> {
    let mut seen = HashSet::new();
    file.types()
        .filter(|t| !seen.insert(t.name.clone()))
        .map(|t| t.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
