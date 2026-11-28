// Implements project generation via a Rust Embed provider that contains
// all the necessary templates, files and directories required to be generated
// for a project.

use core::str;
use foundation_core::extensions::strings_ext::TryIntoString;
use foundation_nostd::embeddable::EmbeddableDirectory;
use std::{collections::HashSet, io::Write, path::PathBuf, sync};

use crate::{error::BoxedError, FileContent, FileSystemCommand};

// -- Rust Embed wrapper methods and constructor

pub trait PackageDirectorate {
    /// Returns the underlying content of a file.
    fn get_file(&self, target_file: &str) -> Option<Vec<u8>>;

    /// Returns all filenames in directorate.
    fn files(&self) -> Vec<String>;

    /// Returns all filenames for giving root directory.
    fn files_for(&self, directory: &str) -> Option<Vec<String>>;

    /// Returns all filenames for giving root directory as a jinja
    /// Environment object which can be rendered out from.
    fn jinja_for<'a>(&self, directory: &str) -> Option<minijinja::Environment<'a>>;

    /// Returns all top-level directories within package.
    fn root_directories(&self) -> Vec<String>;

    /// `list_all` returns all the files within the package directorate as a Vec<String>.
    fn list_all(&self) -> Vec<String>;
}

// Keep the provider key for data access and use the slash name for template lookup.
struct LogicalEntry {
    name: String,
    source_path: &'static str,
    name_from_parent: Option<String>,
    is_directory: bool,
}

fn logical_name(name: &str) -> Option<String> {
    if name.contains([':', '\0'])
        || name
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return None;
    }
    Some(name.replace('\\', "/"))
}

/// Lists the logical names in an embedded package.
/// A package with an invalid or duplicate name has no usable entries.
pub struct Directorate<T: EmbeddableDirectory + Default> {
    store: T,
    entries: Vec<LogicalEntry>,
}

impl<T: EmbeddableDirectory + Default> Directorate<T> {
    fn logical_entries(store: &T) -> Option<Vec<LogicalEntry>> {
        let mut names = HashSet::new();
        store
            .info_iter()
            .map(|item| {
                let name = if item.is_directory && item.source_path.is_empty() {
                    String::new()
                } else {
                    logical_name(item.source_path)?
                };
                if !names.insert(name.clone()) {
                    return None;
                }
                let name_from_parent =
                    if item.is_directory || item.source_path_from_parent.is_empty() {
                        None
                    } else {
                        let alias = logical_name(item.source_path_from_parent)?;
                        if alias != name && !names.insert(alias.clone()) {
                            return None;
                        }
                        Some(alias)
                    };
                Some(LogicalEntry {
                    name,
                    source_path: item.source_path,
                    name_from_parent,
                    is_directory: item.is_directory,
                })
            })
            .collect()
    }

    fn files_under(&self, directory: &str) -> Option<impl Iterator<Item = &LogicalEntry>> {
        let prefix = logical_name(directory.trim_end_matches(['/', '\\']))? + "/";
        Some(
            self.entries
                .iter()
                .filter(move |entry| !entry.is_directory && entry.name.starts_with(&prefix)),
        )
    }
}

impl<T: EmbeddableDirectory + Default> Default for Directorate<T> {
    fn default() -> Self {
        let store = T::default();
        let entries = Self::logical_entries(&store).unwrap_or_default();
        Self { store, entries }
    }
}

impl<T: EmbeddableDirectory + Default> PackageDirectorate for Directorate<T> {
    fn get_file(&self, target_file: &str) -> Option<Vec<u8>> {
        let name = logical_name(target_file)?;
        let entry = self.entries.iter().find(|entry| {
            !entry.is_directory
                && (entry.name == name || entry.name_from_parent.as_ref() == Some(&name))
        })?;
        self.store.read_utf8_for(entry.source_path)
    }

    fn files(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn root_directories(&self) -> Vec<String> {
        let mut directories: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| entry.is_directory && !entry.name.is_empty())
            .filter_map(|entry| entry.name.split('/').next().map(String::from))
            .collect();
        directories.sort_unstable();
        directories.dedup();
        directories
    }

