use foundation_macros::EmbedFileAs;

pub mod js_runtimes {
    use super::*;

    #[derive(EmbedFileAs, Default)]
    #[gzip_compression]
    #[source = "../../assets/jsruntime/megatron.js"]
    pub struct JSHostRuntime;
}
