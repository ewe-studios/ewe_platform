//! Validate and stage a native release before publication.
use super::{archive, output::query, swift, NativeBuildEnv};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256, Sha512};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use zip::ZipArchive;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub version: String,
    pub repository: String,
    pub apple_archive: String,
    maven_group: String,
    maven_modules: Vec<String>,
    android_targets: Vec<String>,
    swift_product: String,
}

/// Read the native release settings from the workspace.
///
/// # Errors
/// Returns a file or configuration error.
pub fn config(root: &Path) -> Result<Config, TaskError> {
    toml::from_str(
        &fs::read_to_string(root.join("release/native-ui.toml")).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)
}

pub(super) async fn copy(source: &Path, destination: &Path) -> TaskResult {
    let (source, destination) = (source.to_owned(), destination.to_owned());
    blocking::unblock(move || fs::copy(source, destination))
        .await
        .map_err(TaskError::operation)?;
    Ok(())
}

/// Compute the SHA-256 digest of a file.
///
/// # Errors
/// Returns file open and read errors.
pub async fn digest(path: &Path) -> Result<String, TaskError> {
    let path = path.to_owned();
    blocking::unblock(move || digest_file(&path)).await
}

pub(super) fn digest_file(path: &Path) -> Result<String, TaskError> {
    let mut hash = Sha256::new();
    let mut reader = BufReader::new(File::open(path).map_err(TaskError::operation)?);
    loop {
        let bytes = reader.fill_buf().map_err(TaskError::operation)?;
        if bytes.is_empty() {
            break;
        }
        hash.update(bytes);
        let count = bytes.len();
        reader.consume(count);
    }
    Ok(hex::encode(hash.finalize()))
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Framework {
    available_libraries: Vec<Library>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Library {
    #[serde(rename = "LibraryIdentifier")]
    identifier: String,
    #[serde(rename = "LibraryPath")]
    path: String,
    headers_path: String,
    supported_platform: String,
    supported_platform_variant: Option<String>,
    supported_architectures: Vec<String>,
}

fn verify_apple(path: &Path, config: &Config) -> TaskResult {
    if path.file_name().and_then(|s| s.to_str()) != Some(config.apple_archive.as_str()) {
        return Err(TaskError::operation("Unexpected Apple artifact name"));
    }
    let mut archive = ZipArchive::new(File::open(path).map_err(TaskError::operation)?)
        .map_err(TaskError::operation)?;
    let plist = archive
        .by_name("EweCoreFFI.xcframework/Info.plist")
        .map_err(TaskError::operation)?;
    let info: Framework = plist::from_reader_xml(plist).map_err(TaskError::operation)?;
    let mut variants = BTreeSet::new();
    for lib in info.available_libraries {
        let prefix = format!("EweCoreFFI.xcframework/{}/", lib.identifier);
        if archive
            .by_name(&(prefix.clone() + &lib.path))
            .map_err(TaskError::operation)?
            .size()
            == 0
        {
            return Err(TaskError::operation("Empty Apple library"));
        }
        for header in ["module.modulemap", "EweCoreFFI.h"] {
            archive
                .by_name(&format!("{prefix}{}/{header}", lib.headers_path))
                .map_err(TaskError::operation)?;
        }
        let platform = lib
            .supported_platform_variant
            .unwrap_or(lib.supported_platform);
        for arch in lib.supported_architectures {
            variants.insert(format!("{platform}-{arch}"));
        }
    }
    if variants
        != BTreeSet::from(
            [
                "ios-arm64",
                "simulator-arm64",
                "simulator-x86_64",
                "macos-arm64",
                "macos-x86_64",
            ]
            .map(String::from),
        )
    {
        return Err(TaskError::operation("Incorrect Apple library variants"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pom {
    group_id: String,
    artifact_id: String,
    version: String,
    packaging: Option<String>,
    name: String,
    description: String,
    url: String,
    licenses: Licenses,
    developers: Developers,
    scm: Scm,
    dependencies: Option<Dependencies>,
}
#[derive(Deserialize)]
struct Licenses {
    license: Vec<Named>,
}
#[derive(Deserialize)]
struct Developers {
    developer: Vec<Named>,
}
#[derive(Deserialize)]
struct Named {
    name: String,
}
#[derive(Deserialize)]
struct Scm {
    connection: String,
}
#[derive(Deserialize)]
struct Dependencies {
    dependency: Vec<Coordinates>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Coordinates {
    group_id: String,
    artifact_id: String,
    version: Option<String>,
}

/// List the validated publication files in a local Maven repository.
///
/// # Errors
/// Returns an error if a package or its required metadata is invalid or missing.
pub async fn maven_inputs(repo: &Path, config: &Config) -> Result<Vec<PathBuf>, TaskError> {
    let (repo, config) = (repo.to_owned(), config.clone());
    blocking::unblock(move || read_maven_inputs(&repo, &config)).await
}

fn read_maven_inputs(repo: &Path, config: &Config) -> Result<Vec<PathBuf>, TaskError> {
    let mut files = Vec::new();
    for module in &config.maven_modules {
        let dir = repo
            .join(config.maven_group.replace('.', "/"))
            .join(module)
            .join(&config.version);
        let stem = format!("{module}-{}", config.version);
        let pom = dir.join(format!("{stem}.pom"));
        let declaration: Pom = quick_xml::de::from_reader(BufReader::new(
            File::open(&pom)
                .map_err(|error| TaskError::operation(format!("{}: {error}", pom.display())))?,
        ))
        .map_err(|error| TaskError::operation(format!("{}: {error}", pom.display())))?;
        let c = &declaration;
        if c.group_id != config.maven_group
            || c.artifact_id != *module
            || c.version != config.version
            || [
                &declaration.name,
                &declaration.description,
                &declaration.url,
                &declaration.scm.connection,
            ]
            .into_iter()
            .any(String::is_empty)
            || !declaration
                .licenses
                .license
                .iter()
                .any(|x| !x.name.is_empty())
            || !declaration
                .developers
                .developer
                .iter()
                .any(|x| !x.name.is_empty())
        {
            return Err(TaskError::operation(format!(
                "Invalid publication metadata in {}",
                pom.display()
            )));
        }
        let kind = declaration.packaging.as_deref().unwrap_or("jar");
        if kind
            != if module == "ewe-ui-core" {
                "jar"
            } else {
                "aar"
            }
        {
            return Err(TaskError::operation("Incorrect package type"));
        }
        let primary = dir.join(format!("{stem}.{kind}"));
        if module == "ewe-ui-binding" {
            let mut archive = ZipArchive::new(File::open(&primary).map_err(TaskError::operation)?)
                .map_err(TaskError::operation)?;
            for abi in &config.android_targets {
                if archive
                    .by_name(&format!("jni/{abi}/libfoundation_ui_native.so"))
                    .map_err(TaskError::operation)?
                    .size()
                    == 0
                {
                    return Err(TaskError::operation(format!("Empty library for {abi}")));
                }
            }
        }
        if module == "ewe-ui" {
            let deps = declaration
                .dependencies
                .ok_or_else(|| TaskError::operation("Missing runtime dependencies"))?;
            for dep in config.maven_modules.iter().filter(|name| *name != module) {
                if !deps.dependency.iter().any(|d| {
                    d.group_id == config.maven_group
                        && d.artifact_id == *dep
                        && d.version.as_deref() == Some(config.version.as_str())
                }) {
                    return Err(TaskError::operation(format!(
                        "Missing runtime dependency {dep}"
                    )));
                }
            }
        }
        for file in [
            pom,
            primary,
            dir.join(format!("{stem}-sources.jar")),
            dir.join(format!("{stem}-javadoc.jar")),
            dir.join(format!("{stem}.module")),
        ] {
            if !file.is_file() || file.metadata().map_err(TaskError::operation)?.len() == 0 {
                return Err(TaskError::operation(format!(
                    "Missing publication input {}",
                    file.display()
                )));
            }
            files.push(file);
        }
    }
    Ok(files)
}

pub(super) async fn maven(
    ctx: &TaskContext,
    repo: &Path,
    config: &Config,
    stage: &Path,
    key: Option<&str>,
    keyring: Option<&Path>,
) -> Result<PathBuf, TaskError> {
    let files = maven_inputs(repo, config).await?;
    let temp = tempfile::tempdir_in(stage).map_err(TaskError::operation)?;
    let mut members = Vec::new();
    for original in files {
        let path = temp
            .path()
            .join(original.strip_prefix(repo).map_err(TaskError::operation)?);
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| TaskError::operation("Missing Maven directory"))?,
        )
        .map_err(TaskError::operation)?;
        copy(&original, &path).await?;
        if let Some(key) = key {
            let sig = path.with_added_extension("asc");
            ctx.exec(
                super::sign::gpg(keyring)
                    .args([
                        "--batch",
                        "--armor",
                        "--detach-sign",
                        "--local-user",
                        key,
                        "--output",
                    ])
                    .arg(&sig)
                    .arg(&path),
            )
            .await?;
            members.push(sig);
        }
        let input = path.clone();
        let (sha1, sha256, sha512) = blocking::unblock(move || -> Result<_, TaskError> {
            let mut reader = BufReader::new(File::open(&input).map_err(TaskError::operation)?);
            let (mut sha1, mut sha256, mut sha512) =
                (sha1::Sha1::new(), Sha256::new(), Sha512::new());
            loop {
                let bytes = reader.fill_buf().map_err(TaskError::operation)?;
                if bytes.is_empty() {
                    break;
                }
                sha1.update(bytes);
                sha256.update(bytes);
                sha512.update(bytes);
                let size = bytes.len();
                reader.consume(size);
            }
            Ok((sha1, sha256, sha512))
        })
        .await?;
        let md5 = query(
            ctx,
            ProcessCommand::new("openssl")
                .args(["dgst", "-md5", "-binary"])
                .arg(&path),
        )
        .await?;
        if md5.len() != 16 {
            return Err(TaskError::operation("Invalid MD5 digest from OpenSSL"));
        }
        for (ext, hash) in [
            ("md5", hex::encode(md5)),
            ("sha1", hex::encode(sha1.finalize())),
            ("sha256", hex::encode(sha256.finalize())),
            ("sha512", hex::encode(sha512.finalize())),
        ] {
            let file = path.with_added_extension(ext);
            fs::write(&file, hash).map_err(TaskError::operation)?;
            members.push(file);
        }
        members.push(path);
    }
    let bundle = stage.join("android-publication.zip");
    let result = bundle.clone();
    blocking::unblock(move || archive::pack(temp.path(), members, &result)).await?;
    Ok(bundle)
}

fn tool_value(root: &Path, file: &str) -> Result<Value, TaskError> {
    let data = toml::from_str::<toml::Value>(
        &fs::read_to_string(root.join(file)).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    serde_json::to_value(data).map_err(TaskError::operation)
}

fn wire_version(root: &Path, file: &str, name: &str) -> Result<u64, TaskError> {
    let source = fs::read_to_string(root.join(file)).map_err(TaskError::operation)?;
    let pattern = regex::Regex::new(&format!("pub const {name}: u(?:16|32) = ([0-9]+);"))
        .map_err(TaskError::operation)?;
    pattern
        .captures(&source)
        .and_then(|c| c[1].parse().ok())
        .ok_or_else(|| TaskError::operation(format!("Missing {name} in {file}")))
}

pub(super) fn write_json(path: &Path, value: &Value) -> TaskResult {
    let mut value = value.clone();
    value.sort_all_objects();
    let mut file = File::create(path).map_err(TaskError::operation)?;
    serde_json::to_writer_pretty(&mut file, &value).map_err(TaskError::operation)?;
    file.write_all(b"\n").map_err(TaskError::operation)
}

pub(super) async fn release(
    ctx: &TaskContext,
    root: &Path,
    repo: &Path,
    apple: &Path,
    out: &Path,
    revision: &str,
) -> TaskResult {
    let config = config(root)?;
    if tool_value(root, "backends/foundation_ui_native/Cargo.toml")?["package"]["version"]
        != config.version
    {
        return Err(TaskError::operation(
            "The native crate and release versions differ",
        ));
    }
    verify_apple(apple, &config)?;
    let apple_digest = digest(apple).await?;
    let url = format!(
        "https://github.com/{}/releases/download/native-ui-v{}/{}",
        config.repository, config.version, config.apple_archive
    );
    let (commit, manifest) = swift::release(ctx, root, revision, &url, &apple_digest).await?;
    fs::create_dir_all(out).map_err(TaskError::operation)?;
    let stage = tempfile::tempdir_in(out).map_err(TaskError::operation)?;
    let staged_apple = stage.path().join(&config.apple_archive);
    copy(apple, &staged_apple).await?;
    let env = ctx.require::<NativeBuildEnv>()?;
    let android = maven(
        ctx,
        repo,
        &config,
        stage.path(),
        env.signing_key.as_deref(),
        None,
    )
    .await?;
    let mut apple_tools = BTreeMap::new();
    for (name, program, args) in [
        ("xcode", "xcodebuild", &["-version"][..]),
        (
            "iphoneos_sdk",
            "xcrun",
            &["--sdk", "iphoneos", "--show-sdk-version"],
        ),
        (
            "iphonesimulator_sdk",
            "xcrun",
            &["--sdk", "iphonesimulator", "--show-sdk-version"],
        ),
        ("swift", "xcrun", &["swift", "--version"]),
    ] {
        let text = String::from_utf8(
            query(
                ctx,
                env.apple_command(root, program).args(args.iter().copied()),
            )
            .await?,
        )
        .map_err(TaskError::operation)?;
        apple_tools.insert(
            name,
            if name == "swift" {
                text.lines().next().unwrap_or_default()
            } else {
                text.trim()
            }
            .to_string(),
        );
    }
    let mise = tool_value(root, "mise.toml")?;
    let select = |section: &str, keys: &[&str]| -> Result<BTreeMap<String, Value>, TaskError> {
        Ok(mise[section]
            .as_object()
            .ok_or_else(|| TaskError::operation("Invalid mise configuration"))?
            .iter()
            .filter(|(name, _)| {
                keys.contains(&name.as_str())
                    || (section == "tools" && name.contains("flatbuffers"))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    };
    let record = json!({
        "version":config.version, "repositories":{"repository":config.repository}, "source_revision":revision,
        "schema_version":wire_version(root,"backends/foundation_ui_wire/src/lib.rs","SCHEMA_VERSION")?,
        "vocabulary_version":wire_version(root,"backends/foundation_ui_wire/src/vocab/version.rs","VOCABULARY_VERSION")?,
        "swift_tag_revision":commit,"swift_manifest":manifest,
        "rust_toolchain":tool_value(root,"rust-toolchain.toml")?["toolchain"]["channel"],
        "apple_tools":apple_tools,"platform_tools":select("env",&["ANDROID_NDK","ANDROID_UI_COMPILE_SDK","ANDROID_GRADLE_PLUGIN","XCODE_MIN"])?,
        "tools":select("tools",&["java","gradle","kotlin"])?,
        "swift_product":config.swift_product,"maven_coordinates":config.maven_modules.iter().map(|m|format!("{}:{m}:{}",config.maven_group,config.version)).collect::<Vec<_>>(),
        "central_signed":env.signing_key.is_some(), "sha256":{(config.apple_archive.clone()):apple_digest,"android-publication.zip":digest(&android).await?}
    });
    let description = stage.path().join("native-ui.json");
    write_json(&description, &record)?;
    install_candidate(out, [staged_apple, android, description]).await
}

async fn install_candidate(out: &Path, files: [PathBuf; 3]) -> TaskResult {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(out.join(".stage.lock"))
        .map_err(TaskError::operation)?;
    lock.try_lock().map_err(TaskError::operation)?;
    for item in &files {
        let existing = out.join(
            item.file_name()
                .ok_or_else(|| TaskError::operation("Missing release filename"))?,
        );
        if existing.exists() && digest(&existing).await? != digest(item).await? {
            return Err(TaskError::operation(format!(
                "Different release input for {}",
                existing.display()
            )));
        }
    }
    for item in files {
        let target = out.join(
            item.file_name()
                .ok_or_else(|| TaskError::operation("Missing release filename"))?,
        );
        fs::rename(&item, target).map_err(TaskError::operation)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub(super) struct Tag {
    #[arg(long)]
    tag: Option<String>,
}

pub(super) async fn check_tag(ctx: TaskContext, args: Tag) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let release = config(&root)?;
    if args
        .tag
        .as_ref()
        .is_some_and(|tag| tag != &format!("native-ui-v{}", release.version))
    {
        return Err(TaskError::operation("Tag differs from release version"));
    }
    Ok(())
}

#[derive(clap::Args)]
pub(super) struct Stage {
    #[arg(long)]
    maven: PathBuf,
    #[arg(long)]
    apple: PathBuf,
    #[arg(long)]
    out: PathBuf,
}
pub(super) async fn run(ctx: TaskContext, args: Stage) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let revision = super::release::clean_revision(&ctx, &root).await?;
    let revision = std::str::from_utf8(&revision)
        .map_err(TaskError::operation)?
        .trim();
    release(
        &ctx,
        &root,
        &root.join(args.maven),
        &root.join(args.apple),
        &root.join(args.out),
        revision,
    )
    .await
}
