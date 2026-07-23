use foundation_macros::EmbedDirectoryAs;

#[derive(EmbedDirectoryAs, Default)]
#[source = "$CRATE/sdk/web/reloader"]
pub struct AssetReloader;
