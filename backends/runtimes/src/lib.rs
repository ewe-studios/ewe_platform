use foundation_macros::EmbedFileAs;

pub mod js_runtimes {
    use super::*;

    #[derive(EmbedFileAs)]
    #[source = "../../assets/jsruntime/js_host_runtime.js"]
    pub struct JSHostRuntime;

    #[derive(EmbedFileAs)]
    #[source = "../../assets/jsruntime/runtime.js"]
    pub struct RuntimeCore;

    #[derive(EmbedFileAs)]
    #[source = "../../assets/jsruntime/packer.js"]
    pub struct PackerCore;
}
