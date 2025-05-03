use proc_macro::TokenStream;

mod embedders;

#[proc_macro_derive(EmbedFileAs, attributes(source))]
pub fn embed_file_as(item: TokenStream) -> TokenStream {
    embedders::embed_file_on_struct(item)
}
