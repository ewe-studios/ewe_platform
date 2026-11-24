//! Read the image policy and resolve virtual machine layer overrides.
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize)]
pub(super) struct Config {
    pub registry: Registry,
    pub base: Base,
    pub image: Image,
    #[serde(default)]
    pub vm: BTreeMap<String, Vm>,
}
#[derive(Deserialize)]
pub(super) struct Registry {
    #[serde(default = "docker_hub")]
    pub host: String,
    pub namespace: String,
    pub username: String,
}
fn docker_hub() -> String {
    "docker.io".into()
}
#[derive(Deserialize)]
pub(super) struct Base {
    pub name: String,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub browsers: Vec<String>,
    pub flavors: BTreeMap<String, Flavor>,
}
#[derive(Deserialize)]
pub(super) struct Flavor {
    pub from: String,
}
#[derive(Deserialize)]
pub(super) struct Image {
    pub name: String,
    pub dockerfile: PathBuf,
    #[serde(flatten)]
    pub inputs: Inputs,
    #[serde(default)]
    pub cargo_fetch: bool,
    #[serde(default)]
    pub build_args: toml::Table,
}
#[derive(Deserialize)]
pub(super) struct Vm {
    pub name: String,
    pub base_name: String,
    pub base_from: String,
    pub dockerfile: PathBuf,
    pub layer_dir: PathBuf,
    #[serde(flatten)]
    pub inputs: Inputs,
    #[serde(default)]
    pub build_args: toml::Table,
}
#[derive(Clone, Deserialize, Default)]
pub(super) struct Inputs {
    pub files: Option<Vec<String>>,
    pub mise_config: Option<String>,
    pub mise_lock: Option<String>,
    pub version_checks: Option<Vec<VersionCheck>>,
    pub submodules: Option<Vec<String>>,
}
#[derive(Clone, Deserialize)]
pub(super) struct VersionCheck {
    pub tool: String,
    pub cargo_package: String,
}
impl Inputs {
    pub fn inherit(&self, base: &Self) -> Self {
        Self {
            files: self.files.clone().or_else(|| base.files.clone()),
            mise_config: self
                .mise_config
                .clone()
                .or_else(|| base.mise_config.clone()),
            mise_lock: self.mise_lock.clone().or_else(|| base.mise_lock.clone()),
            version_checks: self
                .version_checks
                .clone()
                .or_else(|| base.version_checks.clone()),
            submodules: self.submodules.clone().or_else(|| base.submodules.clone()),
        }
    }
}
