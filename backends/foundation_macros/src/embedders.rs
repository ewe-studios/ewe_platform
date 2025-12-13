extern crate proc_macro;
use flate2::write::GzEncoder;
use flate2::Compression;
use foundation_nostd::embeddable::{DataCompression, DirectoryInfo, FileInfo, FsInfo};
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
        write!(f, "{self:?}")
    }
}

pub fn embed_directory_on_struct(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(item as syn::DeriveInput);

    match &ast.data {
        Data::Struct(_) => {}
        _ => panic!("Please use the macro on a struct only"),
    };

    let is_binary = has_attr(&ast, "is_binary");
    let gzip_compression = has_attr(&ast, "gzip_compression");
    let brottli_compression = has_attr(&ast, "brottli_compression");

    if gzip_compression && brottli_compression {
        panic!("You can only use brotli or gzip compression and not both");
    }

    let compression = if gzip_compression && !brottli_compression {
        foundation_nostd::embeddable::DataCompression::GZIP
    } else if !gzip_compression && brottli_compression {
        foundation_nostd::embeddable::DataCompression::BROTTLI
    } else {
        foundation_nostd::embeddable::DataCompression::NONE
    };

    let file_path = if let Some(path_str) = get_attr(&ast, "source") {
        path_str
    } else {
        panic!("A #[path=\"...\"] is required for the #[EmbedFileAs] macro")
    };

    proc_macro::TokenStream::from(impl_embeddable_directory(
        &ast.ident,
        file_path,
        is_binary,
        compression,
    ))
}

pub fn embed_file_on_struct(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(item as syn::DeriveInput);

    match &ast.data {
        Data::Struct(_) => {}
        _ => {
            panic!("Only declaration on Struct is allowed")
        }
    };

    let is_binary = has_attr(&ast, "is_binary");
    let gzip_compression = has_attr(&ast, "gzip_compression");
    let brottli_compression = has_attr(&ast, "brottli_compression");

    if gzip_compression && brottli_compression {
        panic!("You can only use brotli or gzip compression and not both");
    }

    let compression = if gzip_compression && !brottli_compression {
        foundation_nostd::embeddable::DataCompression::GZIP
    } else if !gzip_compression && brottli_compression {
        foundation_nostd::embeddable::DataCompression::BROTTLI
    } else {
        foundation_nostd::embeddable::DataCompression::NONE
    };

    let file_path = if let Some(path_str) = get_attr(&ast, "source") {
        path_str
    } else {
        panic!("A #[path=\"...\"] is required for the #[EmbedFileAs] macro")
    };

    proc_macro::TokenStream::from(impl_embeddable_file(
        &ast.ident,
        file_path,
        is_binary,
        compression,
    ))
}

