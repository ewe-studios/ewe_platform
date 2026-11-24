use foundation_context::Ctx;
use foundation_tasks::{
    Cancellation, OutputStream, ProcessCommand, TaskEnv, TaskError, TaskOutput, TaskProcess,
    TaskResult, TaskRunner,
};
use foundation_ui_native::tasks::{self, NativeBuildEnv};
use futures_util::future::BoxFuture;
use sha2::{Digest, Sha256};
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Capture(Mutex<Vec<u8>>);

impl TaskOutput for Capture {
    fn write<'a>(&'a self, _: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(())
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct FakeProcess(Mutex<Vec<ProcessCommand>>, bool);

impl TaskProcess for FakeProcess {
    fn run(
        &self,
        command: ProcessCommand,
        output: Arc<dyn TaskOutput>,
        _: Cancellation,
    ) -> BoxFuture<'_, TaskResult> {
        Box::pin(async move {
            if self.1 {
                if command.program == "mise" && command.args.first().is_some_and(|arg| arg == "env")
                {
                    output.write(OutputStream::Stdout, br#"{"PATH":"/tools","ANDROID_HOME":"/android home","ANDROID_NDK":"123"}"#)
                        .await.map_err(TaskError::operation)?;
                }
                if command.program == "cargo" {
                    let after = |flag| {
                        command
                            .args
                            .iter()
                            .skip_while(|arg| *arg != flag)
                            .nth(1)
                            .expect("Cargo argument")
                    };
                    let directory = std::path::PathBuf::from(after("-o"));
                    let package = after("-p").to_string_lossy();
                    for abi in ["arm64-v8a", "x86_64"] {
                        let abi = directory.join(abi);
                        std::fs::create_dir_all(&abi).map_err(TaskError::operation)?;
                        std::fs::write(abi.join(format!("lib{package}.so")), package.as_bytes())
                            .map_err(TaskError::operation)?;
                    }
                }
            }
            self.0.lock().unwrap().push(command);
            Ok(())
        })
    }
}

fn command(
    root: &Path,
    argv: &[&str],
    process: Arc<FakeProcess>,
    output: Arc<Capture>,
) -> TaskResult {
    let ctx = Ctx::host();
    ctx.store(
        NativeBuildEnv::new(root.into())
            .with_host_directories(root.as_os_str().to_owned(), root.join("temporary files")),
    );
    ctx.store(TaskEnv {
        process,
        output,
        cancellation: Cancellation::new(),
    });
    let mut runner = TaskRunner::new(ctx, argv.iter().map(Into::into).collect());
    runner.register(tasks::set()?)?;
    futures_lite::future::block_on(runner.run())
}

#[test]
fn list_and_help_do_not_find_release_inputs_or_start_tools() {
    let root = tempfile::tempdir().unwrap();
    let process = Arc::new(FakeProcess::default());
    let output = Arc::new(Capture::default());
    command(
        root.path(),
        &["native-task", "task", "ls"],
        process.clone(),
        output.clone(),
    )
    .unwrap();
    let bytes = output.0.lock().unwrap();
    let listing = String::from_utf8_lossy(&bytes);
    let names = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(names.contains(&"native:release:publish"));
    drop(bytes);
    command(
        root.path(),
        &[
            "native-task",
            "task",
            "run",
            "native:release:publish",
            "--help",
        ],
        process.clone(),
        output,
    )
    .unwrap();
    assert!(process.0.lock().unwrap().is_empty());
}

#[test]
fn verify_checks_the_candidate_before_starting_tools() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("release")).unwrap();
    std::fs::write(
        root.path().join("release/native-ui.toml"),
        r#"
[consumers]
apple_directory = "fixtures/consumer/ios"
apple_project = "Starter.xcodeproj"
apple_scheme = "Starter"
android_directory = "fixtures/consumer/android"
android_task = ":app:assembleDebug"
"#,
    )
    .unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "").unwrap();
    let candidate = root.path().join("candidate with spaces");
    std::fs::create_dir(&candidate).unwrap();
    let checksum = hex::encode(Sha256::digest(b"complete"));
    std::fs::write(
        candidate.join("native-ui.json"),
        serde_json::json!({
            "source_revision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256": {"android-publication.zip": checksum, "EweCoreFFI.xcframework.zip": checksum }
        })
        .to_string(),
    )
    .unwrap();
    for name in ["EweCoreFFI.xcframework.zip", "android-publication.zip"] {
        std::fs::write(candidate.join(name), b"complete").unwrap();
    }
    let process = Arc::new(FakeProcess::default());
    let output = Arc::new(Capture::default());
    let verify = [
        "native-task",
        "task",
        "run",
        "native:release:verify",
        "--candidate",
        "candidate with spaces",
        "--templates",
        "android",
    ];
    command(root.path(), &verify, process.clone(), output.clone()).unwrap();
    let mut commands = process.0.lock().unwrap();
    let names = commands
        .iter()
        .map(|command| command.program.to_string_lossy())
        .collect::<Vec<_>>();
    assert_eq!(names, ["git", "tar", "unzip", "mise"]);
    assert!(commands.iter().any(|cmd| cmd
        .args
        .iter()
        .any(|arg| arg.to_string_lossy().contains("candidate with spaces"))));
    commands.clear();
    drop(commands);

    std::fs::write(candidate.join("android-publication.zip"), b"corrupt").unwrap();
    let error = command(root.path(), &verify, process.clone(), output).unwrap_err();
    assert!(error.to_string().contains("Artifact digest mismatch"));
    assert!(process.0.lock().unwrap().is_empty());
}

