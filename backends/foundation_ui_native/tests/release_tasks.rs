//! Exercise publication with local artifacts and a registry double.
use foundation_context::Ctx;
use foundation_tasks::{
    Cancellation, OutputStream, ProcessCommand, TaskEnv, TaskError, TaskOutput, TaskProcess,
    TaskResult, TaskRunner,
};
use foundation_ui_native::tasks::{self, NativeBuildEnv};
use futures_util::future::BoxFuture;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use zip::{write::SimpleFileOptions, ZipWriter};

const SIGNER: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ID: &str = "46a8097f-a23d-4974-a0c7-c935abf36660";
const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Default)]
struct Quiet;
impl TaskOutput for Quiet {
    fn write<'a>(&'a self, _: OutputStream, _: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}
#[derive(Default)]
struct Remote {
    assets: BTreeMap<String, String>,
    states: VecDeque<String>,
    operations: Vec<String>,
    receipt: Value,
    signer: String,
    ambiguous: bool,
    changed_tag: bool,
    hidden_draft: bool,
    absent_draft: bool,
    lookup_denied: bool,
}
impl Remote {
    fn release(&self) -> Value {
        let assets = self
            .assets
            .iter()
            .map(|(name, digest)| json!({"name": name, "digest": digest}));
        json!({"draft": true, "assets": assets.collect::<Vec<_>>()})
    }
}