fn has_attr(ast: &syn::DeriveInput, attr_name: &str) -> bool {
    ast.attrs
        .iter()
        .filter(|value| value.path().is_ident(attr_name))
        .map(|value| &value.meta)
        .take(1)
        .next()
        .is_some()
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

fn impl_embeddable_file(
    struct_name: &syn::Ident,
    target_source: String,
    is_binary: bool,
    compression: foundation_nostd::embeddable::DataCompression,
) -> TokenStream {
    let cargo_manifest_dir_env =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let manifest_dir = Path::new(&cargo_manifest_dir_env);
    let working_dir = env::current_dir().expect("get current working directory");

    let (root_workspace, _) = find_root_cargo(manifest_dir.to_owned(), None)
        .expect("heuristically identify root workspace or crate");

    let root_workspace_str = root_workspace.to_str().unwrap_or_else(|| {
        panic!("cannot get str path for {root_workspace:?}");
    });

    let project_dir = manifest_dir
        .strip_prefix(&working_dir)
        .expect("should be from home directory");

    let target_file = if target_source.contains(CURRENT_CRATE_MATCHER) {
        target_source.replace(CURRENT_CRATE_MATCHER, &cargo_manifest_dir_env)
    } else if target_source.contains(ROOT_WORKSPACE_MATCHER) {
        target_source.replace(ROOT_WORKSPACE_MATCHER, root_workspace_str)
    } else {
        target_source
    };

    let embed_file_candidate = if target_file.starts_with("/") {
        Path::new(&target_file).to_owned()
    } else {
        manifest_dir.join(target_file.as_str())
    };

    let embed_file_path = match std::fs::canonicalize(&embed_file_candidate) {
        Ok(inner) => inner.to_owned(),
        Err(err) => {
            panic!(
                "Failed to call fs.exists on file: {:?} due to {:?}",
                &embed_file_candidate, err
            );
        }
    };

    let embedded_file_relative_path = embed_file_path
        .strip_prefix(&working_dir)
        .expect("should be from home directory");

    let embeddable_file =
        get_file(embed_file_path.clone(), is_binary).expect("Failed to generate file embeddings");

    // let target_file_abs_tokens = Literal::string(embed_file_path.as_str());
    let target_file_tokens = Literal::string(
        embed_file_path
            .file_name()
            .expect("get name of file")
            .to_str()
            .expect("unwrap as str"),
    );

    let target_file_path_tokens = Literal::string(embed_file_path.to_str().expect("unwrap as str"));

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
            Some(String::from(#inner))
        },
        None => quote! {
            None
        },
    };

    let embeddable_file_tokens = quote! {
        impl #struct_name {
            const _FILE_INFO: &'static foundation_nostd::embeddable::FileInfo = foundation_nostd::embeddable::FileInfo::create(
                None,
                String::from(#target_file_path_tokens),
                String::from(#target_file_tokens),
                String::from(#embedded_file_relative_path_tokens),
                String::from(#project_dir_tokens),
                String::from(#hash_tokens),
                String::from(#etag_tokens),
                #mime_type,
                #date_modified_tokens,
            );

        }

        impl foundation_nostd::embeddable::EmbeddableFile for #struct_name {
            fn get_info(&self) -> &foundation_nostd::embeddable::FileInfo {
                &self._FILE_INFO
            }

            fn info_for<'a>(&self, source: &'a str) -> Option<&'a foundation_nostd::embeddable::FileInfo> {
                None
            }
        }
    };

    let file_data_tokens = match compression {
        DataCompression::NONE => {
            if cfg!(debug_assertions) {
                quote! {
                    impl foundation_nostd::embeddable::FileData for #struct_name {
                        fn compression(&self) -> foundation_nostd::embeddable::DataCompression {
                            foundation_nostd::embeddable::DataCompression::NONE
                        }

                        fn read_utf8(&self) -> Option<Vec<u8>> {
                            extern crate std;

                            use std::fs::File;
                            use std::io::Read;

                            let mut handle = File::open(#target_file_tokens).expect("read target file: #target_file_tokens");
                            let mut data_bytes = vec![];
                            handle.read_to_end(&mut data_bytes).expect("should have read file bytes");

                            Some(data_bytes)
                        }

                        fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }

                        fn read_utf16(&self) -> Option<Vec<u8>> {
                            extern crate std;

                            use std::fs::File;
                            use std::io::Read;

                            let mut handle = File::open(#target_file_tokens).expect("read target file: #target_file_tokens");
                            let mut data_string = String::new();
                            handle.read_to_string(&mut data_string).expect("should have read file bytes");

                            Some(data_string.encode_utf16().flat_map(|u| u.to_le_bytes()).collect())
                        }

                        fn read_utf16_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }
                    }
                }
            } else {
                let utf8_token_tree = UTF8List(embeddable_file.data.as_slice());
                let utf16_token_tree = embeddable_file
                    .data_utf16
                    .map(UTF8Vec)
                    .map_or(quote! {None}, |v| quote! { Some(#v)});

                quote! {
                    impl #struct_name {
                        /// [`UTF8`] provides the utf-8 byte slices of the file as is
                        /// read from file which uses the endiancess of the native system
                        /// when compiled by rust.
                        const _DATA_U8: &'static [u8] = #utf8_token_tree;

                        /// [`UTF16`] provides the utf-16 byte slices of the file as is
                        /// read from file which uses the endiancess of the native system
                        /// when compiled by rust.
                        const _DATA_UTF16: Option<&'static [u8]> = #utf16_token_tree;
                    }

                    impl foundation_nostd::embeddable::FileData for #struct_name {
                        fn compression(&self) -> foundation_nostd::embeddable::DataCompression {
                            foundation_nostd::embeddable::DataCompression::NONE
                        }

                        fn read_utf8(&self) -> Option<Vec<u8>> {
                            let mut data: Vec<u8> = Vec::with_capacity(Self::_DATA_U8.len());
                            data.extend_from_slice(Self::_DATA_U8);
                            Some(data)
                        }

                        fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }

                        fn read_utf16(&self) -> Option<Vec<u8>> {
                            if Self::_DATA_UTF16.is_some() {
                                let mut data: Vec<u16> = Vec::with_capacity(Self::_DATA_U16.len());
                                data.extend_from_slice(Self::_DATA_U16);
                                return Some(data);
                            }
                            None
                        }

                        fn read_utf16_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }
                    }
                }
            }
        }
        DataCompression::GZIP => {
            if cfg!(debug_assertions) {
                quote! {
                    impl foundation_nostd::embeddable::FileData for #struct_name {
                        fn compression(&self) -> foundation_nostd::embeddable::DataCompression {
                            foundation_nostd::embeddable::DataCompression::GZIP
                        }

                        fn read_utf8(&self) -> Option<Vec<u8>> {
                            extern crate std;

                            use std::fs::File;
                            use std::io::Read;
                            use std::io::Write;
                            use flate2::write::GzEncoder;
                            use flate2::Compression;

                            let mut data_bytes: Vec<u8> = vec![];

                            let mut handle = File::open(#target_file_tokens).expect("read target file: #target_file_tokens");
                            handle.read_to_end(&mut data_bytes).expect("should have read file bytes");

                            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                            encoder.write_all(data_bytes.as_slice()).expect("written data");

                            let generated = encoder.finish().expect("should finish encoding");
                            Some(generated)
                        }

                        fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }

                        fn read_utf16(&self) -> Option<Vec<u8>> {
                            extern crate std;

                            use std::fs::File;
                            use std::io::Read;
                            use std::io::Write;
                            use flate2::write::GzEncoder;
                            use flate2::Compression;

                            let mut data_string = String::new();

                            let mut handle = File::open(#target_file_tokens).expect("read target file: #target_file_tokens");
                            handle.read_to_string(&mut data_string).expect("should have read file bytes");

                            let data_utf16: Vec<u8> = data_string.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();

                            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                            encoder.write_all(data_utf16.as_slice()).expect("written data");

                            Some(encoder.finish().expect("should finish encoding"))
                        }

                        fn read_utf16_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }
                    }
                }
            } else {
                let utf8_token_tree = UTF8Vec(gzipped_vec(embeddable_file.data));
                let utf16_token_tree = embeddable_file
                    .data_utf16
                    .map(|data| UTF8Vec(gzipped_vec(data)))
                    .map_or(quote! {None}, |v| quote! { Some(#v)});

                quote! {

                    impl #struct_name {
                        /// [`UTF8`] provides the utf-8 byte slices of the file as is
                        /// read from file which uses the endiancess of the native system
                        /// when compiled by rust.
                        const _DATA_UTF8: &'static [u8] = #utf8_token_tree;

                        /// [`UTF16`] provides the utf-16 byte slices of the file as is
                        /// read from file which uses the endiancess of the native system
                        /// when compiled by rust.
                        const _DATA_UTF16: Option<&'static [u8]> = #utf16_token_tree;
                    }

                    impl foundation_nostd::embeddable::FileData for #struct_name {
                        fn compression(&self) -> foundation_nostd::embeddable::DataCompression {
                            foundation_nostd::embeddable::DataCompression::GZIP
                        }

                        fn read_utf8(&self) -> Option<Vec<u8>> {
                            let mut data: Vec<u8> = Vec::with_capacity(Self::_DATA_U8.len());
                            data.extend_from_slice(Self::_DATA_U8);
                            Some(data)
                        }

                        fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }

                        fn read_utf16(&self) -> Option<Vec<u8>> {
                            if Self::_DATA_UTF16.is_none() {
                                return None;
                            }
                            let mut data: Vec<u16> = Vec::with_capacity(Self::_DATA_UTF16.len());
                            data.extend_from_slice(Self::_DATA_UTF16);
                            Some(data)
                        }

                        fn read_utf16_for(&self, _: &str) -> Option<Vec<u16>> {
                            None
                        }
                    }
                }
            }
        }
        DataCompression::BROTTLI => {
            if cfg!(debug_assertions) {
                quote! {
                    impl foundation_nostd::embeddable::FileData for #struct_name {

                        fn compression(&self) -> foundation_nostd::embeddable::DataCompression {
                            foundation_nostd::embeddable::DataCompression::BROTTLI
                        }

                        fn read_utf8(&self) -> Option<Vec<u8>> {
                            extern crate std;

                            use std::fs::File;
                            use std::io::Read;
                            use std::io::Write;
                            use flate2::write::GzEncoder;
                            use flate2::Compression;

                            let mut data_bytes: Vec<u8> = vec![];

                            let mut handle = File::open(#target_file_tokens).expect("read target file: #target_file_tokens");
                            handle.read_to_end(&mut data_bytes).expect("should have read file bytes");

                            let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
                            writer.write_all(data_bytes.as_slice()).expect("written data");
                            writer.flush().expect("flushed data");

                            Some(writer.into_inner())
                        }

                        fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }

                        fn read_utf16(&self) -> Option<Vec<u8>> {
                            extern crate std;

                            use std::fs::File;
                            use std::io::Read;
                            use std::io::Write;
                            use flate2::write::GzEncoder;
                            use flate2::Compression;

                            let mut data_string = String::new();

                            let mut handle = File::open(#target_file_tokens).expect("read target file: #target_file_tokens");
                            handle.read_to_string(&mut data_string).expect("should have read file bytes");

                            let data_utf16: Vec<u8> = data_string.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();

                            let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
                            writer.write_all(data_utf16.as_slice()).expect("written data");
                            writer.flush().expect("flushed data");

                            Some(writer.into_inner())
                        }

                        fn read_utf16_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }
                    }
                }
            } else {
                let utf8_token_tree = UTF8Vec(brottli_vec(embeddable_file.data));
                let utf16_token_tree = embeddable_file
                    .data_utf16
                    .map(|data| UTF8Vec(brottli_vec(data)))
                    .map_or(quote! {None}, |v| quote! { Some(#v)});

                quote! {

                    impl #struct_name {
                        /// [`UTF8`] provides the utf-8 byte slices of the file as is
                        /// read from file which uses the endiancess of the native system
                        /// when compiled by rust.
                        const _DATA_UTF8: &'static [u8] = #utf8_token_tree;

                        /// [`UTF16`] provides the utf-16 byte slices of the file as is
                        /// read from file which uses the endiancess of the native system
                        /// when compiled by rust.
                        const _DATA_UTF16: Option<&'static [u8]> = #utf16_token_tree;
                    }

                    impl foundation_nostd::embeddable::FileData for #struct_name {
                        fn compression(&self) -> foundation_nostd::embeddable::DataCompression {
                            foundation_nostd::embeddable::DataCompression::BROTTLI
                        }

                        fn read_utf8(&self) -> Option<Vec<u8>> {
                            let mut data: Vec<u8> = Vec::with_capacity(Self::_DATA_U8.len());
                            data.extend_from_slice(Self::_DATA_U8);
                            Some(data)
                        }

                        fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
                            None
                        }

                        fn read_utf16(&self) -> Option<Vec<u8>> {
                            if Self::_DATA_UTF16.is_none() {
                                return None;
                            }
                            let mut data: Vec<u16> = Vec::with_capacity(Self::_DATA_UTF16.len());
                            data.extend_from_slice(Self::_DATA_UTF16);
                            Some(data)
                        }

                        fn read_utf16_for(&self, _: &str) -> Option<Vec<u16>> {
                            None
                        }
                    }

                }
            }
        }
    };

    quote! {
        #embeddable_file_tokens

        #file_data_tokens

    }
}

