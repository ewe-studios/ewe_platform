use foundation_macros::EmbedDirectoryAs;

pub mod js_runtimes {
    use super::*;

    #[derive(EmbedDirectoryAs, Default)]
    #[source = "$CRATE/../../sdk/web/reloader"]
    pub struct AssetReloader;

    #[derive(EmbedDirectoryAs, Default)]
    #[source = "$CRATE/../../sdk/web/jsruntime"]
    pub struct AssetHostRuntimes;
}
