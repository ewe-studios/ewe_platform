//! Publish a signed release and reconcile interrupted Maven Central deployments.
use super::{
    output::{query, query_status},
    sign, stage, swift, NativeBuildEnv,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone)]
pub(super) struct Credentials {
    fingerprint: String,
    header: Vec<u8>,
}
impl Credentials {
    pub(super) fn from_host() -> Option<Self> {
        let fingerprint = std::env::var("NATIVE_GPG_FINGERPRINT").ok()?;
        let name = std::env::var("CENTRAL_USERNAME").ok()?;
        let password = std::env::var("CENTRAL_PASSWORD").ok()?;
        Some(Self::new(fingerprint, &name, &password))
    }
    pub(super) fn new(fingerprint: String, user: &str, password: &str) -> Self {
        Self {
            fingerprint,
            header: format!(
                "Authorization: Bearer {}\n",
                STANDARD.encode(format!("{user}:{password}"))
            )
            .into_bytes(),
        }
    }
}

struct Registry<'a> {
    ctx: &'a TaskContext,
    repo: &'a str,
    tag: &'a str,
    credentials: &'a Credentials,
}

pub(super) async fn github_lookup(
    ctx: &TaskContext,
    route: &str,
    select: &str,
) -> Result<Option<Value>, TaskError> {
    let cmd = ProcessCommand::new("gh").args(["api", "--include", route, "--jq", select]);
    let (status, bytes) = query_status(ctx, cmd).await?;
    let data = std::str::from_utf8(&bytes).map_err(TaskError::operation)?;
    let data = data.replace("\r\n", "\n");
    let (head, body) = data
        .split_once("\n\n")
        .ok_or_else(|| TaskError::operation("Missing GitHub response headers"))?;
    let code = head
        .lines()
        .next()
        .and_then(|x| x.split_whitespace().nth(1));
    if code == Some("404") && matches!(status, Err(TaskError::Exit { .. })) {
        return Ok(None);
    }
    status?;
    Ok(Some(
        serde_json::from_str(body.trim()).map_err(TaskError::operation)?,
    ))
}