#[test]
fn archive_checks_platform_slices_and_reproduces_the_same_bytes() {
    use std::io::Read as _;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join("release")).unwrap();
    std::fs::write(root.join("release/native-ui.toml"), "").unwrap();
    std::fs::write(root.join("Cargo.toml"), "").unwrap();
    let bundle = root.join("EweCoreFFI.xcframework");
    std::fs::create_dir(&bundle).unwrap();
    let mut slices = Vec::new();
    for (id, platform, variant, architectures) in [
        ("ios-arm64", "ios", None, vec!["arm64"]),
        (
            "ios-arm64_x86_64-simulator",
            "ios",
            Some("simulator"),
            vec!["arm64", "x86_64"],
        ),
        ("macos-arm64_x86_64", "macos", None, vec!["arm64", "x86_64"]),
    ] {
        let mut lib = serde_json::json!({"LibraryIdentifier":id,"LibraryPath":"libEweCoreFFI.a","SupportedPlatform":platform,"SupportedArchitectures":architectures});
        if let Some(variant) = variant {
            lib["SupportedPlatformVariant"] = serde_json::json!(variant);
        }
        slices.push(lib);
        std::fs::create_dir(bundle.join(id)).unwrap();
        std::fs::write(bundle.join(id).join("libEweCoreFFI.a"), id).unwrap();
    }
    let info = bundle.join("Info.plist");
    let plist = |v: &Vec<serde_json::Value>| {
        plist::to_file_xml(&info, &serde_json::json!({"AvailableLibraries":v})).unwrap();
    };
    plist(&slices);
    let invoke = || {
        command(
            root,
            &[
                "native-task",
                "task",
                "run",
                "native:release:archive-ios",
                "--stage",
                ".",
            ],
            Arc::new(FakeProcess::default()),
            Arc::new(Capture::default()),
        )
    };
    invoke().unwrap();
    let path = root.join("EweCoreFFI.xcframework.zip");
    let original = std::fs::read(&path).unwrap();
    invoke().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    for lib in &slices {
        let id = lib["LibraryIdentifier"].as_str().unwrap();
        let mut text = String::new();
        archive
            .by_name(&format!("EweCoreFFI.xcframework/{id}/libEweCoreFFI.a"))
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, id);
    }
    slices.pop();
    plist(&slices);
    assert!(invoke()
        .unwrap_err()
        .to_string()
        .contains("Unexpected XCFramework slices"));
}

#[derive(clap::Args)]
struct SwiftFixture {
    #[arg(long)]
    source: std::path::PathBuf,
    #[arg(long)]
    base: String,
}

