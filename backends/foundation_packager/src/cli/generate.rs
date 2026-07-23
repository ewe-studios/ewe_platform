//! Project generation CLI — thin wrapper over the packager library.
//!
//! This module exposes `register` and `run` for `bin/platform`'s `generate`
//! subcommand. The `ProjectTemplates` embed is kept in `bin/platform` because
//! `EmbedDirectoryAs` depends on `$OUT_DIR`, which is only set during binary
//! crate compilation — not inside a library.

use clap::ValueEnum;
use crate::{
    Directorate, PackageConfig, PackageConfigurator, PackageGenerator, RustConfig,
    RustProjectConfigurator,
};
use foundation_core::extensions::strings_ext::TryIntoString;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Language support variants for project generation.
#[derive(Clone, PartialEq, Eq, Hash, Debug, ValueEnum)]
pub enum LanguageSupport {
    /// Plain: simple copy-paste of template files with variable substitution.
    Plain,

    /// SimpleRust: basic Rust project with Cargo workspace integration.
    SimpleRust,

    /// SimpleHTML: HTML project without Rust/wasm support.
    SimpleHTML,
}

impl LanguageSupport {
    /// Generate the appropriate package configurator for this language.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_package_config(
        &self,
        template_name: String,
        project_name: String,
        retain_lib_section: bool,
        github_namespace: Option<String>,
        root_directory: std::path::PathBuf,
        new_project_directory: std::path::PathBuf,
        workspace_cargo_file: Option<std::path::PathBuf>,
    ) -> std::result::Result<Box<dyn PackageConfigurator>, BoxedError> {
        match self {
            LanguageSupport::Plain | LanguageSupport::SimpleHTML => {
                let mut params: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                params.insert("GITHUB_NAMESPACE".into(), serde_json::Value::from(github_namespace.clone()));
                params.insert("PROJECT_NAME".into(), serde_json::Value::from(project_name.clone()));
                params.insert("TEMPLATE_NAME".into(), serde_json::Value::from(template_name.clone()));
                params.insert(
                    "ROOT_PROJECT_DIRECTORY".into(),
                    serde_json::Value::from(
                        root_directory.clone().try_into_string().expect("can be string"),
                    ),
                );
                params.insert(
                    "PROJECT_DIRECTORY".into(),
                    serde_json::Value::from(
                        new_project_directory.try_into_string().expect("can be string"),
                    ),
                );
                Ok(Box::new(PackageConfig::new(
                    root_directory,
                    params,
                    template_name,
                    project_name,
                )))
            }
            LanguageSupport::SimpleRust => {
                let mut params: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                params.insert("PROJECT_NAME".into(), serde_json::Value::from(project_name.clone()));
                params.insert("GITHUB_NAMESPACE".into(), serde_json::Value::from(github_namespace.clone()));
                params.insert("TEMPLATE_NAME".into(), serde_json::Value::from(template_name.clone()));
                params.insert(
                    "ROOT_PROJECT_DIRECTORY".into(),
                    serde_json::Value::from(
                        root_directory.clone().try_into_string().expect("can be string"),
                    ),
                );
                params.insert(
                    "PROJECT_DIRECTORY".into(),
                    serde_json::Value::from(
                        new_project_directory.try_into_string().expect("can be string"),
                    ),
                );

                let package_config = PackageConfig::new(root_directory, params, template_name, project_name);
                let rust_config = Some(RustConfig::new(workspace_cargo_file, retain_lib_section));
                Ok(Box::new(RustProjectConfigurator::new(package_config, rust_config)?))
            }
        }
    }
}

/// Register the `generate` subcommand.
pub fn command() -> clap::Command {
    clap::Command::new("generate")
        .about("generate a new project from supported templates")
        .arg(
            clap::Arg::new("template_name")
                .short('t')
                .help("name of the template to be used")
                .long("template_name")
                .required(true)
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            clap::Arg::new("project_name")
                .short('p')
                .long("project_name")
                .help("name to call the new project")
                .required(true)
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            clap::Arg::new("github_url")
                .long("github_url")
                .help("path to your github namespace")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(String))
                .default_value("https://github.com/<USER>"),
        )
        .arg(
            clap::Arg::new("output")
                .short('o')
                .long("output_directory")
                .help("the directory to generate the giving project (defaults to current directory)")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
        .arg(
            clap::Arg::new("cargo_file")
                .short('c')
                .long("cargo_file")
                .help("the Cargo.toml file for identifying relevant workspace")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
        .arg(
            clap::Arg::new("retain_lib_section")
                .long("retain_lib_section")
                .help("do not wipe the [lib] section in new Cargo.toml after replicating template")
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            clap::Arg::new("lang")
                .short('l')
                .long("language")
                .help("the language configuration for the generated project")
                .action(clap::ArgAction::Set)
                .value_parser(clap::builder::EnumValueParser::<LanguageSupport>::new())
                .default_value("simple-rust"),
        )
        .arg_required_else_help(true)
}

/// Run project generation from an `EmbeddableDirectory` over `ProjectTemplates`.
///
/// The `T` parameter carries the embedded template directory; the caller
/// (typically `bin/platform`) provides it as a concrete struct with
/// `#[derive(EmbedDirectoryAs)]`.
pub fn run_with_templates<T: foundation_nostd::embeddable::EmbeddableDirectory + Default>(
    args: &clap::ArgMatches,
) -> std::result::Result<(), BoxedError> {
    let current_dir = std::env::current_dir().expect("should have gotten directory");

    let template_name = args
        .get_one::<String>("template_name")
        .expect("should have template_name");

    let retain_lib_section = args.get_one::<bool>("retain_lib_section").copied().unwrap_or(false);

    let project_name = args
        .get_one::<String>("project_name")
        .expect("should have project_name");

    let github_namespace = args.get_one::<String>("github_url").cloned();

    let output_directory = args
        .get_one::<std::path::PathBuf>("output")
        .unwrap_or(&current_dir);

    let root_project_cargo_file = args.get_one::<std::path::PathBuf>("cargo_file").cloned();

    let selected_language = args
        .get_one::<LanguageSupport>("lang")
        .expect("should have language");

    let project_output_directory = output_directory.join(project_name.clone());
    let template_directorate = Directorate::<T>::default();
    let packager = PackageGenerator::new(template_directorate);

    let package_configurator = selected_language.generate_package_config(
        template_name.clone(),
        project_name.clone(),
        retain_lib_section,
        github_namespace,
        output_directory.clone(),
        project_output_directory.clone(),
        root_project_cargo_file,
    )?;

    packager.create(package_configurator).map_err(|e| Box::new(e) as BoxedError)
}