impl Registry<'_> {
    async fn lookup(&self, path: &str, select: &str) -> Result<Option<Value>, TaskError> {
        let route = format!("repos/{}/{path}", self.repo);
        github_lookup(self.ctx, &route, select).await
    }
    async fn tag_matches(&self, name: &str, sha: &str) -> Result<bool, TaskError> {
        let Some(mut object) = self
            .lookup(&format!("git/ref/tags/{name}"), ".object")
            .await?
        else {
            return Ok(false);
        };
        while object["type"] == "tag" {
            let target = field(&object, "sha")?;
            object = self
                .lookup(&format!("git/tags/{target}"), ".object")
                .await?
                .ok_or_else(|| TaskError::operation("Missing annotated tag"))?;
        }
        if object["type"] != "commit" || object["sha"] != sha {
            return Err(TaskError::operation(format!("Tag conflict for {name}")));
        }
        Ok(true)
    }
    async fn add_tag(&self, name: &str, sha: &str) -> TaskResult {
        if !self.tag_matches(name, sha).await? {
            self.ctx
                .exec(ProcessCommand::new("gh").args([
                    "api",
                    "--silent",
                    "--method",
                    "POST",
                    &format!("repos/{}/git/refs", self.repo),
                    "-f",
                    &format!("ref=refs/tags/{name}"),
                    "-f",
                    &format!("sha={sha}"),
                ]))
                .await?;
        }
        Ok(())
    }
    fn release(&self, verb: &str) -> ProcessCommand {
        ProcessCommand::new("gh").args(["release", verb, self.tag, "--repo", self.repo])
    }
    async fn release_info(&self) -> Result<Option<Value>, TaskError> {
        self.lookup(
            &format!("releases/tags/{}", self.tag),
            "{draft, assets:[.assets[]|{name,digest}]}",
        )
        .await
    }
    async fn upload(&self, file: &Path, assets: &BTreeMap<String, String>) -> TaskResult {
        let name = file
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| TaskError::operation("Invalid asset filename"))?;
        if let Some(saved) = assets.get(name) {
            if saved != &format!("sha256:{}", stage::digest(file).await?) {
                return Err(TaskError::operation(format!("Asset conflict: {name}")));
            }
        } else {
            self.ctx.exec(self.release("upload").arg(file)).await?;
        }
        Ok(())
    }
    async fn download(&self, name: &str, directory: &Path) -> TaskResult {
        self.ctx
            .exec(
                self.release("download")
                    .args(["--pattern", name, "--dir"])
                    .arg(directory),
            )
            .await
    }
    async fn central(&self, operation: &str, form: Option<&Path>) -> Result<String, TaskError> {
        let mut cmd = ProcessCommand::new("curl").args([
            "--fail",
            "--silent",
            "--show-error",
            "--request",
            "POST",
            "--header",
            "@-",
        ]);
        if let Some(path) = form {
            // Curl treats quotes and semicolons as syntax in multipart file paths.
            let path = path
                .to_str()
                .ok_or_else(|| TaskError::operation("The Maven bundle path requires Unicode"))?
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            cmd = cmd.arg("--form").arg(format!("bundle=@\"{path}\""));
        }
        let bytes = query(
            self.ctx,
            cmd.arg(format!(
                "https://central.sonatype.com/api/v1/publisher/{operation}"
            ))
            .private_input(self.credentials.header.clone()),
        )
        .await?;
        Ok(String::from_utf8(bytes)
            .map_err(TaskError::operation)?
            .trim()
            .to_owned())
    }
    async fn status(&self, id: &str, name: &str) -> Result<String, TaskError> {
        let data = self.central(&format!("status?id={id}"), None).await?;
        let value: Value = serde_json::from_str(&data).map_err(TaskError::operation)?;
        if value["deploymentName"] != name {
            return Err(TaskError::operation(
                "Central deployment name differs from the receipt",
            ));
        }
        Ok(field(&value, "deploymentState")?.into())
    }
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, TaskError> {
    value[key]
        .as_str()
        .ok_or_else(|| TaskError::operation(format!("Missing release field {key}")))
}
fn read(path: &Path) -> Result<Value, TaskError> {
    serde_json::from_reader(File::open(path).map_err(TaskError::operation)?)
        .map_err(TaskError::operation)
}
fn store(path: &Path, value: &Value) -> TaskResult {
    let mut stage = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| TaskError::operation("Missing receipt directory"))?,
    )
    .map_err(TaskError::operation)?;
    serde_json::to_writer(&mut stage, value).map_err(TaskError::operation)?;
    stage.write_all(b"\n").map_err(TaskError::operation)?;
    stage.as_file().sync_all().map_err(TaskError::operation)?;
    stage.persist(path).map_err(TaskError::operation)?;
    Ok(())
}
fn receipt(path: &Path, name: &str) -> Result<String, TaskError> {
    let data = read(path)?;
    if data["name"] != name {
        return Err(TaskError::operation(
            "Central receipt belongs to another candidate",
        ));
    }
    Ok(uuid::Uuid::parse_str(field(&data, "id")?)
        .map_err(TaskError::operation)?
        .to_string())
}
fn assets(info: Option<&Value>) -> Result<BTreeMap<String, String>, TaskError> {
    let Some(info) = info else {
        return Ok(BTreeMap::new());
    };
    info["assets"]
        .as_array()
        .ok_or_else(|| TaskError::operation("Missing GitHub assets"))?
        .iter()
        .map(|a| Ok((field(a, "name")?.to_owned(), field(a, "digest")?.to_owned())))
        .collect()
}

struct Recovery {
    name: String,
    local: PathBuf,
    pending: PathBuf,
    remote: PathBuf,
}
impl Recovery {
    async fn reconcile(
        &self,
        reg: &Registry<'_>,
        assets: &BTreeMap<String, String>,
        temp: &Path,
    ) -> Result<Option<String>, TaskError> {
        let mut id = if self.local.exists() {
            Some(receipt(&self.local, &self.name)?)
        } else {
            None
        };
        if assets.contains_key("central-deployment.json") {
            reg.download("central-deployment.json", temp).await?;
            let remote = receipt(&self.remote, &self.name)?;
            if id.as_ref().is_some_and(|local| local != &remote) {
                return Err(TaskError::operation(
                    "Local and remote deployment receipts differ",
                ));
            }
            id = Some(remote);
        }
        if id.is_none()
            && (self.pending.exists() || assets.contains_key("central-deployment-pending.json"))
        {
            if !self.pending.exists() {
                reg.download("central-deployment-pending.json", temp)
                    .await?;
                let attempt = read(&temp.join("central-deployment-pending.json"))?;
                if attempt["name"] != self.name {
                    return Err(TaskError::operation(
                        "Central upload attempt belongs to another candidate",
                    ));
                }
                store(&self.pending, &attempt)?;
            }
            return Err(TaskError::operation(format!(
                "Central upload outcome is unresolved. Inspect {}",
                self.pending.display()
            )));
        }
        Ok(id)
    }
}

