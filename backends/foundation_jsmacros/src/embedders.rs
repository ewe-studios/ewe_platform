extern crate proc_macro;
use proc_macro::TokenStream;

pub fn embed_file_on_struct(attrs: TokenStream, item: TokenStream) -> TokenStream {
    println!("attr: \"{attrs}\"");
    println!("item: \"{item}\"");
    item
}