#[derive(Default)]
struct Registry(Mutex<Remote>);
impl TaskProcess for Registry {
    fn run(
        &self,
        cmd: ProcessCommand,
        out: Arc<dyn TaskOutput>,
        _: Cancellation,
    ) -> BoxFuture<'_, TaskResult> {
        Box::pin(async move {
            let args = cmd
                .args
                .iter()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>();
            assert!(args.iter().all(|s| !s.contains("dGVzdDpwYXNz")));
            let mut exit = Ok(());
            let response = {
                let mut remote = self.0.lock().unwrap();
                let op = cmd.program.to_str().unwrap();
                match (op, args.as_slice()) {
                    ("gpg", ["--batch", "--status-fd", "1", "--verify", sig, product]) => {
                        assert_eq!(fs::read(sig).unwrap(), b"signature");
                        assert_eq!(fs::read(product).unwrap(), b"package");
                        format!("[GNUPG:] VALIDSIG {} signer\n", remote.signer)
                    }
                    ("gh", ["api", "--include", route, "--jq", _]) => {
                        let mut status = "200 OK";
                        let object = if route.contains("git/ref/tags/") {
                            json!({"type":"commit","sha":if remote.changed_tag { "changed" } else { REVISION }})
                        } else if route.contains("git/commits/") {
                            json!({"sha":REVISION})
                        } else if route.contains("releases/tags/") {
                            if remote.hidden_draft || remote.lookup_denied || remote.absent_draft {
                                status = if remote.lookup_denied {
                                    "403 Forbidden"
                                } else {
                                    "404 Not Found"
                                };
                                exit = Err(TaskError::Exit {
                                    program: "gh".into(),
                                    code: 1,
                                });
                                json!({})
                            } else {
                                remote.release()
                            }
                        } else {
                            panic!("Unknown GitHub lookup {route}")
                        };
                        format!("HTTP/2.0 {status}\r\nX-Test: local\r\n\r\n{object}\n")
                    }
                    ("gh", ["api", "--paginate", route, "--jq", filter]) => {
                        assert_eq!(*route, "repos/demo/project/releases?per_page=100");
                        assert!(filter.contains("select(.tag_name == \"native-ui-v1.2.3\")"));
                        if remote.absent_draft {
                            String::new()
                        } else {
                            remote.release().to_string()
                        }
                    }
                    (
                        "gh",
                        ["release", "create", "native-ui-v1.2.3", "--repo", "demo/project", ..],
                    ) => {
                        assert!(
                            remote.absent_draft,
                            "An existing draft must not be recreated"
                        );
                        remote.absent_draft = false;
                        remote.hidden_draft = true;
                        remote.operations.push("create draft".into());
                        String::new()
                    }
                    (
                        "gh",
                        ["release", "download", _, "--repo", _, "--pattern", file, "--dir", dir],
                    ) => {
                        fs::write(
                            Path::new(dir).join(file),
                            serde_json::to_vec(&remote.receipt).unwrap(),
                        )
                        .unwrap();
                        remote.operations.push(format!("download {file}"));
                        String::new()
                    }
                    ("gh", ["release", "upload", _, "--repo", _, file]) => {
                        let name = Path::new(file).file_name().unwrap().to_str().unwrap();
                        let digest = hash(&fs::read(file).unwrap());
                        remote
                            .assets
                            .insert(name.into(), format!("sha256:{digest}"));
                        remote.operations.push(format!("upload {name}"));
                        String::new()
                    }
                    ("gh", ["release", "delete-asset", _, "--repo", _, file, "--yes"]) => {
                        remote.assets.remove(*file);
                        remote.operations.push(format!("delete {file}"));
                        String::new()
                    }
                    ("gh", ["release", "edit", _, "--repo", _, "--draft=false"]) => {
                        remote.operations.push("release".into());
                        String::new()
                    }
                    ("curl", _) => {
                        let operation = args
                            .last()
                            .unwrap()
                            .strip_prefix("https://central.sonatype.com/api/v1/publisher/")
                            .unwrap();
                        remote.operations.push(operation.into());
                        if operation.starts_with("status?") {
                            json!({"deploymentName":remote.receipt["name"],"deploymentState":remote.states.pop_front().expect("Unexpected poll")}).to_string()
                        } else if operation.starts_with("upload?") {
                            if remote.ambiguous {
                                return Err(TaskError::operation("Response interrupted"));
                            }
                            ID.into()
                        } else if operation == format!("deployment/{ID}") {
                            String::new()
                        } else {
                            panic!("Unexpected Central request")
                        }
                    }
                    ("sleep", _) => String::new(),
                    _ => panic!("Unexpected task tool {op}: {args:?}"),
                }
            };
            out.write(OutputStream::Stdout, response.as_bytes())
                .await
                .map_err(TaskError::operation)?;
            exit
        })
    }
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
struct ImmediateTimer;
impl foundation_context::env::Timer for ImmediateTimer {
    fn sleep_until(
        &self,
        _: Ctx,
        _: foundation_context::Instant,
    ) -> foundation_context::env::TimerFuture {
        Box::pin(async {})
    }
}
struct Fixture {
    root: tempfile::TempDir,
    candidate: PathBuf,
    remote: Arc<Registry>,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Cargo.toml"), "").unwrap();
        fs::create_dir(root.path().join("release")).unwrap();
        fs::write(
            root.path().join("release/native-ui.toml"),
            r#"version="1.2.3"
repository="demo/project"
apple_archive="EweCoreFFI.xcframework.zip"
maven_group="example"
maven_modules=["sample"]
android_targets=["arm64-v8a"]
swift_product="EweUI"
"#,
        )
        .unwrap();
        let candidate = root.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        fs::write(candidate.join("EweCoreFFI.xcframework.zip"), b"apple").unwrap();
        let android = candidate.join("android-publication.zip");
        let mut writer = ZipWriter::new(File::create(&android).unwrap());
        for (name, bytes) in [
            ("sample.pom", &b"package"[..]),
            ("sample.pom.asc", &b"signature"[..]),
        ] {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        let sums = BTreeMap::from([
            ("EweCoreFFI.xcframework.zip", hash(b"apple")),
            ("android-publication.zip", hash(&fs::read(android).unwrap())),
        ]);
        let record = json!({"version":"1.2.3","repositories":{"repository":"demo/project"},"source_revision":REVISION,"swift_tag_revision":REVISION,"central_signed":true,"sha256":sums});
        let manifest = serde_json::to_vec(&record).unwrap();
        fs::write(candidate.join("native-ui.json"), &manifest).unwrap();
        let name = format!("native-ui-v1.2.3-{}", hash(&manifest));
        let mut assets = sums
            .into_iter()
            .map(|(k, v)| (k.into(), format!("sha256:{v}")))
            .collect::<BTreeMap<_, _>>();
        assets.insert(
            "native-ui.json".into(),
            format!("sha256:{}", hash(&manifest)),
        );
        let remote = Arc::new(Registry(Mutex::new(Remote {
            assets,
            receipt: json!({"name":name,"id":ID}),
            signer: SIGNER.into(),
            ..Remote::default()
        })));
        Self {
            root,
            candidate,
            remote,
        }
    }
    fn run(&self) -> TaskResult {
        let ctx = Ctx::host();
        let env = foundation_core::env::simulated(&ctx, 17)
            .unwrap()
            .with_timer(Arc::new(ImmediateTimer));
        ctx.store(env);
        ctx.store(
            NativeBuildEnv::new(self.root.path().into()).with_publication_credentials(
                SIGNER.into(),
                "test",
                "pass",
            ),
        );
        ctx.store(TaskEnv {
            process: self.remote.clone(),
            output: Arc::new(Quiet),
            cancellation: Cancellation::new(),
        });
        let mut runner = TaskRunner::new(
            ctx,
            [
                "native-task",
                "task",
                "run",
                "native:release:publish",
                "--candidate",
                "candidate",
                "--resume",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
        );
        runner.register(tasks::set()?)?;
        futures_lite::future::block_on(runner.run())
    }
    fn resume(&self, states: &[&str]) {
        let mut remote = self.remote.0.lock().unwrap();
        remote
            .assets
            .insert("central-deployment.json".into(), "fixture".into());
        remote.states.extend(states.iter().map(ToString::to_string));
    }
}

#[test]
fn published_receipt_avoids_a_second_upload() {
    let f = Fixture::new();
    f.resume(&["PUBLISHED"]);
    f.run().unwrap();
    assert_eq!(
        f.remote.0.lock().unwrap().operations,
        [
            "download central-deployment.json",
            &format!("status?id={ID}"),
            "release"
        ]
    );
}
#[test]
fn a_draft_hidden_from_tag_lookup_reuses_its_published_receipt() {
    let f = Fixture::new();
    f.remote.0.lock().unwrap().hidden_draft = true;
    f.resume(&["PUBLISHED"]);
    f.run().unwrap();
    assert_eq!(
        f.remote.0.lock().unwrap().operations,
        [
            "download central-deployment.json",
            &format!("status?id={ID}"),
            "release"
        ]
    );
}

#[test]
fn a_new_draft_is_found_after_creation() {
    let f = Fixture::new();
    let mut remote = f.remote.0.lock().unwrap();
    remote.absent_draft = true;
    remote.assets.clear();
    remote
        .states
        .extend(["VALIDATED".into(), "PUBLISHED".into()]);
    drop(remote);
    f.run().unwrap();
    let remote = f.remote.0.lock().unwrap();
    assert_eq!(remote.operations.first().unwrap(), "create draft");
    assert_eq!(remote.operations.last().unwrap(), "release");
    assert!(remote.assets.contains_key("native-ui.json"));
    assert!(remote.assets.contains_key("central-deployment.json"));
}

#[test]
fn denied_tag_lookup_stops_before_fallback_and_writes() {
    let f = Fixture::new();
    f.remote.0.lock().unwrap().lookup_denied = true;
    assert!(f.run().is_err());
    assert!(f.remote.0.lock().unwrap().operations.is_empty());
}

#[test]
fn validated_receipt_publishes_once() {
    let f = Fixture::new();
    f.resume(&["VALIDATING", "VALIDATED", "PUBLISHED"]);
    f.run().unwrap();
    let remote = f.remote.0.lock().unwrap();
    assert_eq!(
        remote
            .operations
            .iter()
            .filter(|s| s.starts_with("deployment/"))
            .count(),
        1
    );
    assert!(!remote.operations.iter().any(|s| s.starts_with("upload")));
}
#[test]
fn changed_release_tag_prevents_remote_writes() {
    let f = Fixture::new();
    f.remote.0.lock().unwrap().changed_tag = true;
    assert!(f.run().unwrap_err().to_string().contains("Tag conflict"));
    assert_eq!(f.remote.0.lock().unwrap().operations, Vec::<String>::new());
}

#[test]
fn untrusted_signer_and_remote_conflict_prevent_writes() {
    let f = Fixture::new();
    f.remote.0.lock().unwrap().signer = "untrusted".into();
    assert!(f
        .run()
        .unwrap_err()
        .to_string()
        .contains("Unexpected signer"));
    assert_eq!(f.remote.0.lock().unwrap().operations, Vec::<String>::new());
    let mut remote = f.remote.0.lock().unwrap();
    remote.signer = SIGNER.into();
    remote
        .assets
        .insert("native-ui.json".into(), "sha256:wrong".into());
    drop(remote);
    assert!(f.run().unwrap_err().to_string().contains("Asset conflict"));
    assert_eq!(f.remote.0.lock().unwrap().operations, Vec::<String>::new());
}
#[test]
fn ambiguous_upload_requires_a_receipt_before_retry() {
    let f = Fixture::new();
    f.remote.0.lock().unwrap().ambiguous = true;
    assert!(f.run().unwrap_err().to_string().contains("unresolved"));
    assert!(f
        .candidate
        .join(".central-deployment-pending.json")
        .exists());
    assert!(f.run().unwrap_err().to_string().contains("unresolved"));
    assert_eq!(
        f.remote
            .0
            .lock()
            .unwrap()
            .operations
            .iter()
            .filter(|s| s.starts_with("upload?"))
            .count(),
        1
    );
}
#[test]
fn new_deployment_records_its_receipt_before_releasing() {
    let f = Fixture::new();
    f.remote
        .0
        .lock()
        .unwrap()
        .states
        .push_back("PUBLISHED".into());
    f.run().unwrap();
    let local: Value =
        serde_json::from_reader(File::open(f.candidate.join(".central-deployment.json")).unwrap())
            .unwrap();
    assert_eq!(local["id"], ID);
    assert!(!f
        .candidate
        .join(".central-deployment-pending.json")
        .exists());
    let remote = f.remote.0.lock().unwrap();
    let position = |op: &str| remote.operations.iter().position(|x| x == op).unwrap();
    assert!(
        position("upload central-deployment.json")
            < position("delete central-deployment-pending.json")
    );
    assert!(position("delete central-deployment-pending.json") < position("release"));
}
