pub mod arrow_schema_writer;
pub mod error;
pub mod field_extractor;
pub mod json_schema_writer;

use std::path::{Path, PathBuf};

use error::SchemaGenError;
use field_extractor::{extract_fields, StructField};

#[derive(Debug, Clone)]
pub struct DiscoveredStruct {
    pub name: String,
    pub fields: Vec<StructField>,
    pub has_arrow_schema: bool,
    pub has_json_schema: bool,
    pub source_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GeneratedSchema {
    pub struct_name: String,
    pub path: PathBuf,
    pub content: String,
}

pub struct SchemaGenerator {
    crate_dir: PathBuf,
    crate_name: String,
    structs: Vec<DiscoveredStruct>,
}

impl SchemaGenerator {
    /// # Errors
    ///
    /// Returns error if the crate directory is missing, has no Cargo.toml,
    /// or source files cannot be parsed.
    pub fn new(crate_dir: &Path) -> Result<Self, SchemaGenError> {
        if !crate_dir.exists() {
            return Err(SchemaGenError::CrateNotFound(crate_dir.to_path_buf()));
        }

        let cargo_toml_path = crate_dir.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return Err(SchemaGenError::NoCargoToml(crate_dir.to_path_buf()));
        }

        let cargo_content =
            std::fs::read_to_string(&cargo_toml_path).map_err(|e| SchemaGenError::Io {
                path: cargo_toml_path.clone(),
                source: e,
            })?;
        let cargo_toml: toml::Value =
            toml::from_str(&cargo_content).map_err(|e| SchemaGenError::CargoTomlParse {
                path: cargo_toml_path,
                source: e,
            })?;

        let crate_name = cargo_toml
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        let src_dir = crate_dir.join("src");
        let rust_files = foundation_codegen::file_walker::find_rust_files(&src_dir)?;

        let mut structs = Vec::new();
        for file_path in &rust_files {
            let content =
                std::fs::read_to_string(file_path).map_err(|e| SchemaGenError::Io {
                    path: file_path.clone(),
                    source: e,
                })?;

            let syntax = syn::parse_file(&content).map_err(|e| SchemaGenError::ParseError {
                path: file_path.clone(),
                message: e.to_string(),
            })?;

            for item in &syntax.items {
                if let syn::Item::Struct(item_struct) = item {
                    let (has_arrow, has_json) = check_derives(&item_struct.attrs);
                    if !has_arrow && !has_json {
                        continue;
                    }

                    if let Some(fields) = extract_fields(item_struct) {
                        structs.push(DiscoveredStruct {
                            name: item_struct.ident.to_string(),
                            fields,
                            has_arrow_schema: has_arrow,
                            has_json_schema: has_json,
                            source_file: file_path.clone(),
                        });
                    }
                }
            }
        }

        Ok(Self {
            crate_dir: crate_dir.to_path_buf(),
            crate_name,
            structs,
        })
    }

    #[must_use]
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    #[must_use]
    pub fn structs(&self) -> &[DiscoveredStruct] {
        &self.structs
    }

    /// # Errors
    ///
    /// Returns error if no schema structs were found or file I/O fails.
    #[allow(clippy::missing_panics_doc)]
    pub fn generate(
        &self,
        emit_json_schema: bool,
        emit_arrow_schema: bool,
    ) -> Result<Vec<GeneratedSchema>, SchemaGenError> {
        if self.structs.is_empty() {
            return Err(SchemaGenError::NoSchemaStructs(self.crate_dir.clone()));
        }

        let mut generated = Vec::new();

        for s in &self.structs {
            if emit_json_schema && s.has_json_schema {
                let schema = json_schema_writer::build_json_schema(&s.name, &s.fields);
                let content = serde_json::to_string_pretty(&schema).expect("valid json");
                let rel_path = PathBuf::from("sdk/jsonschema").join(format!("{}.json", s.name));
                let full_path = self.crate_dir.join(&rel_path);

                write_file(&full_path, &content)?;
                generated.push(GeneratedSchema {
                    struct_name: s.name.clone(),
                    path: rel_path,
                    content,
                });
            }

            if emit_arrow_schema && s.has_arrow_schema {
                let schema = arrow_schema_writer::build_arrow_schema(&s.name, &s.fields);
                let content = serde_json::to_string_pretty(&schema).expect("valid json");
                let rel_path = PathBuf::from("sdk/arrow_schema").join(format!("{}.json", s.name));
                let full_path = self.crate_dir.join(&rel_path);

                write_file(&full_path, &content)?;
                generated.push(GeneratedSchema {
                    struct_name: s.name.clone(),
                    path: rel_path,
                    content,
                });
            }
        }

        Ok(generated)
    }
}

fn write_file(path: &Path, content: &str) -> Result<(), SchemaGenError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SchemaGenError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    std::fs::write(path, content).map_err(|e| SchemaGenError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

fn check_derives(attrs: &[syn::Attribute]) -> (bool, bool) {
    let mut has_arrow = false;
    let mut has_json = false;

    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let Ok(nested) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) else {
            continue;
        };
        for path in &nested {
            if path.is_ident("ArrowSchema") {
                has_arrow = true;
            }
            if path.is_ident("JsonSchema") {
                has_json = true;
            }
        }
    }

    (has_arrow, has_json)
}
