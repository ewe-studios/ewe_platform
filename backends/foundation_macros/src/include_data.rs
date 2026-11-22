//! Compile-time data inclusion: `include_json!` and `include_yaml!`.
//!
//! WHY: a crate that needs a fixed table should not read and parse a file at
//! run time, and should not turn the data into a hand-written match arm.
//!
//! WHAT: one proc macro expands to a `phf::Map<&'static str, &'static str>`
//! literal that holds every pair of the file.
//!
//! HOW: the macro reads the file from `CARGO_MANIFEST_DIR`, parses it with
//! `serde_json` (or `serde_yaml_ng`), refuses a duplicate key or a non-string
//! value, and writes the map with `phf_codegen::Map`.
//!
//! The emitted literal names `::foundation_nostd::phf`, so a crate that uses
//! the macro depends on `foundation_nostd` only. It needs no `phf` dependency
//! and no build script.

use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::Deserializer as _;

/// `include_json!("data/table.json")` — a map from one JSON object of strings.
pub(crate) fn include_json(input: TokenStream) -> TokenStream {
    match expand_json(input) {
        Ok(tokens) => tokens.into(),
        Err(message) => compile_error(&message),
    }
}

/// `include_yaml!("data/table.yaml")` — a map from one YAML mapping of strings.
pub(crate) fn include_yaml(input: TokenStream) -> TokenStream {
    match expand_yaml(input) {
        Ok(tokens) => tokens.into(),
        Err(message) => compile_error(&message),
    }
}

fn expand_json(input: TokenStream) -> Result<TokenStream2, String> {
    let path = parse_path(input, "include_json")?;
    let text = read_file(&path, "include_json")?;
    let visitor = JsonPairs {
        file: path.as_str(),
    };
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    let pairs = serde::Deserializer::deserialize_any(&mut deserializer, visitor)
        .map_err(|error| parse_error("include_json", &path, &error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| parse_error("include_json", &path, &error.to_string()))?;
    Ok(build_map(&pairs))
}

fn expand_yaml(input: TokenStream) -> Result<TokenStream2, String> {
    let path = parse_path(input, "include_yaml")?;
    let text = read_file(&path, "include_yaml")?;
    let visitor = YamlPairs {
        file: path.as_str(),
    };
    let deserializer = serde_yaml_ng::Deserializer::from_str(&text);
    // serde_yaml_ng implements the deserializer trait on the value itself, not
    // on `&mut`, so the call consumes it. The first document is enough.
    let pairs = deserializer
        .deserialize_any(visitor)
        .map_err(|error| parse_error("include_yaml", &path, &error.to_string()))?;
    Ok(build_map(&pairs))
}

/// Read the one string literal that names the file.
fn parse_path(input: TokenStream, macro_name: &str) -> Result<String, String> {
    // `syn::parse` refuses two arguments and a non-string literal.
    syn::parse::<syn::LitStr>(input)
        .map(|literal| literal.value())
        .map_err(|_| format!("{macro_name}! takes exactly one string literal path"))
}

/// Read the file. The path is relative to the calling crate's manifest directory.
fn read_file(path: &str, macro_name: &str) -> Result<String, String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let full_path = PathBuf::from(&manifest_dir).join(path);
    fs::read_to_string(&full_path)
        .map_err(|_| format!("{macro_name}! cannot read {path} (relative to {manifest_dir})"))
}

/// Keep the message of an error this module raised, and give a raw parser error
/// the file name in front of it.
fn parse_error(macro_name: &str, path: &str, message: &str) -> String {
    if message.starts_with(macro_name) {
        message.to_string()
    } else {
        format!("{macro_name}! cannot parse {path}: {message}")
    }
}

/// Write the `phf::Map` literal. The caller already refused a duplicate key.
fn build_map(pairs: &[(String, String)]) -> TokenStream2 {
    let mut map = phf_codegen::Map::new();
    // phf_codegen 0.14 writes a bare `::phf` by default. Name the re-export so
    // the caller does not need a `phf` dependency.
    map.phf_path("::foundation_nostd::phf");
    for (key, value) in pairs {
        map.entry(key.as_str(), format!("{value:?}"));
    }
    map.build()
        .to_string()
        .parse()
        .expect("phf_codegen emits valid Rust tokens")
}

