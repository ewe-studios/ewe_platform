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
    std::fs::create_dir_all(root.path().join("scripts/ui_native")).unwrap();
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
fn android_build_installs_two_abis_for_the_binding_and_todo() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("workspace with spaces");
    std::fs::create_dir_all(root.join("scripts/ui_native")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "").unwrap();
    let process = Arc::new(FakeProcess(Mutex::default(), true));
    command(
        &root,
        &[
            "native-task",
            "task",
            "run",
            "native:build-android",
            "--with-todo",
        ],
        process.clone(),
        Arc::new(Capture::default()),
    )
    .unwrap();
    let calls = process.0.lock().unwrap();
    let builds = calls
        .iter()
        .filter(|call| call.program == "cargo")
        .collect::<Vec<_>>();
    assert_eq!(builds.len(), 2);
    for call in &builds {
        assert!(call.env.iter().any(|(key, value)| key == "ANDROID_NDK_HOME"
            && value.as_ref().is_some_and(
                |value| value == Path::new("/android home").join("ndk/123").as_os_str()
            )));
    }
    assert!(builds[1]
        .args
        .iter()
        .any(|arg| arg == "--no-default-features"));
    for abi in ["arm64-v8a", "x86_64"] {
        assert_eq!(
            std::fs::read(
                root.join("mobile/android/binding/src/main/jniLibs")
                    .join(abi)
                    .join("libfoundation_ui_native.so")
            )
            .unwrap(),
            b"foundation_ui_native"
        );
        for app in ["todo_server", "todo_on_device"] {
            assert_eq!(
                std::fs::read(
                    root.join("examples/ui_todo/android")
                        .join(app)
                        .join("app/src/main/jniLibs")
                        .join(abi)
                        .join("libui_todo.so")
                )
                .unwrap(),
                b"ui_todo"
            );
        }
    }
}
