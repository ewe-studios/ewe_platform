//! Package an application's generated inputs, browser modules, and declared resources.
#[cfg(feature = "browser-package")]
mod browser;
use crate::ResourceName;
use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Package settings for a Cargo application that has a WebAssembly library target.
pub struct BrowserPackage<'a> {
    entry: &'a Path,
    feature: &'a str,
    module_path: &'a str,
    connect: bool,
    resources: bool,
    companions: Vec<(&'a str, &'a str)>,
    generated: Vec<(&'a str, Vec<u8>)>,
    browser: bool,
    publish_dev_preview: bool,
    npm_lock: Option<&'a Path>,
}

impl<'a> BrowserPackage<'a> {
    /// Select a package-relative JavaScript entry and the default Cargo feature `web`.
    #[must_use]
    pub fn new(entry: &'a Path) -> Self {
        Self {
            entry,
            feature: "web",
            module_path: "module.wasm",
            connect: false,
            resources: false,
            companions: Vec::new(),
            generated: Vec::new(),
            browser: true,
            publish_dev_preview: true,
            npm_lock: None,
        }
    }

    /// Select the Cargo feature that enables the application's browser library.
    #[must_use]
    pub fn with_feature(mut self, feature: &'a str) -> Self {
        self.feature = feature;
        self
    }

    /// Select a package-relative name for the application's WebAssembly binary.
    /// The private `ewe-app-config.js` module exports its URL as `moduleUrl`.
    #[must_use]
    pub fn with_module_path(mut self, path: &'a str) -> Self {
        self.module_path = path;
        self
    }

    /// Package the Connect transport from the application's Cargo dependency graph.
    #[must_use]
    pub fn with_connect(mut self) -> Self {
        self.connect = true;
        self
    }

    /// Package the framework's browser resource reader.
    #[must_use]
    pub fn with_resources(mut self) -> Self {
        self.resources = true;
        self
    }

    /// Load runtime modules from the exact npm archive in the project lockfile.
    #[must_use]
    pub fn with_npm_runtime(mut self, lock: &'a Path) -> Self {
        self.npm_lock = Some(lock);
        self
    }

    /// Select whether this build needs the browser application in addition to manifest resources.
    #[must_use]
    pub fn browser_enabled(mut self, enabled: bool) -> Self {
        self.browser = enabled;
        self
    }

    /// Control publication beside the development server executable.
    ///
    /// Publication is enabled by default for Web staging without an explicit destination.
    /// Package-local staging and explicit destinations use the same target in both settings.
    #[must_use]
    pub fn with_dev_preview(mut self, enabled: bool) -> Self {
        self.publish_dev_preview = enabled;
        self
    }

    /// Build another browser library from this application's Cargo package.
    /// Import its generated JavaScript from the supplied directory to load its binary on demand.
    #[must_use]
    pub fn with_companion(mut self, feature: &'a str, directory: &'a str) -> Self {
        self.companions.push((feature, directory));
        self
    }

    /// Supply bytes for a generated input declared in the resource manifest.
    #[must_use]
    pub fn with_generated(mut self, name: &'a str, bytes: impl Into<Vec<u8>>) -> Self {
        self.generated.push((name, bytes.into()));
        self
    }

    /// Build the selected assets and stage the resource manifest.
    ///
    /// # Errors
    /// Returns an error for invalid inputs, missing tools, or failed generation.
    pub fn build(self) -> io::Result<()> {
        if env::var_os("CARGO_CFG_TARGET_FAMILY").as_deref() == Some(OsStr::new("wasm")) {
            return Ok(());
        }
        ResourceName::new(self.module_path).map_err(io::Error::other)?;
        for script in ["bundle.js", "bundle.min.js"] {
            if self
                .module_path
                .split('/')
                .next()
                .is_some_and(|part| part.eq_ignore_ascii_case(script))
            {
                return Err(io::Error::other(format!(
                    "Module path {} conflicts with {script}",
                    self.module_path
                )));
            }
        }
        let output = PathBuf::from(
            env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("Cargo did not set OUT_DIR"))?,
        );
        let mut names = BTreeSet::new();
        for (name, bytes) in &self.generated {
            let name = ResourceName::new(name).map_err(io::Error::other)?;
            if !names.insert(name.clone()) {
                return Err(io::Error::other("Duplicate generated resource input"));
            }
            let path = output.join(name.as_str());
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("Invalid generated input"))?;
            fs::create_dir_all(parent)?;
            let mut builder = tempfile::Builder::new();
            builder.prefix(".ewe-resource-");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                builder.permissions(fs::Permissions::from_mode(0o666));
            }
            let mut file = builder.tempfile_in(parent)?;
            match fs::metadata(&path) {
                Ok(existing)
                    if existing.len() == bytes.len() as u64
                        && existing.permissions() == file.as_file().metadata()?.permissions()
                        && fs::read(&path)? == *bytes =>
                {
                    continue;
                }
                Ok(_) => {}
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
            file.write_all(bytes)?;
            file.persist(path)?;
        }
        if !self.browser || env::var_os("EWE_RESOURCE_TARGET").is_some_and(|t| t != "web") {
            return crate::build::stage_outputs(&[], self.publish_dev_preview).map(drop);
        }
        for (_, folder) in &self.companions {
            ResourceName::new(folder).map_err(io::Error::other)?;
        }
        #[cfg(feature = "browser-package")]
        {
            browser::build(&self)
        }
        #[cfg(not(feature = "browser-package"))]
        Err(io::Error::other(format!(
            "Enable browser-package to build {}",
            self.entry.display()
        )))
    }
}
