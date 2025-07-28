// Types for the packages

use std::time;

use derive_more::{Debug, From};

use crate::{types::HyperFuncMap, ProxyType};

/// `ProjectDefinition` defines the underlying project location, directory path and target crate
/// we want executed in our behalf for the dev service.
#[derive(Clone, Debug, From)]
pub struct ProjectDefinition {
    pub proxy: ProxyType,
    pub target_directory: String,
    pub workspace_root: String,
    pub crate_name: String,
    pub skip_rust_checks: bool,
    pub stop_on_failure: bool,
    pub reload_directories: Vec<String>,
    pub build_directories: Vec<String>,
    pub build_arguments: Vec<String>,
    pub run_arguments: Vec<String>,
    pub wait_before_reload: time::Duration,
}

impl core::fmt::Display for ProjectDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// --  Methods

impl ProjectDefinition {
    pub fn and_proxy_routes(&mut self, mutator: impl Fn(&mut HyperFuncMap)) {
        self.proxy.and_routes(mutator);
    }
}