    fn list_all(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn files_for(&self, directory: &str) -> Option<Vec<String>> {
        let files: Vec<_> = self
            .files_under(directory)?
            .map(|entry| entry.name.clone())
            .collect();
        (!files.is_empty()).then_some(files)
    }

    fn jinja_for<'a>(&self, directory: &str) -> Option<minijinja::Environment<'a>> {
        let mut files = self.files_under(directory)?.peekable();
        files.peek()?;
        let mut jinja_env = minijinja::Environment::new();
        for entry in files {
            let bytes = self.store.read_utf8_for(entry.source_path)?;
            let Ok(source) = String::from_utf8(bytes) else {
                continue;
            };
            jinja_env
                .add_template_owned(entry.name.clone(), source)
                .expect("should store template");
        }
        Some(jinja_env)
    }
}

/// [`PackageConfig`] defines underlying default configuration that
/// the [`PackageGenerator`] will use in it's underlying behavior of
/// outputting the final package out.
///
/// It might also be wrapped by Higher Level [`PackageConfigurator`]s that
/// might do custom things like rust [`PackageConfigurator`].
#[derive(Clone, Debug)]
pub struct PackageConfig {
    pub params: serde_json::Map<String, serde_json::Value>,
    pub output_directory: PathBuf,
    pub template_name: String,
    pub package_name: String,
}

impl PackageConfig {
    pub fn new<S>(
        output_directory: PathBuf,
        params: serde_json::Map<String, serde_json::Value>,
        template_name: S,
        package_name: S,
    ) -> Self
    where
        S: Into<String>,
    {
        Self {
            params,
            template_name: template_name.into(),
            package_name: package_name.into(),
            output_directory,
        }
    }
}

/// [`PackageConfigurator`] defines the underlying expectation the
/// [`PackageGenerator`] expects when it receives a target configuration
/// like the target output directory, custom parameters to apply to
/// generated files in cases of templates and what target template name
/// representing the Project template to be used in package generation.
pub trait PackageConfigurator {
    /// Returns the target directory where the output is written
    /// i.e the actual end project directory (`output_directory` / `package_name`).
    fn directory(&self) -> std::path::PathBuf;
    fn config(&self) -> PackageConfig;
    fn params(&self) -> serde_json::Map<String, serde_json::Value>;
    /// Applies the final changes after package generation.
    ///
    /// # Errors
    ///
    /// Returns an error when the final step cannot update the project files.
    fn finalize(&self) -> std::result::Result<(), BoxedError>;
}

impl PackageConfigurator for Box<dyn PackageConfigurator> {
    fn directory(&self) -> std::path::PathBuf {
        (**self).directory()
    }

    fn config(&self) -> PackageConfig {
        // We do NOT want to do this!
        // self.look()

        // That would recursively call *this* function!
        // We need to call `<dyn Trait as Trait>::look`.

        // Any of the below forms work, it depends on
        // how explicit you want to be.

        // Very explicit
        // <dyn Trait as Trait>::look(&**self)

        // Yay auto-deref for function parameters
        // <dyn Trait>::look(self)

        // Very succinct and a "makes sense once you've
        // seen it enough times" form.  The first deref
        // is for the reference (`&Self`) and the second
        // deref is for the `Box<_>`.
        (**self).config()
    }

    fn params(&self) -> serde_json::Map<String, serde_json::Value> {
        (**self).params()
    }

    fn finalize(&self) -> std::result::Result<(), BoxedError> {
        (**self).finalize()
    }
}

impl PackageConfigurator for PackageConfig {
    fn directory(&self) -> std::path::PathBuf {
        self.output_directory.join(self.package_name.clone())
    }

    fn config(&self) -> PackageConfig {
        self.clone()
    }

    fn params(&self) -> serde_json::Map<String, serde_json::Value> {
        self.params.clone()
    }