fn impl_embeddable_directory(
    struct_name: &syn::Ident,
    target_source: String,
    is_binary: bool,
    compression: foundation_nostd::embeddable::DataCompression,
) -> TokenStream {
    let cargo_manifest_dir_env =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let manifest_dir = Path::new(&cargo_manifest_dir_env);
    let working_dir = env::current_dir().expect("get current working directory");

    let (root_workspace, _) = find_root_cargo(manifest_dir.to_owned(), None)
        .expect("heuristically identify root workspace or crate");

    let root_workspace_str = root_workspace.to_str().unwrap_or_else(|| {
        panic!("cannot get str path for {root_workspace:?}");
    });

    let project_dir = manifest_dir
        .strip_prefix(&working_dir)
        .expect("should be from home directory");

    let target_directory = if target_source.contains(CURRENT_CRATE_MATCHER) {
        target_source.replace(CURRENT_CRATE_MATCHER, &cargo_manifest_dir_env)
    } else if target_source.contains(ROOT_WORKSPACE_MATCHER) {
        target_source.replace(ROOT_WORKSPACE_MATCHER, root_workspace_str)
    } else {
        target_source
    };

    let embed_directory_candidate = if target_directory.starts_with("/") {
        Path::new(&target_directory).to_owned()
    } else {
        manifest_dir.join(target_directory.as_str())
    };

    let embed_directory_path = match std::fs::canonicalize(&embed_directory_candidate) {
        Ok(inner) => inner.to_owned(),
        Err(err) => {
            panic!(
                "Failed to call fs.exists on file: {:?} due to {:?}",
                &embed_directory_candidate, err
            );
        }
    };

    let embedded_file_relative_path = embed_directory_path
        .strip_prefix(&working_dir)
        .expect("should be from home directory");

    let embedded_file_relative_path_tokens =
        Literal::string(embed_directory_path.to_str().expect("unwrap as str"));

    // let embeddable_file = get_file(embed_directory_path.clone(), is_binary)
    //     .expect("Failed to generate file embeddings");
    //
    // // let target_file_abs_tokens = Literal::string(embed_file_path.as_str());
    // let target_file_tokens = Literal::string(
    //     embed_directory_path
    //         .file_name()
    //         .expect("get name of file")
    //         .to_str()
    //         .expect("unwrap as str"),
    // );

    todo!()
}

