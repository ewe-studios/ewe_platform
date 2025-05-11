extern crate proc_macro;
use new_mime_guess::MimeGuess;
use quote::{quote, ToTokens};
use sha2::{Digest, Sha256};
use std::io::prelude::*;
use std::iter;
use std::time::SystemTime;
use std::{
    env, error, fs,
    path::{Path, PathBuf},
};
use syn::Data;
use syn::{parse_macro_input, Expr, Lit, Meta};

use proc_macro2::{Literal, Punct, Spacing, TokenStream, TokenTree};

#[allow(unused)]
#[derive(Debug)]
pub enum GenError {
    UnableToGetModifiedDate,
    UnableToGetMimeType,
    Any(Box<dyn error::Error>),
}

impl From<Box<dyn error::Error>> for GenError {
    fn from(value: Box<dyn error::Error>) -> Self {
        Self::Any(value)
    }
}

impl std::error::Error for GenError {}
impl core::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub fn embed_file_on_struct(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(item as syn::DeriveInput);

    match &ast.data {
        Data::Struct(_) => {}
        _ => {
            panic!("Only declaration on Struct is allowed")
        }
    };

    let file_path = if let Some(path_str) = get_attr(&ast, "source") {
        path_str
    } else {
        panic!("A #[path=\"...\"] is required for the #[EmbedFileAs] macro")
    };

    proc_macro::TokenStream::from(impl_embeddable_file(&ast.ident, file_path))
}

fn get_attr(ast: &syn::DeriveInput, attr_name: &str) -> Option<String> {
    let attributed: Option<&syn::Meta> = ast
        .attrs
        .iter()
        .filter(|value| value.path().is_ident(attr_name))
        .map(|value| &value.meta)
        .take(1)
        .next();

    match attributed {
        Some(Meta::NameValue(item)) => match &item.value {
            Expr::Lit(lit) => match &lit.lit {
                Lit::Str(inner) => Some(inner.value().to_string()),
                _ => None,
            },
            _ => None,
        },
        Some(_) => None,
        None => None,
    }
}

fn find_root_cargo(
    manifest_dir: PathBuf,
    previous_dir: Option<PathBuf>,
) -> Option<(PathBuf, PathBuf)> {
    if let Ok(true) = fs::exists(manifest_dir.join("Cargo.toml")) {
        return find_root_cargo(
            manifest_dir
                .parent()
                .expect("path to have parent")
                .to_owned(),
            Some(manifest_dir.to_owned()),
        );
    }

    if let Ok(true) = fs::exists(manifest_dir.join("cargo.toml")) {
        return find_root_cargo(
            manifest_dir
                .parent()
                .expect("path to have parent")
                .to_owned(),
            Some(manifest_dir.to_owned()),
        );
    }

    if let Some(prev_dir) = previous_dir {
        let prev_cargo_file = prev_dir.join("Cargo.toml");
        if fs::exists(&prev_cargo_file).is_ok() {
            return Some((prev_dir.to_owned(), prev_cargo_file));
        }

        let prev_cargo_file2 = prev_dir.join("cargo.toml");
        if fs::exists(&prev_cargo_file2).is_ok() {
            return Some((prev_dir.to_owned(), prev_cargo_file2));
        }
    }

    None
}

static ROOT_WORKSPACE_MATCHER: &str = "$ROOT_CRATE";
static CURRENT_CRATE_MATCHER: &str = "$CURRENT_CRATE";

