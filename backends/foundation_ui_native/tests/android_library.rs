//! Stage only the Android library produced for the application.
use foundation_context::Ctx;
use foundation_tasks::{
    Cancellation, OutputStream, ProcessCommand, TaskEnv, TaskError, TaskOutput, TaskProcess,
    TaskResult, TaskRunner, TaskSet,
};
use foundation_ui_native::tasks::{project, NativeBuildEnv};
use foundation_ui_project::application::{Application, Host, Target};
use futures_util::future::BoxFuture;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

struct BuildTools;
impl TaskOutput for BuildTools {
    fn write<'a>(&'a self, _: OutputStream, _: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}
impl TaskProcess for BuildTools {
    fn run(
        &self,
        cmd: ProcessCommand,
        _: Arc<dyn TaskOutput>,
        _: Cancellation,
    ) -> BoxFuture<'_, TaskResult> {
        Box::pin(async move {
            let args = cmd
                .args
                .iter()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>();
            let after = |key: &str| {
                args.windows(2)
                    .find(|args| args[0] == key)
                    .map(|args| PathBuf::from(args[1]))
            };
            let release = args.contains(&"--release");
            if args.first() == Some(&"ndk") {
                assert!(!args.contains(&"-o"));
                assert!(cmd
                    .env
                    .iter()
                    .any(|(key, value)| key == "CARGO_NDK_OUTPUT_PATH" && value.is_none()));
                let root = after("--target-dir").unwrap();
                for triple in [
                    "armv7-linux-androideabi",
                    "aarch64-linux-android",
                    "i686-linux-android",
                    "x86_64-linux-android",
                ] {
                    let dir = root
                        .join(triple)
                        .join(if release { "release" } else { "debug" });
                    fs::create_dir_all(&dir).map_err(TaskError::operation)?;
                    fs::write(dir.join("libcounter_app.so"), triple)
                        .map_err(TaskError::operation)?;
                    fs::write(dir.join("libdependency.rlib"), b"dependency")
                        .map_err(TaskError::operation)?;
                }
            } else if let Some(generated) = after("--out-dir") {
                fs::create_dir_all(&generated).map_err(TaskError::operation)?;
                fs::write(generated.join("Counter.kt"), b"binding")
                    .map_err(TaskError::operation)?;
            } else {
                assert!(args.contains(&"bindgen"));
            }
            Ok(())
        })
    }
}

fn staged(root: &Path, release: bool) -> TaskResult {
    let root = root.to_owned();
    let ctx = Ctx::host();
    ctx.store(
        NativeBuildEnv::new(root.clone())
            .with_host_directories(root.as_os_str().to_owned(), root.join("locks")),
    );
    ctx.store(TaskEnv {
        process: Arc::new(BuildTools),
        output: Arc::new(BuildTools),
        cancellation: Cancellation::new(),
    });
    let mut tasks = TaskSet::new("test");
    tasks.add_command(
        "prepare",
        "Prepare the application",
        clap::Command::new("prepare"),
        move |ctx, _| {
            let root = root.clone();
            async move {
                let mut app = Application::new(
                    "Grove".into(),
                    [Target::Android].into(),
                    Some("example.grove".into()),
                    Host::OnDevice,
                )
                .unwrap();
                app.android.as_mut().unwrap().architectures =
                    ["armeabi-v7a", "arm64-v8a", "x86", "x86_64"]
                        .map(String::from)
                        .into();
                project::prepare(&ctx, &root, &app, Target::Android, release).await
            }
        },
    )?;
    let mut runner = TaskRunner::new(
        ctx,
        ["test", "task", "run", "prepare"].map(Into::into).into(),
    );
    runner.register(tasks)?;
    futures_lite::future::block_on(runner.run())
}

#[test]
fn application_stages_its_own_shared_library_for_each_android_architecture() {
    let root = tempfile::tempdir().unwrap();
    let module = root.path().join("android/app");
    fs::create_dir_all(&module).unwrap();
    for release in [false, true] {
        staged(root.path(), release).unwrap();
        assert_eq!(
            fs::read(module.join("generated/kotlin/Counter.kt")).unwrap(),
            b"binding"
        );
        for (abi, triple) in [
            ("armeabi-v7a", "armv7-linux-androideabi"),
            ("arm64-v8a", "aarch64-linux-android"),
            ("x86", "i686-linux-android"),
            ("x86_64", "x86_64-linux-android"),
        ] {
            let dir = module.join("generated/jniLibs").join(abi);
            assert_eq!(dir.read_dir().unwrap().count(), 1);
            assert_eq!(
                fs::read_to_string(dir.join("libcounter_app.so")).unwrap(),
                triple
            );
        }
    }
}
