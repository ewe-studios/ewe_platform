// Implements project generation via a Rust Embed provider that contains
// all the necessary templates, files and directories required to be generated
// for a project.

use core::str;
use ewe_templates::minijinja;
use foundation_core::extensions::strings_ext::{TryIntoStr, TryIntoString};
use std::{io::Write, marker::PhantomData, path::PathBuf, sync};

use crate::{error::BoxedError, FileContent, FileSystemCommand};

pub struct Directorate<T: rust_embed::RustEmbed> {
    pub _data: PhantomData<T>,
}

// -- constructor + default

impl<T: rust_embed::Embed + Default> Default for Directorate<T> {
    fn default() -> Self {
        Self {
            _data: PhantomData,
        }
    }
}

// -- Rust Embed wrapper methods and constructor

pub struct StringIterator(rust_embed::Filenames);

impl Iterator for StringIterator {
    type Item = std::borrow::Cow<'static, str>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub trait PackageDirectorate {
    /// Returns the underlying content of a file.
    fn get_file(&self, target_file: &str) -> Option<rust_embed::EmbeddedFile>;

    /// Returns all filenames in directorate.
    fn files(&self) -> StringIterator;

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

impl<T: rust_embed::Embed + 'static> From<Directorate<T>> for Box<dyn PackageDirectorate> {
    fn from(val: Directorate<T>) -> Self {
        Box::new(val)
    }
}

impl<T: rust_embed::Embed> PackageDirectorate for Directorate<T> {
    fn get_file(&self, target_file: &str) -> Option<rust_embed::EmbeddedFile> {
        T::get(target_file)
    }

    fn files(&self) -> StringIterator {
        StringIterator(T::iter())
    }

    fn root_directories(&self) -> Vec<String> {
        let mut dirs: Vec<String> = T::iter()
            .filter(|t| t.contains('/'))
            .filter_map(|t| t.split_once('/').map(|(directory, _)| String::from(directory)))
            .collect();

        // sort and de-dup
        dirs.sort();
        dirs.dedup();

        dirs
    }

    fn list_all(&self) -> Vec<String> {
        T::iter().map(String::from).collect()
    }

    fn files_for(&self, directory: &str) -> Option<Vec<String>> {
        let target_dir = if directory.ends_with('/') {
            directory
        } else {
            &format!("{directory}/")
        };

        let files: Vec<String> = T::iter()
            .filter(|t| t.starts_with(target_dir))
            .map(String::from)
            .collect();

        if files.is_empty() {
            return None;
        }

        Some(files)
    }

    fn jinja_for<'a>(&self, directory: &str) -> Option<minijinja::Environment<'a>> {
        let target_dir = if directory.ends_with('/') {
            directory
        } else {
            &format!("{directory}/")
        };

        let mut jinja_env = minijinja::Environment::new();

        let items: Vec<std::borrow::Cow<'_, str>> =
            T::iter().filter(|t| t.starts_with(target_dir)).collect();

        if items.is_empty() {
            return None;
        }

        for relevant_path in items {
            let relevant_file = T::get(&relevant_path).unwrap();
            let relevant_file_data = relevant_file.data.try_into_str().expect("should be string");
            jinja_env
                .add_template_owned(
                    relevant_path
                        .try_into_string()
                        .expect("should turn into String"),
                    relevant_file_data
                        .try_into_string()
                        .expect("convert into String"),
                )
                .expect("should store template");
        }

        Some(jinja_env)
    }
}

#[cfg(test)]
mod directorate_tests {

    use super::*;

    #[derive(rust_embed::Embed, Default)]
    #[folder = "templates/test_directory/"]
    struct Directory;

    #[test]
    fn validate_can_create_directorate_generator_safely() {
        let generator = Directorate::<Directory>::default();
        assert!(generator.get_file("README.md").is_some());
    }

    #[test]
    fn validate_can_read_top_directories() {
        let generator = Directorate::<Directory>::default();
        let directories: Vec<String> = generator.root_directories();
        assert_eq!(directories, vec! {"docs", "schema"});
    }