fn impl_embeddable_file(struct_name: &syn::Ident, target_source: String) -> TokenStream {
    let cargo_manifest_dir_env =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let manifest_dir = Path::new(&cargo_manifest_dir_env);
    let working_dir = env::current_dir().expect("get current working directory");

    let (root_workspace, _) = find_root_cargo(manifest_dir.to_owned(), None)
        .expect("heuristically identify root workspace or crate");

    let root_workspace_str = root_workspace.to_str().unwrap_or_else(|| {
        panic!("cannot get str path for {:?}", root_workspace);
    });

    let project_dir = manifest_dir
        .strip_prefix(&working_dir)
        .expect("should be from home directory");

    let target_file = if target_source.contains(CURRENT_CRATE_MATCHER) {
        target_source.replace(CURRENT_CRATE_MATCHER, &cargo_manifest_dir_env)
    } else if target_source.contains(ROOT_WORKSPACE_MATCHER) {
        target_source.replace(ROOT_WORKSPACE_MATCHER, &root_workspace_str)
    } else {
        target_source
    };

    let embed_file_path = if target_file.starts_with("/") {
        Path::new(&target_file).to_owned()
    } else {
        manifest_dir.join(target_file.as_str())
    };

    match fs::exists(&embed_file_path) {
        Ok(true) => {}
        Ok(false) => {
            panic!("Failed to find file: {:?}", &embed_file_path);
        }
        Err(err) => {
            panic!(
                "Failed to call fs.exists on file: {:?} due to {:?}",
                &embed_file_path, err
            );
        }
    };

    let embedded_file_relative_path = embed_file_path
        .strip_prefix(&working_dir)
        .expect("should be from home directory");

    let embeddable_file =
        get_file(embed_file_path.clone()).expect("Failed to generate file embeddings");

    let target_file_tokens = Literal::string(target_file.as_str());
    let etag_tokens = Literal::string(embeddable_file.etag.as_str());
    let hash_tokens = Literal::string(embeddable_file.hash.as_str());
    let project_dir_tokens =
        Literal::string(project_dir.to_str().expect("get path string for file"));
    let embedded_file_relative_path_tokens = Literal::string(
        embedded_file_relative_path
            .to_str()
            .expect("get path string for file"),
    );

    let date_modified_tokens = match embeddable_file.date_modified {
        Some(inner) => quote! {
            Some(#inner)
        },
        None => quote! {
            None
        },
    };

    let mime_type = match embeddable_file.mime_type {
        Some(inner) => quote! {
            Some(#inner)
        },
        None => quote! {
            None
        },
    };

    let utf8_token_tree = UTF8List(embeddable_file.utf8.as_slice());
    let utf16_token_tree = UTF16List(embeddable_file.utf16.as_slice());

    quote! {
        impl foundation_nostd::embeddable::EmbeddableFile for #struct_name {
            const ROOT_DIR: &str = #project_dir_tokens;
            const SOURCE_FILE: &str = #target_file_tokens;
            const SOURCE_PATH: &str = #embedded_file_relative_path_tokens;

            /// [`DATE_MODIFIED_SINCE_UNIX_EPOC`] is the last known date-time modification
            /// date given in UNIX timestamp.
            const DATE_MODIFIED_SINCE_UNIX_EPOC: Option<i64> = #date_modified_tokens;

            /// [`MIME_TYPE`] is the suggested mime-type for the file based on
            /// the extension of the source file.
            const MIME_TYPE: Option<&str> = #mime_type;

            /// [`ETAG`] provides a safe web-related e-tag value for use in web APIs.
            /// It is really just the [Self::HASH`] enclosed in double quotes.
            const ETAG: &str = #etag_tokens;

            /// [`HASH`] is the SHA-265 encoded content of the file.
            const HASH: &str = #hash_tokens;

            /// [`UTF8`] provides the utf-8 byte slices of the file as is
            /// read from file which uses the endiancess of the native system
            /// when compiled by rust.
            const UTF8: &[u8] = #utf8_token_tree;

            /// [`UTF16`] provides the utf-16 byte slices of the file as is
            /// read from file which uses the endiancess of the native system
            /// when compiled by rust.
            const UTF16: &[u16] = #utf16_token_tree;
        }
    }
}

struct UTF16List<'a>(&'a [u16]);

impl ToTokens for UTF16List<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut items = TokenStream::new();
        for item in self.0.iter() {
            items.extend(iter::once(TokenTree::from(Literal::u16_unsuffixed(*item))));
            items.extend(iter::once(TokenTree::from(Punct::new(',', Spacing::Joint))));
        }
        tokens.extend(quote! {
            &[#items]
        });
    }
}

struct UTF8List<'a>(&'a [u8]);

impl ToTokens for UTF8List<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut items = TokenStream::new();
        for item in self.0.iter() {
            items.extend(iter::once(TokenTree::from(Literal::u8_unsuffixed(*item))));
            items.extend(iter::once(TokenTree::from(Punct::new(',', Spacing::Joint))));
        }
        tokens.extend(quote! {
            &[#items]
        });
    }
}

struct EmbeddableFile {
    pub utf8: Vec<u8>,
    pub utf16: Vec<u16>,
    pub hash: String,
    pub etag: String,
    pub mime_type: Option<String>,
    pub date_modified: Option<i64>,
}

fn get_file(target_file: PathBuf) -> Result<EmbeddableFile, GenError> {
    let mut file = fs::File::open(&target_file).map_err(|err| GenError::Any(Box::new(err)))?;

    let mut file_content = String::new();
    file.read_to_string(&mut file_content)
        .map_err(|err| GenError::Any(Box::new(err)))?;

    let file_content_hash = generate_hash(&file_content);
    let file_content_etag = format!("\"{}\"", &file_content_hash);
    let file_content_as_utf8 = file_content.clone().into_bytes();
    let file_content_as_utf16 = file_content.encode_utf16().collect::<Vec<u16>>();

    let file_metadata = file
        .metadata()
        .map_err(|err| GenError::Any(Box::new(err)))?;

    let file_mime_type = MimeGuess::from_path(&target_file)
        .first()
        .map(|v| v.to_string());
    let date_modified = modified_unix_timestamp(&file_metadata);

    Ok(EmbeddableFile {
        date_modified,
        etag: file_content_etag,
        hash: file_content_hash,
        mime_type: file_mime_type,
        utf8: file_content_as_utf8,
        utf16: file_content_as_utf16,
    })
}

fn generate_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let hash = hasher.finalize();
    base85rs::encode(&hash[..])
}

fn modified_unix_timestamp(metadata: &std::fs::Metadata) -> Option<i64> {
    metadata.modified().ok().and_then(|modified| {
        modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|v| v.as_secs().try_into().ok())
            .or_else(|| {
                SystemTime::UNIX_EPOCH
                    .duration_since(modified)
                    .ok()
                    .and_then(|v| v.as_secs().try_into().ok().map(|v: i64| -v))
            })
    })
}
