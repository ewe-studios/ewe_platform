use rust_embed::Embed;

#[derive(Embed)]
#[folder = "src/megatron/jsdom/packages/"]
#[prefix = "packages/"]
pub struct Packages;
