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

struct BuildTools {
    fail_native: bool,
}
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
                if self.fail_native {
                    return Err(TaskError::operation("Android fixture build failed"));
                }
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
                    let profile = if release { "release" } else { "debug" };
                    fs::write(dir.join("libcounter_app.so"), format!("{triple}/{profile}"))
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

fn staged(root: &Path, release: bool, build_type_staging: bool, fail_native: bool) -> TaskResult {
    let root = root.to_owned();
    let ctx = Ctx::host();
    let env = if build_type_staging {
        NativeBuildEnv::from_workspace()
    } else {
        NativeBuildEnv::new(root.clone())
    };
    ctx.store(env.with_host_directories(root.as_os_str().to_owned(), root.join("locks")));
    let tools = Arc::new(BuildTools { fail_native });
    ctx.store(TaskEnv {
        process: tools.clone(),
        output: tools,
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
        staged(root.path(), release, false, false).unwrap();
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
                format!("{triple}/{}", if release { "release" } else { "debug" })
            );
        }
    }
}

#[test]
fn composed_android_build_types_keep_distinct_native_outputs() {
    const FLAG: &str = "EWE_ANDROID_BUILD_TYPE_STAGING";
    // Isolate the build setting from the other test in this process.
    if std::env::var_os(FLAG).as_deref() != Some(std::ffi::OsStr::new("1")) {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .env(FLAG, "1")
            .args([
                "--exact",
                "android_library::composed_android_build_types_keep_distinct_native_outputs",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success() && String::from_utf8_lossy(&result.stdout).contains("1 passed"),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let abi = "arm64-v8a";
    for order in [[false, true], [true, false]] {
        let root = tempfile::tempdir().unwrap();
        let module = root.path().join("android/app");
        fs::create_dir_all(&module).unwrap();
        for release in order {
            staged(root.path(), release, true, false).unwrap();
        }
        assert!(staged(root.path(), order[0], true, true).is_err());
        for profile in ["debug", "release"] {
            let dir = module.join("build/generated/ewe-native").join(profile);
            assert_eq!(fs::read(dir.join("kotlin/Counter.kt")).unwrap(), b"binding");
            assert_eq!(
                fs::read_to_string(dir.join("jniLibs").join(abi).join("libcounter_app.so"))
                    .unwrap(),
                format!("aarch64-linux-android/{profile}")
            );
        }
        assert!(!module.join("generated").exists());
    }
}