    fn finalize(&self) -> std::result::Result<(), BoxedError> {
        Ok(())
    }
}

pub struct RustConfig {
    workspace_cargo: Option<PathBuf>,
    retain_lib_section: bool,
}

impl RustConfig {
    #[must_use]
    pub fn new(workspace_cargo: Option<PathBuf>, retain_lib_section: bool) -> Self {
        Self {
            workspace_cargo,
            retain_lib_section,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn standard(workspace_cargo: Option<PathBuf>) -> Self {
        Self::new(workspace_cargo, false)
    }
}

pub struct RustProjectConfigurator {
    pub package_config: PackageConfig,
    pub rust_config: Option<RustConfig>,
    pub manifest: Option<cargo_toml::Manifest>,
}

pub type RustProjectConfiguratorResult<T> = core::result::Result<T, RustProjectConfiguratorError>;

#[derive(Debug, derive_more::From)]
pub enum RustProjectConfiguratorError {
    BadCargoManifest(std::path::PathBuf),
    #[from(ignore)]
    NoCargoFile(std::path::PathBuf),
    BadRustWorkspace,
    BadRustProject,
}

impl core::error::Error for RustProjectConfiguratorError {}

impl core::fmt::Display for RustProjectConfiguratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl RustProjectConfigurator {
    /// Generates a new `RustProjectConfigurator` which generates relevant metadata information
    /// about the rust workspace environment available to it if present via the `rust_config`
    /// parameter.
    ///
    /// This allows us take into account the fact we will be generating a new project
    /// template in an existing rust workspace project, else assume we are generating a
    /// standalone rust project when we replicate the relevant template into the
    /// codebase.
    ///
    /// # Errors
    ///
    /// Returns [`RustProjectConfiguratorError::BadRustWorkspace`] when the workspace
    /// manifest cannot be read.
    pub fn new(
        package_config: PackageConfig,
        rust_config: Option<RustConfig>,
    ) -> RustProjectConfiguratorResult<Self> {
        Self {
            package_config,
            rust_config,
            manifest: None,
        }
        .init()
    }

    fn init(mut self) -> RustProjectConfiguratorResult<Self> {
        if let Some(rust_config) = &self.rust_config {
            if let Some(workspace_cargo) = &rust_config.workspace_cargo {
                let manifest =
                    cargo_toml::Manifest::from_path(workspace_cargo.clone()).map_err(|err| {
                        tracing::error!(
                            "Failed to get cargo_toml::Manifest({:}) due to: {:?}",
                            workspace_cargo.display(),
                            err
                        );
                        RustProjectConfiguratorError::BadRustWorkspace
                    })?;

                self.manifest = Some(manifest);
            }
        }
        Ok(self)
    }

    fn output_directory_name(&self) -> String {
        String::from(
            self.package_config
                .output_directory
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
        )
    }
}

impl PackageConfigurator for RustProjectConfigurator {
    fn config(&self) -> PackageConfig {
        self.package_config.clone()
    }

    fn directory(&self) -> std::path::PathBuf {
        self.package_config.directory()
    }

    #[doc = r"params returns a modified `serde_json::Map` where standard
package information are added into the dictionary which are
accessible in the supported template language (minijinja):

When a workspace:

- `IS_WORKSPACE=true`
- `PACKAGE_NAME` (name of package being generated)
- `TEMPLATE_NAME` (name of template used)
- `ROOT_PACKAGE_LICENSE_FILE`
- `ROOT_PACKAGE_EDITION`
- `ROOT_PACKAGE_REPOSITORY`
- `ROOT_PACKAGE_VERSION`
- `ROOT_PACKAGE_RUST_VERSION`
- `ROOT_PACKAGE_RUST_LICENSE`
- `ROOT_PACKAGE_RUST_DESCRIPTION`
- `ROOT_PACKAGE_RUST_AUTHORS`
- `ROOT_PACKAGE_RUST_KEYWORDS`

When not a workspace:

- `IS_WORKSPACE=false`
- `PACKAGE_NAME` (name of package being generated)
- `TEMPLATE_NAME` (name of template used)
- `ROOT_PACKAGE_NAME`
- `ROOT_PACKAGE_LICENSE_FILE`
- `ROOT_PACKAGE_EDITION`
- `ROOT_PACKAGE_REPOSITORY`
- `ROOT_PACKAGE_VERSION`
- `ROOT_PACKAGE_RUST_VERSION`
- `ROOT_PACKAGE_RUST_LICENSE`
- `ROOT_PACKAGE_RUST_DESCRIPTION`
- `ROOT_PACKAGE_RUST_AUTHORS`
- `ROOT_PACKAGE_RUST_KEYWORDS`
"]
    fn params(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut params = self.package_config.params.clone();
        let output_directory_name = self.output_directory_name();

        params
            .entry("TEMPLATE_NAME")
            .or_insert(self.package_config.template_name.clone().into());

        params
            .entry("PACKAGE_NAME")
            .or_insert(self.package_config.package_name.clone().into());

        params.entry("PACKAGE_DIRECTORY").or_insert(
            self.package_config
                .directory()
                .try_into_string()
                .unwrap()
                .into(),
        );

        if let Some(manifest) = &self.manifest {
            if let Some(package) = &manifest.package {
                add_manifest_package_params(&mut params, package, &output_directory_name);
            }
            if let Some(workspace) = &manifest.workspace {
                if let Some(package) = &workspace.package {
                    add_workspace_package_params(&mut params, package, &output_directory_name);
                }
            }
        }

        params
    }

    fn finalize(&self) -> std::result::Result<(), BoxedError> {
        // update cargo workspace manifest
        if let Some(manifest) = &self.manifest {
            let mut updated_manifest = manifest.clone();
            if let Some(mut workspace) = updated_manifest.workspace.clone() {
                if !workspace
                    .members
                    .contains(&self.package_config.package_name)
                {
                    workspace
                        .members
                        .push(self.package_config.package_name.clone());
                }

                updated_manifest.workspace = Some(workspace);

                if let Some(rust_config) = &self.rust_config {
                    let serilized_manifest = toml::to_string(&updated_manifest)?;
                    if let Some(workspace_cargo) = &rust_config.workspace_cargo {
                        let mut cargo_file = std::fs::File::create(workspace_cargo.clone())?;
                        cargo_file.write_all(serilized_manifest.as_bytes())?;
                    }
                }
            }
        }

        // update projects
        let project_directory = self.directory();
        let project_cargo_file = project_directory.join("Cargo.toml");
        if !project_cargo_file.exists() {
            return Err(Box::new(RustProjectConfiguratorError::NoCargoFile(
                project_cargo_file,
            )));
        }

        match cargo_toml::Manifest::from_path(project_cargo_file.clone()).map_err(|err| {
            tracing::error!("Failed to get cargo_toml::Manifest due to: {:?}", err);
            RustProjectConfiguratorError::BadCargoManifest(project_cargo_file.clone())
        }) {
            Ok(manifest) => {
                let mut cloned_manifest = manifest.clone();

                let mut manifest_package = cloned_manifest.package().clone();

                // if the name is correct then do nothing
                if manifest_package.name == self.package_config.package_name {
                    return Ok(());
                }

                manifest_package
                    .name
                    .clone_from(&self.package_config.package_name);

                cloned_manifest.package = Some(manifest_package);

                // this will remove any [lib] configuration setup
                // this works when you do not care but if you do then might be wise to
                // consider an alternative approach .eg update the name correctly but you need to
                // be careful here, hence why i just set it to None and let rust figure that out
                // itself.
                if let Some(rust_config) = &self.rust_config {
                    if !rust_config.retain_lib_section {
                        cloned_manifest.lib = None;
                    }
                }

                let serilized_manifest = toml::to_string(&cloned_manifest)?;
                let mut cargo_file = std::fs::File::create(project_cargo_file.clone())?;

                match cargo_file.write_all(serilized_manifest.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(err) => Err(Box::new(err)),
                }
            }
            Err(err) => Err(Box::new(err)),
        }
    }
}

fn add_manifest_package_params(
    params: &mut serde_json::Map<String, serde_json::Value>,
    package: &cargo_toml::Package,
    output_directory_name: &str,
) {
    params.entry("IS_WORKSPACE").or_insert(false.into());

    params
        .entry("PACKAGE_DIRECTORY_NAME")
        .or_insert(output_directory_name.into());

    params
        .entry("ROOT_PACKAGE_DOCUMENTATION")
        .or_insert(package.documentation().unwrap_or("").into());

    params
        .entry("ROOT_PACKAGE_NAME")
        .or_insert(package.name().into());

    params
        .entry("ROOT_PACKAGE_LICENSE_FILE")
        .or_insert(package.license().unwrap_or("").into());

    params
        .entry("ROOT_PACKAGE_EDITION")
        .or_insert(format!("{:?}", package.edition()).into());

    params
        .entry("ROOT_PACKAGE_REPOSITORY")
        .or_insert(package.repository().unwrap_or("").into());

    params
        .entry("ROOT_PACKAGE_VERSION")
        .or_insert(package.version().into());

    params
        .entry("ROOT_PACKAGE_RUST_VERSION")
        .or_insert(package.rust_version().unwrap_or("").into());

    params
        .entry("ROOT_PACKAGE_LICENSE")
        .or_insert(package.license().unwrap_or("").into());

    params
        .entry("ROOT_PACKAGE_DESCRIPTIONS")
        .or_insert(package.description().unwrap_or("").into());

    params
        .entry("ROOT_PACKAGE_AUTHORS")
        .or_insert(package.authors().into());

    params
        .entry("ROOT_PACKAGE_KEYWORDS")
        .or_insert(package.keywords().into());
}

fn add_workspace_package_params(
    params: &mut serde_json::Map<String, serde_json::Value>,
    package: &cargo_toml::PackageTemplate,
    output_directory_name: &str,
) {
    params.entry("IS_WORKSPACE").or_insert(true.into());

    println!("Package name: {output_directory_name}");

    params
        .entry("ROOT_PACKAGE_NAME")
        .or_insert(output_directory_name.into());

    params
        .entry("ROOT_PACKAGE_DOCUMENTATION")
        .or_insert(package.documentation.clone().unwrap_or_default().into());
    params.entry("ROOT_PACKAGE_LICENSE_FILE").or_insert(
        package
            .license_file
            .clone()
            .unwrap_or_default()
            .try_into_string()
            .unwrap()
            .into(),
    );
    if let Some(edition) = package.edition {
        params
            .entry("ROOT_PACKAGE_EDITION")
            .or_insert(format!("{edition:?}").into());
    }
    params.entry("ROOT_PACKAGE_REPOSITORY").or_insert(
        package
            .repository
            .clone()
            .unwrap_or("".try_into_string().unwrap())
            .into(),
    );
    params.entry("ROOT_PACKAGE_VERSION").or_insert(
        package
            .rust_version
            .clone()
            .unwrap_or("".try_into_string().unwrap())
            .into(),
    );
    params.entry("ROOT_PACKAGE_RUST_VERSION").or_insert(
        package
            .rust_version
            .clone()
            .unwrap_or("".try_into_string().unwrap())
            .into(),
    );
    params.entry("ROOT_PACKAGE_LICENSE").or_insert(
        package
            .license
            .clone()
            .unwrap_or("".try_into_string().unwrap())
            .into(),
    );
    params.entry("ROOT_PACKAGE_DESCRIPTIONS").or_insert(
        package
            .description
            .clone()
            .unwrap_or("".try_into_string().unwrap())
            .into(),
    );
    params
        .entry("ROOT_PACKAGE_AUTHORS")
        .or_insert(package.authors.clone().unwrap_or(vec![]).into());
    params
        .entry("ROOT_PACKAGE_KEYWORDS")
        .or_insert(package.keywords.clone().unwrap_or(vec![]).into());
}

pub type PackageGenResult<T> = core::result::Result<T, PackageGenError>;

#[derive(Debug, derive_more::From)]
pub enum PackageGenError {
    Failed(crate::error::BoxedError),
    NoTemplateFound(String),
}

impl std::error::Error for PackageGenError {}

impl core::fmt::Display for PackageGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct PackageGenerator<T: PackageDirectorate> {
    pub templates: T,
}

#[allow(dead_code)]
impl<T: PackageDirectorate> PackageGenerator<T> {
    pub fn new(templates: T) -> Self {
        Self { templates }
    }
}

impl<T: PackageDirectorate + Default> Default for PackageGenerator<T> {
    fn default() -> Self {
        Self {
            templates: Default::default(),
        }
    }
}

impl<T: PackageDirectorate> PackageGenerator<T> {
    /// Creates the project from the template selected by the configurator.
    ///
    /// # Errors
    ///
    /// Returns [`PackageGenError::NoTemplateFound`] when the template has no files or no Jinja
    /// environment.
    /// Returns [`PackageGenError::Failed`] when writing a file or finalizing the project fails.
    ///
    /// # Panics
    ///
    /// Panics when a template path is not under the template name or has no file name.
    // The public API takes the configurator by value. Changing it to a reference
    // would break the existing callers.
    #[allow(clippy::needless_pass_by_value)]
    pub fn create<S: PackageConfigurator>(&self, configurator: S) -> PackageGenResult<()> {
        let config = configurator.config();

        let template_files_container = self.templates.files_for(config.template_name.as_str());

        tracing::debug!(
            "Project Template: `{}` with files: `{:?}`",
            config.template_name,
            template_files_container,
        );

        if template_files_container.is_none() {
            return Err(PackageGenError::NoTemplateFound(
                config.template_name.clone(),
            ));
        }

        let template_files = template_files_container.unwrap();

        tracing::debug!(
            "Project Template: `{}` with files: `{:?}` where all=`{:?}`",
            config.template_name,
            template_files,
            self.templates.list_all(),
        );

        let jinja_environment = self.templates.jinja_for(config.template_name.as_str());
        if jinja_environment.is_none() {
            return Err(PackageGenError::NoTemplateFound(
                config.template_name.clone(),
            ));
        }

        let file_templates = sync::Arc::new(jinja_environment.unwrap());

        let mut packager = crate::Templater::from(config.output_directory.clone());
        for template_file in &template_files {
            let template_file_path = PathBuf::from(template_file.as_str());
            if template_file_path.is_dir() || template_file_path.ends_with("/") {
                continue;
            }

            let rewritten_template_file_name = config
                .output_directory
                .join(config.package_name.as_str())
                .join(
                    template_file_path
                        .as_path()
                        .strip_prefix(config.template_name.as_str())
                        .unwrap_or_else(|_| {
                            panic!("expected valid starting as `{}`", config.template_name)
                        }),
                );

            let rewritten_template_dir = rewritten_template_file_name
                .parent()
                .expect("should have parent directory");

            let template_file_name =
                String::from(template_file_path.file_name().unwrap().to_str().unwrap());

            // if name starts with underscore(_) then we assume this is only a partial
            // to be reused in another file and skip adding
            if template_file_name.starts_with('_') {
                continue;
            }

            tracing::debug!(
                "Rewriting template path `{:?}` to `{:?}` (dir: {:?}",
                template_file,
                rewritten_template_file_name,
                rewritten_template_dir,
            );

            let content = if file_templates.get_template(template_file).is_ok() {
                FileContent::Jinja(template_file.clone(), file_templates.clone())
            } else {
                FileContent::Bytes(
                    self.templates
                        .get_file(template_file)
                        .ok_or_else(|| PackageGenError::NoTemplateFound(template_file.clone()))?,
                )
            };
            packager.add(FileSystemCommand::DirPath(
                PathBuf::from(rewritten_template_dir),
                vec![FileSystemCommand::File(template_file_name, content)],
            ));
        }

        packager
            .run(configurator.params())
            .map_err(|err| PackageGenError::Failed(err.into()))?;

        configurator.finalize().map_err(PackageGenError::Failed)
    }
}
