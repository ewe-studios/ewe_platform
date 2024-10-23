// Implements project generation via a Rust Embed provider that contains
// all the necessary templates, files and directories required to be generated
// for a project.

use core::str;
use ewe_templates::minijinja;
use std::{collections::HashMap, marker::PhantomData, path::PathBuf, sync};
use strings_ext::{IntoStr, IntoString};

use crate::{FileContent, FileSystemCommand};

pub struct Directorate<T: rust_embed::RustEmbed> {
    pub _data: PhantomData<T>,
}

// -- constructor + default

impl<T: rust_embed::Embed + Default> Default for Directorate<T> {
    fn default() -> Self {
        Self {
            _data: PhantomData::default(),
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
    fn files_for(&self, directory: &str) -> Vec<String>;

    /// Returns all filenames for giving root directory as a jinja
    /// Environment object which can be rendered out from.
    fn jinja_for<'a>(&self, directory: &str) -> minijinja::Environment<'a>;

    /// Returns all top-level directories within package.
    fn root_directories(&self) -> Vec<String>;

    /// list_all returns all the files within the package directorate as a Vec<String>.
    fn list_all(&self) -> Vec<String>;
}

impl<T: rust_embed::Embed + 'static> Into<Box<dyn PackageDirectorate>> for Directorate<T> {
    fn into(self) -> Box<dyn PackageDirectorate> {
        Box::new(self)
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
            .filter(|t| t.contains("/"))
            .map(|t| match t.split_once("/") {
                None => None,
                Some((directory, _)) => Some(String::from(directory)),
            })
            .filter(|t| t.is_some())
            .map(|t| t.unwrap())
            .collect();

        // sort and de-dup
        dirs.sort();
        dirs.dedup();

        dirs
    }

    fn list_all(&self) -> Vec<String> {
        T::iter().map(|t| String::from(t)).collect()
    }

    fn files_for(&self, directory: &str) -> Vec<String> {
        let target_dir = if directory.ends_with("/") {
            directory
        } else {
            &format!("{}/", directory)
        };

        let files: Vec<String> = T::iter()
            .filter(|t| t.starts_with(target_dir))
            .map(|t| String::from(t))
            .collect();

        files
    }

    fn jinja_for<'a>(&self, directory: &str) -> minijinja::Environment<'a> {
        let target_dir = if directory.ends_with("/") {
            directory
        } else {
            &format!("{}/", directory)
        };

        let mut jinja_env = minijinja::Environment::new();

        for relevant_path in T::iter().filter(|t| t.starts_with(target_dir)).into_iter() {
            let relevant_file = T::get(&relevant_path).unwrap();
            let relevant_file_data = relevant_file.data.into_str().expect("should be string");
            jinja_env
                .add_template_owned(
                    relevant_path
                        .into_string()
                        .expect("should turn into String"),
                    relevant_file_data
                        .into_string()
                        .expect("convert into String"),
                )
                .expect("should store template");
        }

        jinja_env
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
        assert!(matches!(generator.get_file("README.md"), Some(_)));
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
        let files: Vec<String> = generator.files_for("schema");
        assert_eq!(
            files,
            vec! {"schema/partials/partial_1.sql", "schema/schema.sql"}
        );
    }

    #[test]
    fn validate_can_read_all_directories() {
        let generator = Directorate::<Directory>::default();
        let files: Vec<String> = generator.files().map(|t| String::from(t)).collect();
        assert_eq!(
            files,
            vec! {"README.md", "docs/runner.sh", "elem.js", "schema/partials/partial_1.sql", "schema/schema.sql"}
        );
    }
}

/// PackageConfig defines underlying default configuration that
/// the PackageGenerator will use in it's underlying behavior of
/// outputing the final package out.
///
/// It might also be wrapped by Higher Level `PackageConfigurator`s tha
/// might do custom things like `RustPackageConfigurator`.
#[derive(Clone, Debug)]
pub struct PackageConfig {
    pub params: HashMap<String, String>,
    pub output_directory: PathBuf,
    pub template_name: String,
    pub package_name: String,
}

impl PackageConfig {
    pub fn new<S>(
        output_directory: PathBuf,
        params: HashMap<String, String>,
        template_name: S,
        package_name: S,
    ) -> Self
    where
        S: Into<String>,
    {
        Self {
            template_name: template_name.into(),
            package_name: package_name.into(),
            output_directory,
            params,
        }
    }
}

/// PackageConfigurator defines the underlying expectation the
/// PackageGenerator expects when it receives a target configuration
/// like the target output directory, custom parameters to apply to
/// generated files in cases of templates and what target template name
/// representing the Project template to be used in pacakge generation.
pub trait PackageConfigurator {
    fn config(&self) -> PackageConfig;
    fn params(&self) -> HashMap<String, String>;
}

impl PackageConfigurator for PackageConfig {
    fn config(&self) -> PackageConfig {
        self.clone()
    }

