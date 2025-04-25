extern crate proc_macro;
use std::{env, path::Path};

use proc_macro::TokenStream;

pub fn embed_file_on_struct(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = Path::new(&manifest_dir).join("Cargo.toml");

    println!("attr: \"{attrs}\"");
    println!("item: \"{item}\"");
    println!("ManifestDir: \"{manifest_dir}\"");
    println!("ManifestPath: \"{manifest_path:?}\"");
    item
}
