//! Publish a verified archive and compare its integrity after a retry.
use super::npm::{NAME, VERSION};
use base64::{engine::general_purpose::STANDARD, Engine};
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskError, TaskOutput, TaskResult, TaskSet,
};
use futures_util::future::BoxFuture;
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{self, BufRead, BufReader, Seek, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    prelude::rust_2021::*,
    sync::{Arc, Mutex},
};

#[derive(Args)]
struct Release {
    /// Exact runtime archive tested by the consumer job.
    #[arg(long)]
    archive: PathBuf,
    /// SHA-512 integrity recorded by the consumer job.
    #[arg(long)]
    integrity: String,
    /// Release tag for `foundation_wasm_ui`.
    #[arg(long)]
    tag: String,
    /// Full Git commit identifier for the release tag.
    #[arg(long)]
    commit: String,
    /// Official npm registry or a loopback registry for testing.
    #[arg(long, default_value = "https://registry.npmjs.org")]
    registry: String,
}

fn usable(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"'\x22`$#;\\{}".contains(&b))
}

fn token(value: Option<String>, home: Option<PathBuf>) -> Result<String, TaskError> {
    if let Some(v) = value.filter(|v| !v.is_empty()) {
        return if usable(&v) {
            Ok(v)
        } else {
            Err(TaskError::operation("Invalid NPM_TOKEN"))
        };
    }
    if let Some(home) = home {
        let path = home.join(".userenv");
        let file = match fs::File::open(path) {
            Ok(file) => Some(file),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return Err(TaskError::operation("Cannot read NPM_TOKEN from .userenv")),
        };
        if let Some(file) = file {
            for line in BufReader::new(file).lines() {
                let line =
                    line.map_err(|_| TaskError::operation("Cannot read NPM_TOKEN from .userenv"))?;
                let trimmed = line.trim();
                let assignment = trimmed
                    .strip_prefix("export")
                    .filter(|v| v.starts_with(char::is_whitespace))
                    .map_or(trimmed, str::trim_start);
                let Some((key, value)) = assignment.split_once('=') else {
                    continue;
                };
                if key.trim() != "NPM_TOKEN" {
                    continue;
                }
                let value = value.trim();
                let value = value
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                    .unwrap_or(value);
                if usable(value) {
                    return Ok(value.to_owned());
                }
            }
        }
    }
    Err(TaskError::operation("Missing NPM_TOKEN"))
}

pub(super) fn integrity(path: &Path) -> io::Result<String> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut hash = Sha512::new();
    loop {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            break;
        }
        hash.update(bytes);
        let n = bytes.len();
        reader.consume(n);
    }
    Ok(std::format!("sha512-{}", STANDARD.encode(hash.finalize())))
}

