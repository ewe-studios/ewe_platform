//! Produce browser artifacts before committing an application's staged resource tree.
use crate::build::{stage_from_cargo, stage_outputs};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    env,
    ffi::OsStr,
    fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Package settings for a Cargo application that has a WebAssembly library target.
pub struct BrowserPackage<'a> {
    entry: &'a Path,
    feature: &'a str,
    connect: bool,
    resources: bool,
    npm_lock: Option<&'a Path>,
}

impl<'a> BrowserPackage<'a> {
    /// Select a package-relative JavaScript entry and the default Cargo feature `web`.
    #[must_use]
    pub fn new(entry: &'a Path) -> Self {
        Self {
            entry,
            feature: "web",
            connect: false,
            resources: false,
            npm_lock: None,
        }
    }

    /// Select the Cargo feature that enables the application's browser library.
    #[must_use]
    pub fn with_feature(mut self, feature: &'a str) -> Self {
        self.feature = feature;
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

    /// Produce the application script and binary modules, then stage the declared resources.
    ///
    /// # Errors
    /// Returns a Cargo, binding generator, bundling, or resource staging error.
    pub fn build(self) -> io::Result<()> {
        if env::var_os("CARGO_CFG_TARGET_FAMILY").as_deref() == Some(OsStr::new("wasm")) {
            return Ok(());
        }
        if env::var_os("EWE_RESOURCE_TARGET").is_some_and(|target| target != "web") {
            return stage_from_cargo().map(drop);
        }
        let package = required("CARGO_MANIFEST_DIR")?.canonicalize()?;
        let output = required("OUT_DIR")?;
        let target = output.join("browser-target");
        let cargo = required("CARGO")?;
        let manifest = package.join("Cargo.toml");
        let packages = metadata(&cargo, &manifest)?;
        let app = packages
            .iter()
            .find(|p| {
                p.manifest_path == manifest
                    || p.manifest_path
                        .canonicalize()
                        .is_ok_and(|path| path == manifest)
            })
            .ok_or_else(|| {
                io::Error::other(format!("Cargo did not report {}", manifest.display()))
            })?;
        let tools = Tools {
            cargo,
            target,
            bindgen: env::var_os("WASM_BINDGEN")
                .map_or_else(|| PathBuf::from("wasm-bindgen"), PathBuf::from),
        };
        println!("cargo:rerun-if-env-changed=WASM_BINDGEN");
        let private = tempfile::tempdir_in(&output)?;
        let tree = private.path().join("modules");
        if let Some(lock) = self.npm_lock {
            npm_runtime(&package, lock, private.path(), &tree)?;
        } else {
            foundation_wasm_ui::build_hook::application_sources(&tree)?;
        }
        let entry = package.join(self.entry).canonicalize()?;
        if !entry.starts_with(&package) {
            return Err(io::Error::other("Application entry escapes its package"));
        }
        let parent = entry
            .parent()
            .ok_or_else(|| io::Error::other("Missing application directory"))?;
        copy_sources(parent, &tree, parent)?;
        let entry = tree.join(
            entry
                .file_name()
                .ok_or_else(|| io::Error::other("Missing application entry name"))?,
        );
        let temporary = private.path().join("bindings");
        let app_stem = tools.produce(app, self.feature, "bundler", &temporary)?;
        fs::create_dir_all(tree.join("bindings"))?;
        let bindings = format!("{app_stem}_bg.js");
        let host_bindings = output.join("browser-bindings");
        fs::create_dir_all(&host_bindings)?;
        foundation_wasm_ui::build_hook::bindings(
            &temporary.join(&bindings),
            &host_bindings.join(&bindings),
        )?;
        fs::copy(
            host_bindings.join(&bindings),
            tree.join("bindings").join(&bindings),
        )?;
        let mut artifacts = BTreeMap::from([(
            "module.wasm".to_owned(),
            temporary.join(format!("{app_stem}_bg.wasm")),
        )]);
        if self.connect {
            let rpc = choose(&packages, "foundation_ui_connectrpc")?;
            add_companion(&tools, rpc, "browser", "rpc", &tree, &mut artifacts)?;
        }
        if self.resources {
            let resources = choose(&packages, "foundation_resources")?;
            add_companion(
                &tools,
                resources,
                "browser-bindings",
                "resources-wasm",
                &tree,
                &mut artifacts,
            )?;
        }
        let compact = env::var("PROFILE").map_err(io::Error::other)? == "release";
        let script = foundation_codegen::javascript::application(&entry, &tree, compact)
            .map_err(|e| io::Error::other(format!("{}: {e:#}", entry.display())))?;
        let script_file = private.path().join("bundle.js");
        fs::write(&script_file, script.bytes)?;
        artifacts.insert("bundle.js".to_owned(), script_file);
        stage_artifacts(artifacts, &output)
    }
}

fn npm_runtime(package: &Path, lock: &Path, private: &Path, tree: &Path) -> io::Result<()> {
    let name = "@ewe-studios/foundation-wasm-ui";
    let version = foundation_wasm_ui::build_hook::RUNTIME_VERSION;
    let lock = package.join(lock).canonicalize()?;
    if !lock.starts_with(package) {
        return Err(io::Error::other("Browser runtime lock escapes its package"));
    }
    let manifest = lock.with_file_name("package.json");
    println!("cargo:rerun-if-changed={}", lock.display());
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-env-changed=EWE_UI_OFFLINE");
    let file: serde_json::Value = serde_json::from_reader(BufReader::new(fs::File::open(&lock)?))?;
    let entry = &file["packages"][format!("node_modules/{name}")];
    if entry["version"].as_str() != Some(version)
        || !entry["integrity"]
            .as_str()
            .is_some_and(|s| s.starts_with("sha512-") && s.len() == 95)
    {
        return Err(io::Error::other(format!(
            "Expected a locked SHA-512 for {name}@{version}"
        )));
    }
    let npm = private.join("npm");
    fs::create_dir(&npm)?;
    fs::copy(lock, npm.join("package-lock.json"))?;
    fs::copy(manifest, npm.join("package.json"))?;
    let mut cmd = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
    cmd.current_dir(&npm)
        .args([
            "ci",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--prefer-offline",
        ])
        .env_remove("NPM_TOKEN")
        .env_remove("NODE_AUTH_TOKEN");
    if env::var_os("EWE_UI_OFFLINE").as_deref() == Some(OsStr::new("1"))
        || env::var_os("CARGO_NET_OFFLINE").as_deref() == Some(OsStr::new("true"))
    {
        cmd.arg("--offline");
    }
    let status = cmd
        .status()
        .map_err(|e| io::Error::other(format!("Install {name}@{version}: {e}")))?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "Cannot resolve {name}@{version} from {} ({status})",
            npm.display()
        )));
    }
    let installed = npm.join("node_modules").join(name);
    fs::create_dir_all(tree)?;
    let modules = installed.join("ui/runtimes");
    for entry in fs::read_dir(&modules)? {
        let entry = entry?;
        if entry.path().extension() == Some(OsStr::new("js")) {
            fs::copy(entry.path(), tree.join(entry.file_name()))?;
        }
    }
    fs::copy(
        installed.join("foundation_wasm/runtime/foundation-wasm.js"),
        tree.join("foundation-wasm.js"),
    )?;
    Ok(())
}

