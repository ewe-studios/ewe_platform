//! Validate and compose one web or mobile application.
use crate::{mobile, BoxedError, ProjectTemplates};
use clap::ValueEnum;
use foundation_packager::{Directorate, PackageConfig, PackageGenerator};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Web,
    Android,
    Ios,
}
impl Target {
    #[must_use]
    pub fn directory(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Android => "android",
            Self::Ios => "ios",
        }
    }
}
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Host {
    #[default]
    Server,
    OnDevice,
}
impl Host {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::OnDevice => "on-device",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidProject {
    pub directory: PathBuf,
    pub module: String,
    pub launch_component: String,
    pub architectures: Vec<String>,
    pub framework_libraries: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleProject {
    pub directory: PathBuf,
    pub project: PathBuf,
    pub scheme: String,
    pub product: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeLibrary {
    pub directory: PathBuf,
    pub package: String,
    pub library: String,
    pub bindings_bin: String,
    pub swift_module: String,
    pub swift_ffi: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Application {
    pub name: String,
    pub targets: BTreeSet<Target>,
    pub app_id: Option<String>,
    pub host: Host,
    pub shared: PathBuf,
    pub android: Option<AndroidProject>,
    pub ios: Option<AppleProject>,
    pub native: Option<NativeLibrary>,
    #[serde(default)]
    pub tasks: Option<LocalTasks>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct LocalTasks {
    pub manifest: PathBuf,
    pub namespace: String,
    pub commands: Vec<LocalCommand>,
}

impl LocalTasks {
    /// Read optional task declarations from a generated or existing application.
    ///
    /// # Errors
    /// Returns an error for an unreadable or invalid application manifest.
    pub fn discover(root: &Path) -> Result<Option<Self>, BoxedError> {
        #[derive(Deserialize)]
        struct Project {
            tasks: Option<LocalTasks>,
        }
        let path = root.join("ewe-ui.toml");
        let data = match fs::read_to_string(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Read {}: {e}", path.display()).into()),
        };
        Ok(toml::from_str::<Project>(&data)?.tasks)
    }
}

/// A project task and its public argument contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalCommand {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub arguments: Vec<LocalArgument>,
}

/// A long option or a positional value accepted by the local runner.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalArgument {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub kind: LocalArgumentKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub repeat: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LocalArgumentKind {
    #[default]
    Option,
    Positional,
    Flag,
}

impl Application {
    /// Construct a plan after validating the application and selected targets.
    ///
    /// # Errors
    /// Returns invalid input or an error from the requested operation.
    pub fn new(
        name: String,
        targets: BTreeSet<Target>,
        app_id: Option<String>,
        host: Host,
    ) -> Result<Self, BoxedError> {
        let android = targets.contains(&Target::Android).then(|| AndroidProject {
            directory: "android".into(),
            module: "app".into(),
            launch_component: ".MainActivity".into(),
            architectures: vec!["arm64-v8a".into(), "x86_64".into()],
            framework_libraries: vec!["libfoundation_ui_native.so".into()],
        });
        let ios = targets.contains(&Target::Ios).then(|| AppleProject {
            directory: "ios".into(),
            project: format!("{name}.xcodeproj").into(),
            scheme: name.clone(),
            product: name.clone(),
        });
        let native = (host == Host::OnDevice).then(|| NativeLibrary {
            directory: "native-core".into(),
            package: "counter_app".into(),
            library: "counter_app".into(),
            bindings_bin: "uniffi-bindgen".into(),
            swift_module: "CounterKit".into(),
            swift_ffi: "CounterFFI".into(),
        });
        let value = Self {
            android,
            ios,
            native,
            name,
            targets,
            app_id,
            host,
            shared: "application".into(),
            tasks: None,
        };
        value.validate()?;
        Ok(value)
    }
    /// Check the project name, target selection, and mobile identifier.
    ///
    /// # Errors
    /// Returns invalid input or an error from the requested operation.
    pub fn validate(&self) -> Result<(), BoxedError> {
        if !mobile::identifier(&self.name) {
            return Err("Invalid --name. Start with an ASCII letter and use letters, digits, or underscores.".into());
        }
        if self.targets.is_empty() {
            return Err("Select at least one --target.".into());
        }
        let mobile = self.targets.iter().any(|target| *target != Target::Web);
        if mobile {
            let id = self
                .app_id
                .as_deref()
                .ok_or("--app-id is required for Android and iOS")?;
            if !mobile::package_identifier(id)
                || self.targets.contains(&Target::Ios) && id.contains('_')
            {
                return Err("Invalid --app-id for the selected mobile targets".into());
            }
        } else if self.app_id.is_some() || self.host == Host::OnDevice {
            return Err("--app-id and --host require a mobile target".into());
        }
        Ok(())
    }
    /// Read the application declaration from its root.
    ///
    /// # Errors
    /// Returns invalid input or an error from the requested operation.
    pub fn load(root: &Path) -> Result<Self, BoxedError> {
        let path = root.join("ewe-ui.toml");
        let data =
            fs::read_to_string(&path).map_err(|e| format!("Read {}: {e}", path.display()))?;
        let app: Self = toml::from_str(&data)?;
        app.validate()?;
        Ok(app)
    }
    /// Publish a complete application tree in the requested parent directory.
    ///
    /// # Errors
    /// Returns invalid input or an error from the requested operation.
    pub fn generate(&self, parent: &Path) -> Result<PathBuf, BoxedError> {
        self.validate()?;
        fs::create_dir_all(parent)?;
        let destination = parent.join(&self.name);
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(format!("Destination exists: {}", destination.display()).into());
        }
        let temporary = tempfile::tempdir_in(parent)?;
        let staging = temporary.path().join("application");
        fs::create_dir(&staging)?;
        self.render(&staging)?;
        publish(&staging, &destination)
            .map_err(|e| format!("Publish {}: {e}", destination.display()))?;
        Ok(destination)
    }
    fn render(&self, root: &Path) -> Result<(), BoxedError> {
        let mut values = mobile::values(
            &self.name,
            self.app_id.as_deref().unwrap_or(""),
            self.host.name(),
        )?;
        values.insert("COMPOSED".into(), true.into());
        values.insert("RESOURCE_PACKAGE".into(), "../application".into());
        let renderer = PackageGenerator::new(Directorate::<ProjectTemplates>::default());
        let render = |template: &str, folder: &str| -> Result<(), BoxedError> {
            renderer
                .create(PackageConfig::new(
                    root.into(),
                    values.clone(),
                    template.to_owned(),
                    folder.to_owned(),
                ))
                .map_err(|e| {
                    format!("Render {template} at {}: {e}", root.join(folder).display()).into()
                })
        };
        if self.targets.iter().any(|target| *target != Target::Web) {
            render("mobile-launch", "public")?;
        }
        render("mobile-resources", "application")?;
        render("ui-application", "application")?;
        if self.host == Host::OnDevice {
            render("mobile-rust", "native-core")?;
        }
        for target in &self.targets {
            let folder = target.directory();
            if *target == Target::Web {
                render("ui-web", folder)?;
                continue;
            }
            for variant in ["common", self.host.name()] {
                render(&format!("{folder}-app/{variant}"), folder)?;
            }
            let host = root.join(folder);
            if *target == Target::Ios {
                let project = host.join("Starter.xcodeproj");
                let schemes = project.join("xcshareddata/xcschemes");
                fs::rename(
                    schemes.join("Starter.xcscheme"),
                    schemes.join(format!("{}.xcscheme", self.name)),
                )
                .map_err(|e| format!("Write iOS scheme for {}: {e}", self.name))?;
                fs::rename(project, host.join(format!("{}.xcodeproj", self.name)))?;
            } else {
                mobile::executable(&host.join("gradlew"))?;
            }
            if self.host == Host::OnDevice {
                fs::remove_file(host.join("build-rust.sh"))?;
            }
        }
        fs::write(root.join("ewe-ui.toml"), toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn publish(source: &Path, target: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        target,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    Ok(())
}
#[cfg(windows)]
fn publish(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}
#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple"
)))]
fn publish(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Atomic project creation is unavailable on this host",
    ))
}