    #[test]
    fn validate_can_read_only_files_for_top_directory() {
        let generator = Directorate::<Directory>::default();
        let files: Option<Vec<String>> = generator.files_for("schema");
        assert_eq!(
            files.unwrap(),
            vec! {"schema/partials/partial_1.sql", "schema/schema.sql"}
        );
    }

    #[test]
    fn validate_can_read_all_directories() {
        let generator = Directorate::<Directory>::default();
        let files: Vec<String> = generator.files().map(String::from).collect();
        assert_eq!(
            files,
            vec! {"README.md", "docs/runner.sh", "elem.js", "schema/partials/partial_1.sql", "schema/schema.sql"}
        );
    }
}

/// `PackageConfig` defines underlying default configuration that
/// the `PackageGenerator` will use in it's underlying behavior of
/// outputing the final package out.
///
/// It might also be wrapped by Higher Level `PackageConfigurator`s tha
/// might do custom things like `RustPackageConfigurator`.
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

/// `PackageConfigurator` defines the underlying expectation the
/// `PackageGenerator` expects when it receives a target configuration
/// like the target output directory, custom parameters to apply to
/// generated files in cases of templates and what target template name
/// representing the Project template to be used in pacakge generation.
pub trait PackageConfigurator {
    /// Returns the target directory where the output is written
    /// i.e the actual end project directory (`output_directory` / `package_name`).
    fn directory(&self) -> std::path::PathBuf;
    fn config(&self) -> PackageConfig;
    fn params(&self) -> serde_json::Map<String, serde_json::Value>;
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
    pub fn new(workspace_cargo: Option<PathBuf>, retain_lib_section: bool) -> Self {
        Self {
            workspace_cargo,
            retain_lib_section,
        }
    }

    #[allow(dead_code)]
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
                        ewe_trace::error!("Failed to get cargo_toml::Manifest due to: {:?}", err);
                        RustProjectConfiguratorError::BadRustWorkspace
                    })?;

                self.manifest = Some(manifest);
            }
        }
        Ok(self)
    }
}

impl PackageConfigurator for RustProjectConfigurator {
    fn config(&self) -> PackageConfig {
        self.package_config.clone()
    }

    fn directory(&self) -> std::path::PathBuf {
        self.package_config.directory()
    }

