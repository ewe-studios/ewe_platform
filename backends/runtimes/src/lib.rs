use foundation_macros::EmbedDirectoryAs;
use foundation_macros::EmbedFileAs;

pub mod js_runtimes {
    use super::*;

    #[derive(EmbedFileAs, Default)]
    #[source = "../../assets/jsruntime/megatron.js"]
    pub struct JSHostRuntime;

    #[derive(EmbedDirectoryAs, Default)]
    #[source = "../../assets/jsruntime"]
    pub struct JSHostRuntimeAssets;
}
