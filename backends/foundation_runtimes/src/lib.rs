use foundation_macros::EmbedDirectoryAs;

pub mod js_runtimes {
    use super::EmbedDirectoryAs;

    #[derive(EmbedDirectoryAs, Default)]
    #[source = "$CRATE/assets/reloader"]
    pub struct AssetReloader;

    #[derive(EmbedDirectoryAs, Default)]
    #[source = "$CRATE/assets/jsruntime"]
    pub struct AssetHostRuntimes;
}