fn visit_dirs(collected: &mut Vec<FsInfo>, dir: &Path, root_dir: Option<&Path>, index: usize) {
    if dir.is_dir() {
        let dir_path_string = String::from(dir.to_str().expect("get strting"));
        let dir_name = get_file_name(dir.to_path_buf());
        let root_dir_parent = root_dir.map(|v| String::from(v.to_str().unwrap()));
        let dir_date_modified =
            get_file_modified_date(dir.to_path_buf()).expect("get modified date");

        collected.push(FsInfo::Dir(DirectoryInfo {
            dir_name,
            index: Some(index),
            root_dir: root_dir_parent,
            date_modified_since_unix_epoc: dir_date_modified,
        }));

        let mut current_index = index;
        for entry in fs::read_dir(dir).expect("to read path") {
            let entry = entry.expect("resolve entry");

            let date_modified = get_file_modified_date(entry.path()).expect("get modified date");
            let file_path_string = String::from(entry.path().to_str().expect("get string"));
            let file_name = get_file_name(entry.path());

            let entry_path = entry.path();

            let file_directory_relative = entry_path
                .strip_prefix(dir)
                .expect("should be able to strip root dir");

            if entry_path.is_dir() {
                current_index += 1;
                visit_dirs(collected, entry_path.as_path(), root_dir, current_index);
            } else {
                let file_relative_str =
                    String::from(file_directory_relative.to_str().expect("hello"));
                let file_hash = get_file_hash(entry.path()).expect("generate hash");
                let file_etag = format!("\"{}\"", &file_hash);
                let file_mime_type = MimeGuess::from_path(entry.path())
                    .first()
                    .map(|v| v.to_string());

                current_index += 1;
                let file_info = FileInfo::new(
                    Some(current_index),
                    file_path_string,
                    file_relative_str,
                    file_name,
                    dir_path_string.clone(),
                    file_hash,
                    file_etag,
                    file_mime_type,
                    date_modified,
                );

                collected.push(FsInfo::File(file_info));
            }
        }
    }
}

