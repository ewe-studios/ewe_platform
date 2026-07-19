extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use std::path::{Path, PathBuf};

use foundation_nostd::embeddable::FsInfo;

use crate::embedders::{find_root_cargo, get_file, get_target_source_path, visit_dirs};

fn get_attr(ast: &syn::DeriveInput, attr_name: &str) -> Option<String> {
    use syn::{Expr, Lit, Meta};
    let attributed: Option<&syn::Meta> = ast
        .attrs
        .iter()
        .filter(|value| value.path().is_ident(attr_name))
        .map(|value| &value.meta)
        .next();
    match attributed {
        Some(Meta::NameValue(item)) => match &item.value {
            Expr::Lit(lit) => match &lit.lit {
                Lit::Str(inner) => Some(inner.value()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

pub fn mobile_directory_on_struct(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(item as syn::DeriveInput);
    let struct_name = &ast.ident;

    if !matches!(&ast.data, syn::Data::Struct(_)) {
        panic!("MobileDirectory must be used on a struct with a `root: PathBuf` field");
    }

    let source = get_attr(&ast, "source")
        .expect("A #[source = \"...\"] attribute is required for MobileDirectory");

    proc_macro::TokenStream::from(impl_mobile_directory(struct_name, &source))
}

fn impl_mobile_directory(struct_name: &syn::Ident, target_source: &str) -> TokenStream {
    let cargo_manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let manifest_dir = Path::new(&cargo_manifest_dir);
    let working_dir = std::env::current_dir().expect("get current working directory");

    let (root_workspace, _) = find_root_cargo(manifest_dir, None)
        .expect("find root workspace");

    let root_workspace_str = root_workspace.to_str().expect("root workspace path");

    let project_dir = manifest_dir
        .strip_prefix(&working_dir)
        .expect("project dir relative to cwd");

    let target_directory =
        get_target_source_path(&cargo_manifest_dir, root_workspace_str, target_source);

    let embed_directory_candidate = if target_directory.starts_with('/') {
        Path::new(&target_directory).to_owned()
    } else {
        manifest_dir.join(&target_directory)
    };

    let embed_directory_path = std::fs::canonicalize(&embed_directory_candidate)
        .unwrap_or_else(|e| {
            panic!(
                "MobileDirectory: failed to resolve source '{}': {:?}",
                embed_directory_candidate.display(),
                e
            )
        });

    let mut collected_entries: Vec<FsInfo> = Vec::new();
    visit_dirs(
        &mut collected_entries,
        &embed_directory_path,
        &embed_directory_path,
        0,
    );

    let file_meta_list: Vec<proc_macro2::TokenStream> = collected_entries
        .iter()
        .map(|item| match item {
            FsInfo::File(info) => {
                let source_file_path = PathBuf::from(&info.source_file_path);
                let file_data = get_file(&source_file_path, false)
                    .expect("Failed to read file for metadata");
                let file_index = syn::LitInt::new(
                    &info.index.expect("should have index").to_string(),
                    proc_macro2::Span::call_site(),
                );
                let file_name = syn::LitStr::new(&info.source_name, proc_macro2::Span::call_site());
                let file_path = syn::LitStr::new(&info.source_path, proc_macro2::Span::call_site());
                let file_path_parent =
                    syn::LitStr::new(&info.source_path_from_parent, proc_macro2::Span::call_site());
                let disk_path =
                    syn::LitStr::new(&info.source_file_path, proc_macro2::Span::call_site());
                let etag = syn::LitStr::new(&file_data.etag, proc_macro2::Span::call_site());
                let hash = syn::LitStr::new(&file_data.hash, proc_macro2::Span::call_site());
                let project = syn::LitStr::new(
                    project_dir.to_str().expect("project dir to str"),
                    proc_macro2::Span::call_site(),
                );
                let date_tokens: proc_macro2::TokenStream = if let Some(d) = file_data.date_modified {
                    quote! { Some(#d) }
                } else {
                    quote! { None }
                };
                let mime_tokens: proc_macro2::TokenStream = if let Some(m) = &file_data.mime_type {
                    let m = syn::LitStr::new(m, proc_macro2::Span::call_site());
                    quote! { Some(#m) }
                } else {
                    quote! { None }
                };

                quote! {
                    foundation_nostd::embeddable::FileInfo::create(
                        Some(#file_index),
                        #disk_path,
                        #file_name,
                        #file_path,
                        #file_path_parent,
                        #project,
                        #hash,
                        #etag,
                        #mime_tokens,
                        #date_tokens,
                        false,
                    ),
                }
            }
            FsInfo::Dir(info) => {
                let file_index = syn::LitInt::new(
                    &info.index.expect("should have index").to_string(),
                    proc_macro2::Span::call_site(),
                );
                let dir_name =
                    syn::LitStr::new(&info.dir_name, proc_macro2::Span::call_site());
                let dir_path =
                    syn::LitStr::new(&info.dir_path, proc_macro2::Span::call_site());
                let disk_path =
                    syn::LitStr::new(&info.root_dir, proc_macro2::Span::call_site());
                let project = syn::LitStr::new(
                    project_dir.to_str().expect("project dir to str"),
                    proc_macro2::Span::call_site(),
                );

                quote! {
                    foundation_nostd::embeddable::FileInfo::create(
                        Some(#file_index),
                        #disk_path,
                        #dir_name,
                        #dir_path,
                        #dir_path,
                        #project,
                        "",
                        "",
                        None,
                        None,
                        true,
                    ),
                }
            }
        })
        .collect();

    quote! {
        impl foundation_nostd::mobile::MobileDirectory for #struct_name {
            const FILES_METADATA: &'static [foundation_nostd::embeddable::FileInfo] = &[
                #(#file_meta_list)*
            ];

            fn root_str(&self) -> &str {
                self.root.as_path().to_str().expect("root path is not valid UTF-8")
            }
        }

        impl #struct_name {
            /// The runtime asset root as a `Path`.
            pub fn root(&self) -> &std::path::Path {
                self.root.as_ref()
            }
        }
    }
    .into()
}
