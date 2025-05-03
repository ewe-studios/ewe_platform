extern crate proc_macro;
use new_mime_guess::MimeGuess;
use quote::quote;
use sha2::{Digest, Sha256};
use std::io::prelude::*;
use std::time::SystemTime;
use std::{
    env, error, fs,
    path::{Path, PathBuf},
};
use syn::Data;
use syn::{parse_macro_input, Expr, Lit, Meta};

use proc_macro::TokenStream;
use proc_macro2::Literal;

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

pub fn embed_file_on_struct(item: TokenStream) -> TokenStream {
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

    impl_embeddable_file(&ast.ident, file_path)
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

fn impl_embeddable_file(struct_name: &syn::Ident, target_file: String) -> TokenStream {
    let cargo_manifest_dir_env =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let manifest_dir = Path::new(&cargo_manifest_dir_env);

    let embed_file_path = manifest_dir.join(target_file.as_str());
    println!("EmbeddedFilePath:: {:?}", &embed_file_path);

    let embeddable_file = get_file(embed_file_path).expect("Failed to generate file embeddings");

    let etag_tokens = Literal::string(embeddable_file.etag.as_str());
    let hash_tokens = Literal::string(embeddable_file.hash.as_str());

    let mime_type = match embeddable_file.mime_type {
        Some(inner) => quote! {
            Some(#inner)
        },
        None => quote! {
            None
        },
    };

    quote! {
        impl #struct_name {
            const ETAG: &'static str = #etag_tokens;
            const HASH: &'static str = #hash_tokens;

            pub fn mime_type() -> core::option::Option<&'static str> {
                #mime_type
            }

            pub fn as_bytes(&self) -> &[u8] {
                &[]
            }
        }
    }
    .into()
}

struct EmbeddableFile {
    pub path: PathBuf,
    pub utf8: Vec<u8>,
    pub utf16: Vec<u16>,
    pub hash: String,
    pub etag: String,
    pub date_modified: i64,
    pub mime_type: Option<String>,
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
    let date_modified =
        modified_unix_timestamp(&file_metadata).ok_or_else(|| GenError::UnableToGetModifiedDate)?;

    Ok(EmbeddableFile {
        date_modified,
        path: target_file,
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
