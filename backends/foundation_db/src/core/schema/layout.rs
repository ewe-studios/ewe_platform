//! Migration groups declared by a directory tree.
//!
//! In a layout, each folder directly under the root is one group, and the
//! folder name is the group name. The folders named in
//! [`MigrationLayoutConfig::template_groups`] apply to every database, first.
//! Every other folder is the specific group of the database with the same
//! name. [`MigrationLayout::scan`] reads the tree at run time;
//! `embed_migration_layout!` reads it at build time.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use foundation_vfs::async_traits::{AsyncVfsDirectory, AsyncVfsFileSystem};
use foundation_vfs::types::VfsFileType;

use super::migrations::{
    join_path, MigrationConfig, MigrationError, MigrationProblem, MigrationRunner, MigrationSet,
    MigrationSource,
};

/// The groups found in one directory tree.
#[derive(Debug, Clone)]
pub struct MigrationLayout {
    templates: Vec<Arc<MigrationSet>>,
    specific: BTreeMap<String, Arc<MigrationSet>>,
    config: MigrationConfig,
}

/// The rules of a layout.
#[derive(Debug, Clone)]
pub struct MigrationLayoutConfig {
    /// Folders that apply to every database, in this order. Default
    /// `["shared"]`.
    pub template_groups: Vec<String>,
}

impl Default for MigrationLayoutConfig {
    fn default() -> Self {
        Self {
            template_groups: vec!["shared".to_string()],
        }
    }
}

impl MigrationLayoutConfig {
    /// Set the template folders, in apply order.
    #[must_use]
    pub fn with_template_groups(mut self, groups: Vec<String>) -> Self {
        self.template_groups = groups;
        self
    }
}

impl MigrationLayout {
    /// Read the folders under `root` through `files`, the file system that the
    /// host installs. The method checks the layout rules and the plan rules
    /// for every group.
    ///
    /// # Errors
    ///
    /// Returns one [`MigrationError`] that holds every problem.
    pub async fn scan<F: AsyncVfsFileSystem>(
        files: &F,
        root: &str,
        layout: MigrationLayoutConfig,
        config: MigrationConfig,
    ) -> Result<Self, MigrationError> {
        let mut problems = Vec::new();
        let directory = match files.open_directory_async(root.to_string()).await {
            Ok(directory) => directory,
            Err(error) => {
                return Err(MigrationError {
                    problems: vec![MigrationProblem::Io {
                        origin: root.to_string(),
                        message: error.to_string(),
                    }],
                });
            }
        };
        let entries = match directory.list_async().await {
            Ok(entries) => entries,
            Err(error) => {
                return Err(MigrationError {
                    problems: vec![MigrationProblem::Io {
                        origin: root.to_string(),
                        message: error.to_string(),
                    }],
                });
            }
        };

        let mut found: BTreeMap<String, Arc<MigrationSet>> = BTreeMap::new();
        for entry in entries {
            if entry.file_type != VfsFileType::Directory {
                continue;
            }
            let path = join_path(root, &entry.name);
            let set = MigrationSet {
                group: Cow::Owned(entry.name.clone()),
                source: MigrationSource::Directory(path.into()),
            };
            let runner = MigrationRunner::from_set(set.clone(), config.clone());
            match runner.plan(files).await {
                Ok(_) => {
                    found.insert(entry.name, Arc::new(set));
                }
                Err(mut error) => problems.append(&mut error.problems),
            }
        }

        let template_names: BTreeSet<&str> =
            layout.template_groups.iter().map(String::as_str).collect();
        for name in &layout.template_groups {
            if !found.contains_key(name) {
                problems.push(MigrationProblem::Io {
                    origin: join_path(root, name),
                    message: "template group folder not found".to_string(),
                });
            }
        }

        if !problems.is_empty() {
            return Err(MigrationError { problems });
        }

        let templates = layout
            .template_groups
            .iter()
            .filter_map(|name| found.get(name).cloned())
            .collect();
        let specific = found
            .into_iter()
            .filter(|(name, _)| !template_names.contains(name.as_str()))
            .collect();

        Ok(Self {
            templates,
            specific,
            config,
        })
    }

    /// The runner of one database: the template groups, then its own group
    /// when the layout has one.
    #[must_use]
    pub fn runner_for(&self, database: &str) -> MigrationRunner {
        let mut sets = self.templates.clone();
        if let Some(set) = self.specific.get(database) {
            sets.push(Arc::clone(set));
        }
        MigrationRunner::from_sets(sets, self.config.clone())
    }

    /// The database names that have their own folder.
    pub fn databases(&self) -> impl Iterator<Item = &str> {
        self.specific.keys().map(String::as_str)
    }

    /// The template group names, in apply order.
    #[must_use]
    pub fn template_groups(&self) -> Vec<&str> {
        self.templates
            .iter()
            .map(|set| set.group.as_ref())
            .collect()
    }
}

/// A layout embedded at build time by `embed_migration_layout!`.
#[derive(Debug)]
pub struct EmbeddedLayout {
    /// The template groups, in the order of `template_groups`.
    pub templates: &'static [&'static MigrationSet],
    /// The specific groups, one for each database folder.
    pub specific: &'static [&'static MigrationSet],
    /// The template folder names, in apply order.
    pub template_groups: &'static [&'static str],
}

impl EmbeddedLayout {
    /// Build the run-time layout. A host calls this once and reuses the value.
    #[must_use]
    pub fn layout(&self, config: MigrationConfig) -> MigrationLayout {
        let mut by_name: BTreeMap<&str, Arc<MigrationSet>> = BTreeMap::new();
        for set in self.templates.iter().chain(self.specific.iter()) {
            by_name.insert(set.group.as_ref(), Arc::new((*set).clone()));
        }
        let template_names: BTreeSet<&str> = self.template_groups.iter().copied().collect();
        let templates = self
            .template_groups
            .iter()
            .filter_map(|name| by_name.get(name).cloned())
            .collect();
        let specific = by_name
            .into_iter()
            .filter(|(name, _)| !template_names.contains(name))
            .map(|(name, set)| (name.to_string(), set))
            .collect();
        MigrationLayout {
            templates,
            specific,
            config,
        }
    }
}