async fn check_swift_revision(
    ctx: foundation_tasks::TaskContext,
    args: SwiftFixture,
) -> TaskResult {
    let url = "https://example.invalid/release.zip";
    let original = "1".repeat(64);
    let updated = "2".repeat(64);
    let path = args.source.join("Package.swift");
    let before = std::fs::read_to_string(&path).unwrap();
    let (same, manifest) =
        tasks::swift_release_manifest(&ctx, &args.source, &args.base, url, &original).await?;
    assert_eq!(same, args.base);
    assert_eq!(manifest, before);
    let revision =
        tasks::swift_release_manifest(&ctx, &args.source, &args.base, url, &updated).await?;
    assert_ne!(revision.0, args.base);
    assert_eq!(revision.1, before.replace(&original, &updated));
    let temp = tempfile::tempdir().unwrap();
    ctx.exec(
        ProcessCommand::new("git")
            .args(["clone", "--quiet"])
            .arg(&args.source)
            .arg(temp.path()),
    )
    .await?;
    assert_eq!(
        tasks::swift_release_manifest(&ctx, temp.path(), &args.base, url, &updated).await?,
        revision
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    Ok(())
}

#[test]
fn swift_release_reproduces_the_checksum_commit_in_an_independent_clone() {
    use foundation_tasks::TaskSet;
    let marker = "EWE_NATIVE_SWIFT_FIXTURE";
    if std::env::var_os(marker).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tasks::swift_release_reproduces_the_checksum_commit_in_an_independent_clone",
            ])
            .env(marker, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "--quiet"]);
    let checksum = "1".repeat(64);
    std::fs::write(root.join("Package.swift"),format!(".binaryTarget(name: \"Archive\", url: \"https://example.invalid/release.zip\", checksum: \"{checksum}\")\n")).unwrap();
    git(&["add", "Package.swift"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-qm",
        "fixture",
    ]);
    let base = git(&["rev-parse", "HEAD"]);
    let args = [
        "test",
        "task",
        "run",
        "swift",
        "--source",
        root.to_str().unwrap(),
        "--base",
        &base,
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let start = root.to_path_buf();
    let exit = foundation_tasks::run_entry("test", move |host| {
        Box::pin(async move {
            let ctx = host.context().clone();
            ctx.store(NativeBuildEnv::new(start));
            let mut runner = TaskRunner::new(ctx, args);
            let mut set = TaskSet::new("swift-fixture");
            set.add(
                "swift",
                "Check the Swift release revision",
                check_swift_revision,
            )?;
            runner.register(set)?;
            runner.run().await
        })
    });
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
    assert_eq!(git(&["status", "--porcelain"]), "");
}

#[test]
fn sync_checks_nested_files_and_replaces_the_owned_tree() {
    #[derive(clap::Args)]
    struct Check {
        #[arg(long)]
        check: bool,
    }
    let tmp = tempfile::tempdir().unwrap();
    let run = |check| {
        let root = tmp.path().to_owned();
        let ctx = Ctx::host();
        ctx.store(
            NativeBuildEnv::new(root.clone())
                .with_host_directories(root.as_os_str().into(), root.join("locks")),
        );
        let mut args = vec!["sync", "task", "run", "copy"];
        if check {
            args.push("--check");
        }
        let mut runner = TaskRunner::new(ctx, args.into_iter().map(Into::into).collect());
        let mut set = foundation_tasks::TaskSet::new("sync");
        set.add("copy", "Sync fixture files", move |ctx, args: Check| {
            let dir = root.clone();
            async move {
                tasks::sync::sync(
                    &ctx,
                    &dir,
                    Path::new("source"),
                    Path::new("destination"),
                    args.check,
                )
                .await
            }
        })
        .unwrap();
        runner.register(set).unwrap();
        futures_lite::future::block_on(runner.run())
    };
    let source = tmp.path().join("source");
    let destination = tmp.path().join("destination");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("nested/one"), "first").unwrap();
    run(false).unwrap();
    run(true).unwrap();
    std::fs::write(destination.join("extra"), "old").unwrap();
    std::fs::write(source.join("nested/one"), "second").unwrap();
    assert!(run(true).is_err());
    assert_eq!(
        std::fs::read_to_string(destination.join("nested/one")).unwrap(),
        "first"
    );
    run(false).unwrap();
    run(true).unwrap();
    assert!(!destination.join("extra").exists());
    assert_eq!(
        std::fs::read_to_string(destination.join("nested/one")).unwrap(),
        "second"
    );
}