struct Output(Option<Mutex<fs::File>>);
impl TaskOutput for Output {
    fn write<'a>(&'a self, stream: OutputStream, data: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if let Some(file) = self.0.as_ref().filter(|_| stream == OutputStream::Stdout) {
                file.lock()
                    .map_err(|_| io::Error::other("Cannot read npm status"))?
                    .write_all(data)?;
            }
            Ok(())
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}
struct Client<'a> {
    host: &'a super::BrowserTaskEnv,
    dir: &'a Path,
    registry: &'a str,
}
impl Client<'_> {
    fn command(&self) -> ProcessCommand {
        let mut cmd = ProcessCommand::new("npm").env_clear().current_dir(self.dir);
        for key in [
            "PATH",
            "SYSTEMROOT",
            "COMSPEC",
            "PATHEXT",
            "TMP",
            "TEMP",
            "TMPDIR",
            "LANG",
            "SSL_CERT_FILE",
            "NODE_EXTRA_CA_CERTS",
        ] {
            if let Some(v) = (self.host.0)(key) {
                cmd = cmd.env(key, v);
            }
        }
        cmd.env("HOME", self.dir)
            .env("USERPROFILE", self.dir)
            .env("NPM_CONFIG_USERCONFIG", self.dir.join(".npmrc"))
            .env("NPM_CONFIG_CACHE", self.dir.join("cache"))
            .env("NPM_CONFIG_LOGS_MAX", "0")
            .env("NPM_CONFIG_REGISTRY", self.registry)
    }

    async fn exists(&self, ctx: &TaskContext, expected: &str) -> Result<bool, TaskError> {
        let mut file = tempfile::tempfile().map_err(TaskError::operation)?;
        let output = Arc::new(Output(Some(Mutex::new(
            file.try_clone().map_err(TaskError::operation)?,
        ))));
        let result = ctx
            .exec_with_output(
                self.command().args([
                    "view",
                    &std::format!("{NAME}@{VERSION}"),
                    "dist.integrity",
                    "--json",
                    "--prefer-online",
                ]),
                output,
            )
            .await;
        file.rewind().map_err(TaskError::operation)?;
        let data: serde_json::Value =
            serde_json::from_reader(BufReader::new(file)).unwrap_or(serde_json::Value::Null);
        match result {
            Ok(()) if data.as_str() == Some(expected) => Ok(true),
            Ok(()) => Err(TaskError::operation(
                "The registry version has different integrity",
            )),
            Err(TaskError::Exit { .. }) if data["error"]["code"] == "E404" => Ok(false),
            Err(e) => Err(e),
        }
    }
}
async fn publish(ctx: TaskContext, args: Release) -> TaskResult {
    let registry = args.registry.trim_end_matches('/');
    if registry != "https://registry.npmjs.org"
        && !registry
            .strip_prefix("http://")
            .and_then(|v| v.parse::<SocketAddr>().ok())
            .is_some_and(|addr| addr.ip().is_loopback())
    {
        return Err(TaskError::operation(
            "Expected the npm registry or a loopback test registry",
        ));
    }
    if args.tag != std::format!("foundation_wasm_ui-v{VERSION}")
        || args.commit.len() != 40
        || !args.commit.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(TaskError::operation(
            "The release tag and commit must identify this crate version",
        ));
    }
    let tmp = tempfile::tempdir().map_err(TaskError::operation)?;
    let archive = tmp.path().join("release.tgz");
    fs::copy(&args.archive, &archive).map_err(TaskError::operation)?;
    if integrity(&archive).map_err(TaskError::operation)? != args.integrity {
        return Err(TaskError::operation(
            "The archive differs from the tested SHA-512 integrity",
        ));
    }
    let archive_file = fs::File::open(&archive).map_err(TaskError::operation)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive_file));
    let mut manifest = None;
    for entry in tar.entries().map_err(TaskError::operation)? {
        let item = entry.map_err(TaskError::operation)?;
        if item.path().map_err(TaskError::operation)?.as_ref() == Path::new("package/package.json")
        {
            if manifest.is_some() {
                return Err(TaskError::operation("Duplicate package manifest"));
            }
            manifest = Some(
                serde_json::from_reader::<_, serde_json::Value>(item)
                    .map_err(TaskError::operation)?,
            );
        }
    }
    let data = manifest.ok_or_else(|| TaskError::operation("Missing npm manifest"))?;
    if data["name"] != NAME || data["version"] != VERSION {
        return Err(TaskError::operation("Unexpected npm package identity"));
    }
    let host_env = ctx.require::<super::BrowserTaskEnv>()?;
    let client = Client {
        host: &host_env,
        dir: tmp.path(),
        registry,
    };
    let host = registry
        .split_once("://")
        .ok_or_else(|| TaskError::operation("Invalid registry"))?
        .1;
    let conf = tmp.path().join(".npmrc");
    let scope = std::format!("@ewe-studios:registry={registry}/\n");
    fs::write(&conf, &scope).map_err(TaskError::operation)?;
    let value = token(
        (host_env.0)("NPM_TOKEN").and_then(|v| v.into_string().ok()),
        (host_env.0)(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from),
    )?;
    if !client.exists(&ctx, &args.integrity).await? {
        // npm expands this reference only in the credentialed child process.
        fs::write(
            &conf,
            std::format!("{scope}//{host}/:_authToken=${{NPM_TOKEN}}\n"),
        )
        .map_err(TaskError::operation)?;
        let output = Arc::new(Output(None));
        let result = ctx
            .exec_with_output(
                client
                    .command()
                    .args(["publish", "--ignore-scripts", "--access", "public"])
                    .arg(&archive)
                    .env("NPM_TOKEN", value),
                output,
            )
            .await;
        fs::write(&conf, &scope).map_err(TaskError::operation)?;
        if !client.exists(&ctx, &args.integrity).await? {
            result?;
            return Err(TaskError::operation(
                "The registry did not confirm the published integrity",
            ));
        }
    }
    std::println!(
        "{} {} {} {}",
        args.tag,
        args.commit,
        VERSION,
        args.integrity
    );
    Ok(())
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "npm-publish",
        "Publish the verified browser archive or confirm a matching release",
        publish,
    )?;
    set.expose("npm-publish", &["wasm-bundle", "npm-publish"])
}