/// Emit `compile_error!` with the full message.
///
/// The macro expands in expression position, so the error token has no
/// trailing semicolon.
fn compile_error(message: &str) -> TokenStream {
    quote! { compile_error!(#message) }.into()
}

/// The top-level value is not an object.
fn not_object<E: de::Error>(macro_name: &str, file: &str) -> E {
    E::custom(format!(
        "{macro_name}! needs a JSON object of strings in {file}"
    ))
}

fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

/// Collect the pairs of a JSON object. A `MapAccess` walk sees every pair, so a
/// repeated key is visible here. `serde_json` alone keeps the last pair.
struct JsonPairs<'a> {
    file: &'a str,
}

impl<'de> Visitor<'de> for JsonPairs<'_> {
    type Value = Vec<(String, String)>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "include_json! needs a JSON object of strings in {}",
            self.file
        )
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = HashSet::new();
        let mut pairs = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "include_json! finds a duplicate key {key:?} in {}",
                    self.file
                )));
            }
            let value = map.next_value::<serde_json::Value>()?;
            match value {
                serde_json::Value::String(text) => pairs.push((key, text)),
                other => {
                    return Err(de::Error::custom(format!(
                        "include_json! needs a string value for {key:?}, found {}",
                        json_kind(&other)
                    )));
                }
            }
        }
        Ok(pairs)
    }

    fn visit_seq<A>(self, _sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Err(not_object("include_json", self.file))
    }

    fn visit_str<E: de::Error>(self, _text: &str) -> Result<Self::Value, E> {
        Err(not_object("include_json", self.file))
    }

    fn visit_bool<E: de::Error>(self, _flag: bool) -> Result<Self::Value, E> {
        Err(not_object("include_json", self.file))
    }

    fn visit_i64<E: de::Error>(self, _number: i64) -> Result<Self::Value, E> {
        Err(not_object("include_json", self.file))
    }

    fn visit_u64<E: de::Error>(self, _number: u64) -> Result<Self::Value, E> {
        Err(not_object("include_json", self.file))
    }

    fn visit_f64<E: de::Error>(self, _number: f64) -> Result<Self::Value, E> {
        Err(not_object("include_json", self.file))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Err(not_object("include_json", self.file))
    }
}

fn yaml_kind(value: &serde_yaml_ng::Value) -> &'static str {
    match value {
        serde_yaml_ng::Value::Null => "null",
        serde_yaml_ng::Value::Bool(_) => "a boolean",
        serde_yaml_ng::Value::Number(_) => "a number",
        serde_yaml_ng::Value::String(_) => "a string",
        serde_yaml_ng::Value::Sequence(_) => "an array",
        serde_yaml_ng::Value::Mapping(_) => "a mapping",
        serde_yaml_ng::Value::Tagged(_) => "a tagged value",
    }
}

/// The text form of a YAML scalar key. A sequence, a mapping or a null key has
/// no text form.
fn yaml_key_text(value: serde_yaml_ng::Value) -> Option<String> {
    match value {
        serde_yaml_ng::Value::String(text) => Some(text),
        serde_yaml_ng::Value::Bool(flag) => Some(flag.to_string()),
        serde_yaml_ng::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Collect the pairs of a YAML mapping, with the same duplicate check as JSON.
struct YamlPairs<'a> {
    file: &'a str,
}

impl<'de> Visitor<'de> for YamlPairs<'_> {
    type Value = Vec<(String, String)>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "include_yaml! needs a YAML mapping of strings in {}",
            self.file
        )
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = HashSet::new();
        let mut pairs = Vec::new();
        while let Some(raw_key) = map.next_key::<serde_yaml_ng::Value>()? {
            let Some(key) = yaml_key_text(raw_key) else {
                return Err(de::Error::custom(format!(
                    "include_yaml! needs a scalar key in {}",
                    self.file
                )));
            };
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "include_yaml! finds a duplicate key {key:?} in {}",
                    self.file
                )));
            }
            let value = map.next_value::<serde_yaml_ng::Value>()?;
            match value {
                serde_yaml_ng::Value::String(text) => pairs.push((key, text)),
                other => {
                    return Err(de::Error::custom(format!(
                        "include_yaml! needs a string value for {key:?}, found {}",
                        yaml_kind(&other)
                    )));
                }
            }
        }
        Ok(pairs)
    }

    fn visit_seq<A>(self, _sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Err(not_yaml_mapping(self.file))
    }

    fn visit_str<E: de::Error>(self, _text: &str) -> Result<Self::Value, E> {
        Err(not_yaml_mapping(self.file))
    }

    fn visit_bool<E: de::Error>(self, _flag: bool) -> Result<Self::Value, E> {
        Err(not_yaml_mapping(self.file))
    }

    fn visit_i64<E: de::Error>(self, _number: i64) -> Result<Self::Value, E> {
        Err(not_yaml_mapping(self.file))
    }

    fn visit_u64<E: de::Error>(self, _number: u64) -> Result<Self::Value, E> {
        Err(not_yaml_mapping(self.file))
    }

    fn visit_f64<E: de::Error>(self, _number: f64) -> Result<Self::Value, E> {
        Err(not_yaml_mapping(self.file))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Err(not_yaml_mapping(self.file))
    }
}

fn not_yaml_mapping<E: de::Error>(file: &str) -> E {
    E::custom(format!(
        "include_yaml! needs a YAML mapping of strings in {file}"
    ))
}
