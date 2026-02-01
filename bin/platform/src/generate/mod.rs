use clap::ValueEnum;
use ewe_temple::{
    Directorate, PackageConfig, PackageConfigurator, PackageGenerator, RustConfig,
    RustProjectConfigurator,
};
use foundation_core::extensions::strings_ext::TryIntoString;
use foundation_macros::EmbedDirectoryAs;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(EmbedDirectoryAs, Default)]
#[source = "$OUT_DIR/templates/"]
struct ProjectTemplates;

/// Provides a means of generating custom configurator for the underlying project.
///
/// This is to allow us support different configuration of the underlying project
/// toolchain settings, inject environment variables as necessary to setup the
/// new project correctly.
///
#[derive(Clone, PartialEq, Eq, Hash, Debug, ValueEnum)]
enum LanguageSupport {
    /// Plain is your no language or project setting specific setup, a simple
    /// copy-paste operation of the target template files though they will still be
    /// parsed over.
    Plain,

    /// `SimpleRust` is your basic configuration setup where you get the generally
    /// injected project setup with the expected environment variables like `PROJECT_DIRECTORY`
    /// ..etc. This should generate work for most cases.
    SimpleRust,

    /// `SimpleHTML` is your standard configuration for a html project without any
    /// underlying rust or webassembly support added.
    /// For now it just generates same configuration as if you used `LanguageSupport::Plain`.
    ///
    /// TODO(alex): figure out the specific setup we want for html projects.
    ///
    SimpleHTML,
}

impl LanguageSupport {
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

                params
                    .entry(String::from("GITHUB_NAMESPACE"))
                    .or_insert(serde_json::Value::from(github_namespace.clone()));

                params
                    .entry(String::from("PROJECT_NAME"))
                    .or_insert(serde_json::Value::from(project_name.clone()));

                params
                    .entry(String::from("TEMPLATE_NAME"))
                    .or_insert(serde_json::Value::from(template_name.clone()));

                params
                    .entry(String::from("ROOT_PROJECT_DIRECTORY"))
                    .or_insert(serde_json::Value::from(
                        root_directory
                            .clone()
                            .try_into_string()
                            .expect("can be string"),
                    ));

                params
                    .entry(String::from("PROJECT_DIRECTORY"))
                    .or_insert(serde_json::Value::from(
                        new_project_directory
                            .try_into_string()
                            .expect("can be string"),
                    ));

                Ok(Box::new(PackageConfig::new(
                    root_directory,
                    params,
                    template_name,
                    project_name,
                )))
            }
            LanguageSupport::SimpleRust => {
                let mut params: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

                params
                    .entry(String::from("PROJECT_NAME"))
                    .or_insert(serde_json::Value::from(project_name.clone()));

                params
                    .entry(String::from("GITHUB_NAMESPACE"))
                    .or_insert(serde_json::Value::from(github_namespace.clone()));

                params
                    .entry(String::from("TEMPLATE_NAME"))
                    .or_insert(serde_json::Value::from(template_name.clone()));

                params
                    .entry(String::from("ROOT_PROJECT_DIRECTORY"))
                    .or_insert(serde_json::Value::from(
                        root_directory
                            .clone()
                            .try_into_string()
                            .expect("can be string"),
                    ));

                params
                    .entry(String::from("PROJECT_DIRECTORY"))
                    .or_insert(serde_json::Value::from(
                        new_project_directory
                            .try_into_string()
                            .expect("can be string"),
                    ));

                let package_config =
                    PackageConfig::new(root_directory, params, template_name, project_name);

                let rust_config: Option<RustConfig> = if workspace_cargo_file.is_some() {
                    Some(RustConfig::new(workspace_cargo_file, retain_lib_section))
                } else {
                    Some(RustConfig::new(None, retain_lib_section))
                };

                Ok(Box::new(RustProjectConfigurator::new(
                    package_config,
                    rust_config,
                )?))
            }
        }
    }
}

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
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
                    .value_parser(clap::value_parser!(String)).default_value("https://github.com/<USER>"),
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
                    .help("Optional configuration to indicate you do not want to wipe the [lib] section in your new Cargo.toml file after replicating template")
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
            .arg_required_else_help(true),
    )
}

pub fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let current_dir = std::env::current_dir().expect("should have gotten directory");

    let template_name = args
        .get_one::<String>("template_name")
        .expect("should have template_name");

    let retain_lib_section = args.get_one::<bool>("retain_lib_section").unwrap_or(&false);

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

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let project_output_directory = output_directory.join(project_name.clone());
    let template_directorate = Directorate::<ProjectTemplates>::default();
    let packager = PackageGenerator::new(template_directorate);

    let package_configurator = selected_language.generate_package_config(
        template_name.clone(),
        project_name.clone(),
        *retain_lib_section,
        github_namespace.clone(),
        output_directory.clone(),
        project_output_directory.clone(),
        root_project_cargo_file.clone(),
    )?;

    match packager.create(package_configurator) {
        Ok(()) => Ok(()),
        Err(err) => Err(Box::new(err)),
    }
}