    fn params(&self) -> HashMap<String, String> {
        self.params.clone()
    }
}

pub struct RustConfig {
    workspace_cargo: PathBuf,
}

impl RustConfig {
    #[allow(dead_code)]
    pub fn new(workspace_cargo: PathBuf) -> Self {
        Self { workspace_cargo }
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
    BadRustWorkspace,
    BadRustProject,
}

impl core::error::Error for RustProjectConfiguratorError {}

impl core::fmt::Display for RustProjectConfiguratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
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
            let manifest = cargo_toml::Manifest::from_path(rust_config.workspace_cargo.clone())
                .map_err(|err| {
                    ewe_logs::error!("Failed to get cargo_toml::Manifest due to: {:?}", err);
                    RustProjectConfiguratorError::BadRustWorkspace
                })?;

            self.manifest = Some(manifest);
        }
        Ok(self)
    }
}

impl PackageConfigurator for RustProjectConfigurator {
    fn config(&self) -> PackageConfig {
        self.package_config.clone()
    }

    fn params(&self) -> HashMap<String, String> {
        self.package_config.params.clone()
    }
}

pub struct PackageGenerator {
    pub templates: Box<dyn PackageDirectorate>,
}

pub type PackageGenResult<T> = core::result::Result<T, PackageGenError>;

pub enum PackageGenError {
    Failed(crate::error::BoxedError),
    NoTemplateFound,
}

impl PackageGenerator {
    pub fn new(templates: Box<dyn PackageDirectorate>) -> Self {
        Self { templates }
    }

    /// create will begin to setup the underlying specified project
    /// defined in the provided configurator.
    #[allow(dead_code)]
    fn create<S>(&self, configurator: S) -> PackageGenResult<()>
    where
        S: PackageConfigurator,
    {
        let config = configurator.config();

        let template_files = self.templates.files_for(config.template_name.as_str());
        ewe_logs::debug!(
            "Project Template: `{}` with files: `{:?}` where all=`{:?}`",
            config.template_name,
            template_files,
            self.templates.list_all(),
        );

        let file_templates =
            sync::Arc::new(self.templates.jinja_for(config.template_name.as_str()));

        let mut packager = crate::Templater::from(config.output_directory.clone());
        for template_file in template_files.iter() {
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
                        .expect(
                            format!("expected valid starting as `{}`", config.template_name)
                                .as_str(),
                        ),
                );

            let rewritten_template_dir = rewritten_template_file_name
                .parent()
                .expect("should have parent directory");

            let template_file_name =
                String::from(template_file_path.file_name().unwrap().to_str().unwrap());

            // if name starts with underscore(_) then we assume this is only a partial
            // to be reused in another file and skip adding
            if template_file_name.starts_with("_") {
                continue;
            }

            ewe_logs::debug!(
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
                )
                .into()],
            ));
        }

        packager
            .run(configurator.params())
            .map_err(|err| PackageGenError::Failed(err.into()))
    }
}

#[cfg(test)]
mod package_generator_tests {

    use std::collections::HashMap;
    use strings_ext::IntoString;

    use tracing_test::traced_test;

    use super::*;

    #[derive(rust_embed::Embed, Default)]
    #[folder = "templates/"]
    struct TemplateDefinitions;

    #[test]
    #[traced_test]
    fn package_generator_can_create_package_for_workspace() {
        let template_directories = Box::new(Directorate::<TemplateDefinitions>::default());
        let packager = PackageGenerator::new(template_directories);

        let current_dir = std::env::current_dir().expect("should have gotten directory");

        let output_directory = current_dir.join("output_directory");
        let project_directory = output_directory.join("workspace_project");
        let project_cargo_file = project_directory.join("Cargo.toml");

        let mut params: HashMap<String, String> = HashMap::new();
        params.entry(String::from("PROJECT_DIRECTORY")).or_insert(
            project_directory
                .into_string()
                .expect("should convert into string"),
        );

        let rust_config = RustConfig::new(project_cargo_file);
        let package_config = PackageConfig::new(
            project_directory,
            params,
            "CustomRustProject",
            "retro_project",
        );

        let rust_configurator = RustProjectConfigurator::new(package_config, Some(rust_config))
            .expect("should generate rust configurator");

        assert!(matches!(packager.create(rust_configurator), Ok(())));
    }
}
