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
pub const SCHEMA_DERIVES: &[&str] = &[
    "Debug",
    "Clone",
    "Default",
    "Serialize",
    "Deserialize",
    "JsonHash",
];

/// Derives for an endpoint's `*Args` struct. Never deserialized; `Default`
/// lets callers fill only the parameters they need (`..Default::default()`).
pub const ARGS_DERIVES: &[&str] = &["Debug", "Clone", "Default", "Serialize", "JsonHash"];

/// One field of a generated struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Doc comment text (without `///`), or empty.
    pub doc: String,
    /// Attribute lines, e.g. `#[serde(rename = "Id")]`.
    pub attrs: Vec<String>,
    /// The field identifier.
    pub name: String,
    /// The field's Rust type.
    pub ty: String,
}

/// One variant of a generated untagged enum (an `OpenAPI` `oneOf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Doc comment text (without `///`), or empty.
    pub doc: String,
    /// The variant identifier.
    pub name: String,
    /// What the variant holds.
    pub shape: VariantShape,
}

/// What a [`Variant`] holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantShape {
    /// `Name(Type)`: the member is another type (a `$ref`, a string, a list).
    Newtype(String),
    /// `Name { fields }`: the member is an inline object.
    Struct(Vec<Field>),
}

/// The body of a generated type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBody {
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
    /// `#[serde(untagged)] pub enum Name { variants }`: a `oneOf`. serde tries
    /// each variant in order, so list the most specific member first.
    Union {
        derives: Vec<String>,
        variants: Vec<Variant>,
    },
}

/// One generated type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    pub name: String,
    /// Doc comment text (without `///`), one entry per line.
    pub doc: Vec<String>,
    pub body: TypeBody,
}

impl TypeDef {
    /// A struct with the given derives and fields.
    #[must_use]
    pub fn structure(name: &str, doc: &str, derives: &[&str], fields: Vec<Field>) -> Self {
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
    #[must_use]
    pub fn alias(name: &str, doc: &str, target: &str) -> Self {
        Self {
            name: name.to_string(),
            doc: vec![doc.to_string()],
            body: TypeBody::Alias {
                target: target.to_string(),
            },
        }
    }

    /// An untagged enum with the given variants (a `oneOf`).
    ///
    /// It has no `Default`: a union has no natural default member.
    #[must_use]
    pub fn union(name: &str, doc: &str, variants: Vec<Variant>) -> Self {
        Self {
            name: name.to_string(),
            doc: vec![doc.to_string()],
            body: TypeBody::Union {
                derives: ["Debug", "Clone", "Serialize", "Deserialize", "JsonHash"]
                    .iter()
                    .map(|d| (*d).to_string())
                    .collect(),
                variants,
            },
        }
    }

    /// Render this type's definition into `out`.
    ///
    /// # Errors
    ///
    /// Returns the [`std::fmt::Error`] that a `writeln!` can surface; writing
    /// to a `String` never fails in practice.
    pub fn render(&self, out: &mut String) -> std::fmt::Result {
        for line in &self.doc {
            if line.is_empty() {
                writeln!(out, "///")?;
            } else {
                writeln!(out, "/// {line}")?;
            }
        }
        match &self.body {
            TypeBody::Alias { target } => writeln!(out, "pub type {} = {};", self.name, target)?,
            TypeBody::Union { derives, variants } => {
                writeln!(out, "#[derive({})]", derives.join(", "))?;
                writeln!(out, "#[serde(untagged)]")?;
                writeln!(out, "pub enum {} {{", self.name)?;
                for variant in variants {
                    if !variant.doc.is_empty() {
                        writeln!(out, "    /// {}", variant.doc)?;
                    }
                    match &variant.shape {
                        VariantShape::Newtype(ty) => writeln!(out, "    {}({ty}),", variant.name)?,
                        VariantShape::Struct(fields) => {
                            writeln!(out, "    {} {{", variant.name)?;
                            for field in fields {
                                if !field.doc.is_empty() {
                                    writeln!(out, "        /// {}", field.doc)?;
                                }
                                for attr in &field.attrs {
                                    writeln!(out, "        {attr}")?;
                                }
                                writeln!(out, "        {}: {},", field.name, field.ty)?;
                            }
                            writeln!(out, "    }},")?;
                        }
                    }
                }
                writeln!(out, "}}")?;
                // `*Args` structs derive `Default` and may hold a union as their
                // body, so a union has one: its first member, defaulted.
                if let Some(first) = variants.first() {
                    writeln!(out)?;
                    writeln!(out, "impl Default for {} {{", self.name)?;
                    writeln!(out, "    fn default() -> Self {{")?;
                    match &first.shape {
                        VariantShape::Newtype(_) => {
                            writeln!(out, "        Self::{}(Default::default())", first.name)?;
                        }
                        VariantShape::Struct(fields) => {
                            writeln!(out, "        Self::{} {{", first.name)?;
                            for field in fields {
                                writeln!(out, "            {}: Default::default(),", field.name)?;
                            }
                            writeln!(out, "        }}")?;
                        }
                    }
                    writeln!(out, "    }}")?;
                    writeln!(out, "}}")?;
                }
            }
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
pub struct ModuleFile {
    /// Path relative to `generated/` (`shared/mod.rs`, `zones/mod.rs`).
    pub rel_path: String,
    items: Vec<Item>,
}

impl ModuleFile {
    pub fn new(rel_path: impl Into<String>) -> Self {
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

    pub fn push(&mut self, def: TypeDef) {
        self.items.push(Item::Type(def));
    }

    /// Names of every type this module defines.
    pub(crate) fn type_names(&self) -> HashSet<String> {
        self.types().map(|t| t.name.clone()).collect()
    }

    pub fn types(&self) -> impl Iterator<Item = &TypeDef> {
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
pub fn hoist_envelopes(files: &mut [ModuleFile], shared: usize) {
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
                TypeBody::Alias { .. } | TypeBody::Union { .. } => continue,
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
pub fn hoist_shared_shapes(files: &mut [ModuleFile], shared: usize) {
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
pub fn dedupe_within_modules(files: &mut [ModuleFile]) {
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