pub(super) async fn run(
    ctx: &TaskContext,
    root: &Path,
    dir: &Path,
    timeout: u64,
    interval: u64,
    public_key: Option<&Path>,
) -> TaskResult {
    let cfg = stage::config(root)?;
    let manifest = dir.join("native-ui.json");
    let rec = read(&manifest)?;
    let source = field(&rec, "source_revision")?;
    let swift_id = field(&rec, "swift_tag_revision")?;
    if rec["version"] != cfg.version || rec["repositories"] != json!({"repository":cfg.repository})
    {
        return Err(TaskError::operation(
            "Release configuration differs from the candidate",
        ));
    }
    let env = ctx.require::<NativeBuildEnv>()?;
    let credentials = env
        .release_credentials
        .as_ref()
        .filter(|c| !c.fingerprint.is_empty())
        .ok_or_else(|| {
            TaskError::operation("Configure the trusted fingerprint and Central credentials")
        })?;
    let android = dir.join(sign::ANDROID);
    verify_signatures(ctx, &android, &credentials.fingerprint, public_key).await?;
    let tag = format!("native-ui-v{}", cfg.version);
    let reg = Registry {
        ctx,
        repo: &cfg.repository,
        tag: &tag,
        credentials,
    };
    reg.tag_matches(&tag, source).await?;
    reg.tag_matches(&cfg.version, swift_id).await?;
    if reg
        .lookup(&format!("git/commits/{source}"), "{sha}")
        .await?
        .is_none()
    {
        return Err(TaskError::operation("Missing source commit"));
    }
    let mut info = reg.release_info().await?;
    let mut known = assets(info.as_ref())?;
    let files = [
        android.clone(),
        dir.join(&cfg.apple_archive),
        manifest.clone(),
    ];
    for file in &files {
        if file
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| known.contains_key(n))
        {
            reg.upload(file, &known).await?;
        }
    }
    let temp = tempfile::tempdir().map_err(TaskError::operation)?;
    let name = format!("{tag}-{}", stage::digest(&manifest).await?);
    let recovery = Recovery {
        name,
        local: dir.join(".central-deployment.json"),
        pending: dir.join(".central-deployment-pending.json"),
        remote: temp.path().join("central-deployment.json"),
    };
    let existing = recovery.reconcile(&reg, &known, temp.path()).await?;
    let already = if let Some(id) = &existing {
        reg.status(id, &recovery.name).await? == "PUBLISHED"
    } else {
        false
    };
    if info.is_none() {
        reg.add_tag(&tag, source).await?;
        ctx.exec(reg.release("create").args([
            "--verify-tag",
            "--draft",
            "--title",
            &tag,
            "--notes",
            "Native user interface packages.",
        ]))
        .await?;
        info = Some(
            reg.release_info()
                .await?
                .ok_or_else(|| TaskError::operation("GitHub did not return the draft release"))?,
        );
        known = assets(info.as_ref())?;
    }
    for file in files {
        reg.upload(&file, &known).await?;
    }
    let id = central_deployment(&reg, &recovery, temp.path(), &known, existing, &android).await?;
    if !already {
        wait_for_publication(&reg, &id, &recovery.name, timeout, interval).await?;
    }
    if info.as_ref().and_then(|v| v["draft"].as_bool()) == Some(true) {
        ctx.exec(reg.release("edit").arg("--draft=false")).await?;
    }
    publish_swift_tag(&reg, &cfg.version, swift_id, source, &rec).await
}

