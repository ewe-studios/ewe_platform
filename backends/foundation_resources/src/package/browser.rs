//! Produce browser artifacts before committing an application's staged resource tree.
use super::BrowserPackage;
use crate::build::stage_outputs;
use serde::Deserialize;
use std::{
    env,
    ffi::OsStr,
    fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(super) fn build(options: &BrowserPackage<'_>) -> io::Result<()> {
    let package = required("CARGO_MANIFEST_DIR")?.canonicalize()?;
    let output = required("OUT_DIR")?;
    let asset_directory = options.page.map(|_| options.asset_directory);
    if let Some(directory) = asset_directory {
        crate::ResourceName::new(directory).map_err(io::Error::other)?;
        if !directory
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte))
        {
            return Err(io::Error::other(
                "Asset directory must use URL path characters",
            ));
        }
    }
    let check_base = url::Url::parse("https://example.invalid/app/").map_err(io::Error::other)?;
    let service = check_base
        .join(options.service_base)
        .map_err(io::Error::other)?;
    if !options.service_base.ends_with('/')
        || options.service_base.starts_with("//")
        || !matches!(service.scheme(), "http" | "https")
        || !service.username().is_empty()
        || service.password().is_some()
        || service.query().is_some()
        || service.fragment().is_some()
    {
        return Err(io::Error::other(
            "Expected an HTTP service directory without credentials, query, or fragment",
        ));
    }
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
        .ok_or_else(|| io::Error::other(format!("Cargo did not report {}", manifest.display())))?;
    let tools = Tools {
        cargo,
        target,
        private_bindings: output.join("browser-bindings"),
        bindgen: env::var_os("WASM_BINDGEN")
            .map_or_else(|| PathBuf::from("wasm-bindgen"), PathBuf::from),
    };
    println!("cargo:rerun-if-env-changed=WASM_BINDGEN");
    let private = tempfile::tempdir_in(&output)?;
    let mut artifacts = Vec::new();
    if let (Some(template), Some(directory)) = (options.page, asset_directory) {
        let source = crate::build::checked_source(&package, template)?;
        println!("cargo:rerun-if-changed={}", source.display());
        let page = fs::read_to_string(&source)?;
        if !page.contains("{{ewe_assets}}") {
            return Err(io::Error::other(
                "Page template needs a {{ewe_assets}} marker",
            ));
        }
        let index = private.path().join("index.html");
        fs::write(&index, page.replace("{{ewe_assets}}", directory))?;
        artifacts.push(("index.html".to_owned(), index));
    }
    let tree = private.path().join("modules");
    if let Some(lock) = options.npm_lock {
        npm_runtime(&package, lock, private.path(), &tree)?;
    } else {
        foundation_wasm_ui::build_hook::application_sources(&tree)?;
    }
    let entry = package.join(options.entry).canonicalize()?;
    if !entry.starts_with(&package) {
        return Err(io::Error::other("Application entry escapes its package"));
    }
    let parent = entry
        .parent()
        .ok_or_else(|| io::Error::other("Missing application directory"))?;
    copy_sources(parent, &tree, parent, false)?;
    let config = tree.join("ewe-app-config.js");
    let mut config = fs::File::create_new(&config).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Create private module {}: {error}", config.display()),
        )
    })?;
    use std::io::Write as _;
    writeln!(
        config,
        "export const moduleUrl = new URL({}.split(\"/\").map(encodeURIComponent).join(\"/\"), import.meta.url).href;",
        serde_json::to_string(options.module_path)?
    )?;
    writeln!(
        config,
        "export const assetBase = new URL('./', import.meta.url).href;
const mount = new URL({}, assetBase);
export const serviceBase = new URL({}, mount).href;
export const serviceUrl = route => new URL(route.startsWith('/') && !route.startsWith('//') ? route.slice(1) : route, serviceBase).href;",
        serde_json::to_string(&asset_directory.map_or_else(
            || "./".to_owned(),
            |directory| "../".repeat(directory.split('/').count()),
        ))?,
        serde_json::to_string(options.service_base)?,
    )?;
    drop(config);
    let entry = tree.join(
        entry
            .file_name()
            .ok_or_else(|| io::Error::other("Missing application entry name"))?,
    );
    let temporary = private.path().join("bindings");
    let app_stem = tools.produce(app, options.feature, "bundler", &temporary)?;
    fs::create_dir_all(tree.join("bindings"))?;
    let bindings = format!("{app_stem}_bg.js");
    let host_bindings = &tools.private_bindings;
    fs::create_dir_all(host_bindings)?;
    foundation_wasm_ui::build_hook::bindings(
        &temporary.join(&bindings),
        &host_bindings.join(&bindings),
    )?;
    fs::copy(
        host_bindings.join(&bindings),
        tree.join("bindings").join(&bindings),
    )?;
    artifacts.push((
        options.module_path.to_owned(),
        temporary.join(format!("{app_stem}_bg.wasm")),
    ));
    if options.connect {
        let rpc = choose(&packages, "foundation_ui_connectrpc")?;
        add_companion(&tools, rpc, "browser", "rpc", &tree, &mut artifacts)?;
    }
    if options.resources {
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
    for (feature, directory) in &options.companions {
        if tree.join(directory).exists() {
            return Err(io::Error::other(format!(
                "Browser module directory already exists: {directory}"
            )));
        }
        add_companion(&tools, app, feature, directory, &tree, &mut artifacts)?;
    }
    let compact = env::var("PROFILE").map_err(io::Error::other)? == "release";
    let script = foundation_codegen::javascript::application(&entry, &tree, compact)
        .map_err(|e| io::Error::other(format!("{}: {e:#}", entry.display())))?;
    let script_file = private.path().join("bundle.js");
    fs::write(&script_file, script.bytes)?;
    artifacts.push(("bundle.js".to_owned(), script_file));
    stage_artifacts(
        artifacts,
        &output,
        options.publish_dev_preview,
        asset_directory,
    )
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

fn stage_artifacts(
    artifacts: Vec<(String, PathBuf)>,
    output: &Path,
    publish_dev_preview: bool,
    asset_directory: Option<&str>,
) -> io::Result<()> {
    let mut generated = Vec::new();
    for (index, (name, source)) in artifacts.into_iter().enumerate() {
        if source.extension() == Some(OsStr::new("wasm")) {
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
        let relative = PathBuf::from("browser-package").join(format!("artifact-{index}"));
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
    stage_outputs(&generated, publish_dev_preview, asset_directory).map(drop)
}

fn add_companion(
    tools: &Tools,
    package: &Package,
    feature: &str,
    folder: &str,
    tree: &Path,
    assets: &mut Vec<(String, PathBuf)>,
) -> io::Result<()> {
    let out = tree.join(folder);
    let name = tools.produce(package, feature, "web", &out)?;
    let private = tools.private_bindings.join(folder);
    fs::create_dir_all(&private)?;
    copy_sources(&out, &private, &out, true)?;
    let file = format!("{name}_bg.wasm");
    assets.push((format!("{folder}/{file}"), out.join(file)));
    Ok(())
}

fn required(name: &str) -> io::Result<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other(format!("Cargo did not set {name}")))
}

fn copy_sources(dir: &Path, dest: &Path, root: &Path, generated: bool) -> io::Result<()> {
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
            copy_sources(&path, &target, root, generated)?;
        } else if path.extension() == Some(OsStr::new("js")) {
            if !path.canonicalize()?.starts_with(root) {
                return Err(io::Error::other(format!(
                    "Source escapes its directory: {}",
                    path.display()
                )));
            }
            if !generated && target.exists() {
                return Err(io::Error::other(format!(
                    "Application source conflicts with {}",
                    target.display()
                )));
            }
            if !generated {
                println!("cargo:rerun-if-changed={}", path.display());
            }
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
    private_bindings: PathBuf,
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
            "--locked",
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
        if let Some(jobs) = std::env::var_os("NUM_JOBS") {
            cmd.arg("--jobs").arg(jobs);
        }
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