    /// params returns a modified `serde_json::Map` where standard
    /// package information are added into the dictionary which are
    /// accessible in the supported template langauge (minijinja):
    ///
    /// When a workspace:
    ///
    /// - `IS_WORKSPACE=true`
    /// - `PACKAGE_NAME` (name of package being generated)
    /// - `TEMPLATE_NAME` (name of template used)
    /// - `ROOT_PACKAGE_LICENSE_FILE`
    /// - `ROOT_PACKAGE_EDITION`
    /// - `ROOT_PACKAGE_REPOSITORY`
    /// - `ROOT_PACKAGE_VERSION`
    /// - `ROOT_PACKAGE_RUST_VERSION`
    /// - `ROOT_PACKAGE_RUST_LICENSE`
    /// - `ROOT_PACKAGE_RUST_DESCRIPTION`
    /// - `ROOT_PACKAGE_RUST_AUTHORS`
    /// - `ROOT_PACKAGE_RUST_KEYWORDS`
    ///
    /// When not a workspace:
    ///
    /// - `IS_WORKSPACE=false`
    /// - `PACKAGE_NAME` (name of package being generated)
    /// - `TEMPLATE_NAME` (name of template used)
    /// - `ROOT_PACKAGE_NAME`
    /// - `ROOT_PACKAGE_LICENSE_FILE`
    /// - `ROOT_PACKAGE_EDITION`
    /// - `ROOT_PACKAGE_REPOSITORY`
    /// - `ROOT_PACKAGE_VERSION`
    /// - `ROOT_PACKAGE_RUST_VERSION`
    /// - `ROOT_PACKAGE_RUST_LICENSE`
    /// - `ROOT_PACKAGE_RUST_DESCRIPTION`
    /// - `ROOT_PACKAGE_RUST_AUTHORS`
    /// - `ROOT_PACKAGE_RUST_KEYWORDS`
    ///
    fn params(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut params = self.package_config.params.clone();

        let output_directory_name = String::from(
            self.package_config
                .output_directory
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
        );

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
            // when its not a workspace
            if let Some(package) = &manifest.package {
                params.entry("IS_WORKSPACE").or_insert(false.into());

                params
                    .entry("PACKAGE_DIRECTORY_NAME")
                    .or_insert(output_directory_name.clone().into());

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

            // when its a workspace
            if let Some(workspace) = &manifest.workspace {
                if let Some(package) = &workspace.package {
                    params.entry("IS_WORKSPACE").or_insert(true.into());

                    println!("Package name: {output_directory_name}");

                    params
                        .entry("ROOT_PACKAGE_NAME")
                        .or_insert(output_directory_name.clone().into());

                    params.entry("ROOT_PACKAGE_DOCUMENTATION").or_insert(
                        package
                            .documentation
                            .clone()
                            .unwrap_or(String::new())
                            .into(),
                    );
                    params.entry("ROOT_PACKAGE_LICENSE_FILE").or_insert(
                        package
                            .license_file
                            .clone()
                            .unwrap_or(PathBuf::new())
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
            ewe_trace::error!("Failed to get cargo_toml::Manifest due to: {:?}", err);
            RustProjectConfiguratorError::BadCargoManifest(project_cargo_file.clone())
        }) {
            Ok(manifest) => {
                let mut cloned_manifest = manifest.clone();

                let mut manifest_package = cloned_manifest.package().clone();

                // if the name is correct then do nothing
                if manifest_package.name == self.package_config.package_name {
                    return Ok(());
                }

                manifest_package.name = self.package_config.package_name.clone();

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

pub struct PackageGenerator {
    pub templates: Box<dyn PackageDirectorate>,
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

impl PackageGenerator {
    pub fn new(templates: Box<dyn PackageDirectorate>) -> Self {
        Self { templates }
    }

    /// create will begin to setup the underlying specified project
    /// defined in the provided configurator.
    pub fn create<S: PackageConfigurator>(&self, configurator: S) -> PackageGenResult<()> {
        let config = configurator.config();

        let template_files_container = self.templates.files_for(config.template_name.as_str());

        ewe_trace::debug!(
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

        ewe_trace::debug!(
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
                        .unwrap_or_else(|_| panic!("expected valid starting as `{}`", config.template_name)),
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

            ewe_trace::debug!(
                "Rewriting template path `{:?}` to `{:?}` (dir: {:?}",
                template_file,
                rewritten_template_file_name,
                rewritten_template_dir,
            );

            packager.add(FileSystemCommand::DirPath(
                PathBuf::from(rewritten_template_dir),
                vec![FileSystemCommand::File(
                    template_file_name,
                    FileContent::Jinja(template_file.clone(), file_templates.clone()),
                )],
            ));
        }

        packager
            .run(configurator.params())
            .map_err(|err| PackageGenError::Failed(err.into()))?;

        configurator
            .finalize()
            .map_err(PackageGenError::Failed)
    }
}

#[cfg(test)]
mod package_generator_tests {

    use std::fs;
    use std::path;

    use foundation_core::extensions::strings_ext::TryIntoString;
    use foundation_core::extensions::vec_ext::VecExt;

    use tracing_test::traced_test;

    use super::*;

    #[derive(rust_embed::Embed, Default)]
    #[folder = "templates/"]
    struct TemplateDefinitions;

    fn list_dir(target_path: &path::Path) -> Vec<String> {
        fs::read_dir(target_path)
            .expect("directory should exists")
            .map(|entry| entry.unwrap())
            .flat_map(|entry| {
                if entry.file_type().unwrap().is_dir() {
                    return list_dir(&entry.path());
                }

                vec![entry.path().try_into_string().unwrap()]
            })
            .collect()
    }

    fn shorten_path(target: Vec<String>, path: String) -> Vec<String> {
        target
            .iter()
            .map(|value| value.replace(path.as_str(), "").replacen('/', "", 1))
            .collect()
    }

    #[test]
    #[traced_test]
    fn package_generator_can_create_package_for_standard() {
        let template_directories = Box::new(Directorate::<TemplateDefinitions>::default());
        let packager = PackageGenerator::new(template_directories);

        let current_dir = std::env::current_dir().expect("should have gotten directory");

        let output_directory = current_dir.join("output_directory");
        let project_directory = output_directory.join("standard_project");
        let project_cargo_file = project_directory.join("Cargo.toml");

        let mut params: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        params
            .entry(String::from("PROJECT_DIRECTORY"))
            .or_insert(serde_json::Value::from(
                project_directory
                    .try_into_string()
                    .expect("should convert into string"),
            ));

        let rust_config = RustConfig::new(Some(project_cargo_file), false);
        let package_config = PackageConfig::new(
            project_directory.clone(),
            params,
            "CustomRustProject",
            "retro_project",
        );

        let rust_configurator = RustProjectConfigurator::new(package_config, Some(rust_config))
            .expect("should generate rust configurator");

        let result = packager.create(rust_configurator);
        ewe_trace::debug!("Result: {:?}", result);
        assert!(matches!(result, Ok(())));

        assert_eq!(
            shorten_path(
                list_dir(&project_directory),
                project_directory.try_into_string().unwrap()
            ),
            vec![
                "Cargo.toml",
                ".gitignore",
                "retro_project/src/lib.rs",
                "retro_project/src/page.rs",
                "retro_project/Cargo.toml",
            ]
            .to_vec_string()
        );
    }

    #[test]
    #[traced_test]
    fn package_generator_can_create_package_for_workspace() {
        let template_directories = Box::new(Directorate::<TemplateDefinitions>::default());
        let packager = PackageGenerator::new(template_directories);

        let current_dir = std::env::current_dir().expect("should have gotten directory");

        let output_directory = current_dir.join("output_directory");
        let project_directory = output_directory.join("workspace_project");
        let project_cargo_file = project_directory.join("Cargo.toml");

        let mut params: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        params
            .entry(String::from("PROJECT_DIRECTORY"))
            .or_insert(serde_json::Value::from(
                project_directory
                    .try_into_string()
                    .expect("should convert into string"),
            ));

        let rust_config = RustConfig::new(Some(project_cargo_file), false);
        let package_config = PackageConfig::new(
            project_directory.clone(),
            params,
            "CustomRustProject",
            "retro_project",
        );

        let rust_configurator = RustProjectConfigurator::new(package_config, Some(rust_config))
            .expect("should generate rust configurator");

        let result = packager.create(rust_configurator);
        ewe_trace::debug!("Result: {:?}", result);
        assert!(matches!(result, Ok(())));

        assert_eq!(
            shorten_path(
                list_dir(&project_directory),
                project_directory.try_into_string().unwrap()
            ),
            vec![
                "Cargo.toml",
                ".gitignore",
                "retro_project/src/lib.rs",
                "retro_project/src/page.rs",
                "retro_project/Cargo.toml",
            ]
            .to_vec_string()
        );
    }

    #[test]
    #[traced_test]
    fn package_generator_can_create_non_rust_package_from_template() {
        let template_directories = Box::new(Directorate::<TemplateDefinitions>::default());
        let packager = PackageGenerator::new(template_directories);

        let current_dir = std::env::current_dir().expect("should have gotten directory");

        let output_directory = current_dir.join("output_directory");
        let project_directory = output_directory.join("static_pages");

        let mut params: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        params
            .entry(String::from("PROJECT_DIRECTORY"))
            .or_insert(serde_json::Value::from(
                project_directory
                    .try_into_string()
                    .expect("should convert into string"),
            ));

        let package_config = PackageConfig::new(
            project_directory.clone(),
            params,
            "SimpleHTMLPage",
            "retro_project",
        );

        assert!(matches!(packager.create(package_config), Ok(())));

        assert_eq!(
            shorten_path(
                list_dir(&project_directory),
                project_directory.clone().try_into_string().unwrap()
            ),
            vec!["retro_project/index.html"].to_vec_string()
        );
    }
}