fn stage_artifacts(artifacts: BTreeMap<String, PathBuf>, output: &Path) -> io::Result<()> {
    let mut generated = Vec::new();
    for (name, source) in artifacts {
        if Path::new(&name).extension() == Some(OsStr::new("wasm")) {
            let mut header = [0; 8];
            fs::File::open(&source)?
                .read_exact(&mut header)
                .map_err(|e| io::Error::new(e.kind(), format!("Read {name}: {e}")))?;
            if &header != b"\0asm\x01\0\0\0" {
                return Err(io::Error::other(format!(
                    "Invalid WebAssembly header in {name}"
                )));
            }
        }
        let relative = PathBuf::from("browser-package").join(&name);
        let destination = output.join(&relative);
        fs::create_dir_all(
            destination
                .parent()
                .ok_or_else(|| io::Error::other("Generated file has no parent"))?,
        )?;
        if !crate::build::same_file(&source, &destination)? {
            fs::copy(&source, &destination)?;
        }
        generated.push((name, relative));
    }
    stage_outputs(&generated).map(drop)
}

fn add_companion(
    tools: &Tools,
    package: &Package,
    feature: &str,
    folder: &str,
    tree: &Path,
    assets: &mut BTreeMap<String, PathBuf>,
) -> io::Result<()> {
    let out = tree.join(folder);
    let name = tools.produce(package, feature, "web", &out)?;
    let file = format!("{name}_bg.wasm");
    assets.insert(format!("{folder}/{file}"), out.join(file));
    Ok(())
}

fn required(name: &str) -> io::Result<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other(format!("Cargo did not set {name}")))
}

