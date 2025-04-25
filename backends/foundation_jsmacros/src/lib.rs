use proc_macro::TokenStream;

mod embedders;

#[proc_macro_attribute]
pub fn embed_file_as(attrs: TokenStream, item: TokenStream) -> TokenStream {
    embedders::embed_file_on_struct(attrs, item)
}