async fn verify_signatures(
    ctx: &TaskContext,
    android: &Path,
    fingerprint: &str,
    public_key: Option<&Path>,
) -> TaskResult {
    let keyring = if let Some(key) = public_key {
        let dir = tempfile::tempdir().map_err(TaskError::operation)?;
        ctx.exec(
            sign::gpg(Some(dir.path()))
                .args(["--batch", "--import"])
                .arg(key),
        )
        .await?;
        Some(dir)
    } else {
        None
    };
    sign::verified(
        ctx,
        android,
        fingerprint,
        keyring.as_ref().map(tempfile::TempDir::path),
    )
    .await
}

async fn central_deployment(
    reg: &Registry<'_>,
    recovery: &Recovery,
    temp: &Path,
    known: &BTreeMap<String, String>,
    existing: Option<String>,
    android: &Path,
) -> Result<String, TaskError> {
    let pending_asset = "central-deployment-pending.json";
    let mut pending_remote = known.contains_key(pending_asset);
    let id = if let Some(id) = existing {
        id
    } else {
        let record = json!({"name":recovery.name});
        store(&recovery.pending, &record)?;
        let remote = temp.join(pending_asset);
        store(&remote, &record)?;
        reg.upload(&remote, known).await?;
        pending_remote = true;
        let id = reg
            .central(
                &format!("upload?name={}&publishingType=USER_MANAGED", recovery.name),
                Some(android),
            )
            .await
            .map_err(|_| {
                TaskError::operation(format!(
                    "Central upload outcome is unresolved. Inspect {}",
                    recovery.pending.display()
                ))
            })?;
        uuid::Uuid::parse_str(&id)
            .map_err(TaskError::operation)?
            .to_string()
    };
    if !known.contains_key("central-deployment.json") {
        let saved = json!({"name":recovery.name,"id":id});
        store(&recovery.local, &saved)?;
        store(&recovery.remote, &saved)?;
        reg.upload(&recovery.remote, known).await?;
    }
    if pending_remote {
        reg.ctx
            .exec(reg.release("delete-asset").args([pending_asset, "--yes"]))
            .await?;
    }
    if recovery.pending.exists() {
        fs::remove_file(&recovery.pending).map_err(TaskError::operation)?;
    }
    Ok(id)
}

async fn wait_for_publication(
    reg: &Registry<'_>,
    id: &str,
    name: &str,
    timeout: u64,
    interval: u64,
) -> TaskResult {
    let clock = reg.ctx.context().clock();
    let start = clock.elapsed();
    loop {
        if clock.elapsed().saturating_sub(start) >= Duration::from_secs(timeout) {
            return Err(TaskError::operation(
                "Central deployment is still pending. Resume this candidate.",
            ));
        }
        match reg.status(id, name).await?.as_str() {
            "PUBLISHED" => break,
            "VALIDATED" => {
                reg.central(&format!("deployment/{id}"), None).await?;
            }
            "PENDING" | "VALIDATING" | "PUBLISHING" => {}
            state => {
                return Err(TaskError::operation(format!(
                    "Unexpected Central deployment state: {state}"
                )))
            }
        }
        super::device::sleep(reg.ctx, Duration::from_secs(interval)).await?;
    }
    Ok(())
}

async fn publish_swift_tag(
    reg: &Registry<'_>,
    version: &str,
    swift_id: &str,
    source: &str,
    rec: &Value,
) -> TaskResult {
    if swift_id != source && !reg.tag_matches(version, swift_id).await? {
        let clone = tempfile::tempdir().map_err(TaskError::operation)?;
        reg.ctx
            .exec(
                ProcessCommand::new("git")
                    .args(["clone", "--quiet", "--no-checkout"])
                    .arg(format!("https://github.com/{}.git", reg.repo))
                    .arg(clone.path()),
            )
            .await?;
        if swift::commit(reg.ctx, clone.path(), source, field(rec, "swift_manifest")?).await?
            != swift_id
        {
            return Err(TaskError::operation(
                "Swift release metadata differs from the staged commit",
            ));
        }
        reg.ctx
            .exec(ProcessCommand::new("git").current_dir(clone.path()).args([
                "-c",
                "credential.helper=",
                "-c",
                "credential.helper=!gh auth git-credential",
                "push",
                "origin",
                &format!("{swift_id}:refs/tags/{version}"),
            ]))
            .await?;
    } else {
        reg.add_tag(version, swift_id).await?;
    }
    Ok(())
}