fn copy_sources(dir: &Path, dest: &Path, root: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if dest.starts_with(&path)
            || ["target", ".git", "node_modules"]
                .iter()
                .any(|skip| entry.file_name() == *skip)
        {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_sources(&path, &target, root)?;
        } else if path.extension() == Some(OsStr::new("js")) {
            if !path.canonicalize()?.starts_with(root) {
                return Err(io::Error::other(format!(
                    "Source escapes its directory: {}",
                    path.display()
                )));
            }
            if target.exists() {
                return Err(io::Error::other(format!(
                    "Application source conflicts with {}",
                    target.display()
                )));
            }
            println!("cargo:rerun-if-changed={}", path.display());
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    crate_types: Vec<String>,
}

fn choose<'a>(packages: &'a [Package], name: &str) -> io::Result<&'a Package> {
    let mut iter = packages.iter().filter(|p| p.name == name);
    let found = iter.next().ok_or_else(|| {
        io::Error::other(format!(
            "Missing browser package {name} in the Cargo dependency graph"
        ))
    })?;
    if iter.next().is_some() {
        return Err(io::Error::other(format!(
            "Multiple browser packages named {name}"
        )));
    }
    Ok(found)
}

fn metadata(cargo: &Path, manifest: &Path) -> io::Result<Vec<Package>> {
    let mut child = Command::new(cargo)
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest)
        .stdout(Stdio::piped())
        .spawn()?;
    let data = serde_json::from_reader::<_, Metadata>(BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Missing metadata pipe"))?,
    ));
    if !child.wait()?.success() {
        return Err(io::Error::other(format!(
            "Cargo metadata failed for {}",
            manifest.display()
        )));
    }
    Ok(data.map_err(io::Error::other)?.packages)
}

struct Tools {
    cargo: PathBuf,
    target: PathBuf,
    bindgen: PathBuf,
}

impl Tools {
    fn produce(
        &self,
        package: &Package,
        feature: &str,
        mode: &str,
        dest: &Path,
    ) -> io::Result<String> {
        let lib = package
            .targets
            .iter()
            .find(|t| t.crate_types.iter().any(|s| s == "cdylib"))
            .ok_or_else(|| io::Error::other(format!("{} needs a cdylib target", package.name)))?;
        let stem = lib.name.replace('-', "_");
        let mut cmd = Command::new(&self.cargo);
        cmd.args([
            "build",
            "--lib",
            "--no-default-features",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--target-dir",
        ])
        .arg(&self.target)
        .arg("--manifest-path")
        .arg(&package.manifest_path)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS");
        if !feature.is_empty() {
            cmd.args(["--features", feature]);
        }
        run(
            &mut cmd,
            &format!("Cargo browser build for {}", package.name),
        )?;
        let release = self.target.join("wasm32-unknown-unknown/release");
        track(&release.join(format!("{stem}.d")), &self.target)?;
        println!("cargo:rerun-if-changed={}", package.manifest_path.display());
        fs::create_dir_all(dest)?;
        let mut command = Command::new(&self.bindgen);
        command
            .arg(release.join(format!("{stem}.wasm")))
            .args(["--target", mode, "--no-typescript", "--out-dir"])
            .arg(dest);
        run(
            &mut command,
            &format!("Generate browser bindings for {}", package.name),
        )?;
        Ok(stem)
    }
}

fn run(command: &mut Command, description: &str) -> io::Result<()> {
    let status = command
        .status()
        .map_err(|e| io::Error::new(e.kind(), format!("{description}: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{description}: {status}")))
    }
}

fn track(depfile: &Path, target: &Path) -> io::Result<()> {
    // Cargo includes build outputs and their source inputs in this file.
    let track_source = |path: &str| {
        if !Path::new(path).starts_with(target) {
            println!("cargo:rerun-if-changed={path}");
        }
    };
    let mut line = String::new();
    BufReader::new(fs::File::open(depfile)?).read_line(&mut line)?;
    let (_, inputs) = line.split_once(": ").ok_or_else(|| {
        io::Error::other(format!(
            "Invalid Cargo dependency file {}",
            depfile.display()
        ))
    })?;
    let mut parts = inputs.trim_end().chars().peekable();
    let mut path = String::new();
    while let Some(ch) = parts.next() {
        match ch {
            '\\' if parts.peek() == Some(&' ') => {
                parts.next();
                path.push(' ');
            }
            ' ' if !path.is_empty() => {
                track_source(&path);
                path.clear();
            }
            ' ' => {}
            _ => path.push(ch),
        }
    }
    if !path.is_empty() {
        track_source(&path);
    }
    Ok(())
}
