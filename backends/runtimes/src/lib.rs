use foundation_macros::EmbedFileAs;

pub mod js_runtimes {
    use super::*;

    #[derive(EmbedFileAs)]
    #[source = "../../assets/jsruntime/megatron.js"]
    pub struct JSHostRuntime;
}