struct EmbeddableFile {
    pub data: Vec<u8>,
    pub data_utf16: Option<Vec<u8>>,
    pub hash: String,
    pub etag: String,
    pub mime_type: Option<String>,
    pub date_modified: Option<i64>,
}

fn get_file_name(target_file: PathBuf) -> String {
    target_file
        .file_name()
        .map(|value| String::from(value.to_str().expect("to create str")))
        .expect("should be string")
}

fn get_file_modified_date(target_file: PathBuf) -> Result<Option<i64>, GenError> {
    let file_metadata = target_file.metadata().expect("ensure to retrieve metadata");
    Ok(modified_unix_timestamp(&file_metadata))
}

fn get_file_hash(target_file: PathBuf) -> Result<String, GenError> {
    let mut file = fs::File::open(&target_file).map_err(|err| GenError::Any(Box::new(err)))?;

    let mut file_content: Vec<u8> = Vec::new();
    file.read_to_end(&mut file_content)
        .map_err(|err| GenError::Any(Box::new(err)))?;

    Ok(generate_hash(&file_content))
}

fn get_file(target_file: PathBuf, is_binary: bool) -> Result<EmbeddableFile, GenError> {
    let mut file = fs::File::open(&target_file).map_err(|err| GenError::Any(Box::new(err)))?;

    let mut file_content: Vec<u8> = Vec::new();
    file.read_to_end(&mut file_content)
        .map_err(|err| GenError::Any(Box::new(err)))?;

    let mut file_content_utf16: Option<Vec<u8>> = None;
    if !is_binary {
        let mut file_content_string = String::new();
        file.seek(std::io::SeekFrom::Start(0))
            .expect("should seek to start");
        file.read_to_string(&mut file_content_string)
            .map_err(|err| GenError::Any(Box::new(err)))?;

        file_content_utf16 = Some(
            file_content_string
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes())
                .collect(),
        );
    }

    let file_content_hash = generate_hash(&file_content);
    let file_content_etag = format!("\"{}\"", &file_content_hash);

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
        data: file_content,
        data_utf16: file_content_utf16,
    })
}

fn generate_hash(content: &[u8]) -> String {
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

struct UTF8Vec(Vec<u8>);

impl ToTokens for UTF8Vec {
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

fn gzipped_vec(data: Vec<u8>) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data.as_slice()).expect("written data");
    encoder.finish().expect("should finish encoding")
}

fn brottli_vec(data: Vec<u8>) -> Vec<u8> {
    let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
    writer.write_all(data.as_slice()).expect("written data");
    writer.flush().expect("flushed data");
    writer.into_inner()
}
