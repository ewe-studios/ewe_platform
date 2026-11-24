//! Run the independent starter or the two Todo apps on Android.
use super::{
    device::{self, cleanup, cleanup_command, text},
    stage, NativeBuildEnv,
};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Args)]
pub(super) struct Options {
    #[arg(long)]
    maven: Option<PathBuf>,
}
pub(super) async fn starter(ctx: TaskContext, args: Options) -> TaskResult {
    run(&ctx, args, false).await
}
pub(super) async fn todo(ctx: TaskContext, args: Options) -> TaskResult {
    run(&ctx, args, true).await
}

struct Device<'a> {
    ctx: &'a TaskContext,
    root: &'a Path,
    adb: PathBuf,
    serial: String,
    options: &'a device::Options,
}
impl Device<'_> {
    fn adb(&self) -> ProcessCommand {
        ProcessCommand::new(&self.adb)
            .current_dir(self.root)
            .args(["-s", &self.serial])
    }
    fn screen<'a>(
        &'a self,
        package: &'a str,
    ) -> Result<super::android_screen::Screen<'a>, TaskError> {
        let setting = |key, default| {
            self.options
                .get(key)
                .map(|v| {
                    v.to_str()
                        .ok_or_else(|| TaskError::operation("Invalid device setting"))?
                        .parse::<u64>()
                        .map_err(TaskError::operation)
                })
                .transpose()
                .map(|v| v.unwrap_or(default))
        };
        Ok(super::android_screen::Screen {
            ctx: self.ctx,
            adb: self.adb(),
            package,
            timeout: Duration::from_secs(setting("EWE_ANDROID_VIEW_TIMEOUT", 60)?),
            interval: Duration::from_millis(setting("EWE_ANDROID_VIEW_POLL_MS", 500)?),
        })
    }
    async fn ui(&self, mode: &str) -> TaskResult {
        let package = if mode.starts_with("server") {
            "ewe.todo"
        } else {
            "ewe.todo.local"
        };
        self.screen(package)?
            .todo(mode, self.options.get("EWE_THEME_CAPTURES").map(Path::new))
            .await
    }
    async fn install(&self, apk: PathBuf) -> TaskResult {
        self.ctx
            .exec(self.adb().args(["install", "-r"]).arg(apk))
            .await
    }
    async fn start(&self, url: &str) -> TaskResult {
        let flag = |k| {
            self.options
                .get(k)
                .and_then(|s| s.to_str())
                .unwrap_or("false")
        };
        self.ctx
            .exec(self.adb().args([
                "shell",
                "am",
                "start",
                "-n",
                "ewe.todo/.MainActivity",
                "--es",
                "server",
                url,
                "--ez",
                "rpc",
                flag("EWE_TODO_RPC"),
                "--ez",
                "socket",
                flag("EWE_TODO_SOCKET"),
            ]))
            .await
    }
    async fn instrument(&self, args: &[&str], expected: &str) -> TaskResult {
        let report = text(
            self.ctx,
            self.adb()
                .args(["shell", "am", "instrument", "-w"])
                .args(args.iter().copied())
                .arg("ewe.todo.test/ewe.todo.ReplayCheck"),
        )
        .await?;
        if !report.contains(expected) {
            return Err(TaskError::operation(format!(
                "Missing instrumentation result: {expected}"
            )));
        }
        Ok(())
    }
    async fn connected<F, Fut>(&self, url: String, work: F) -> TaskResult
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = TaskResult>,
    {
        let port = url
            .strip_prefix("http://127.0.0.1:")
            .and_then(|v| v.parse::<u16>().ok())
            .ok_or_else(|| TaskError::operation("Missing todo server port"))?;
        let endpoint = format!("tcp:{port}");
        self.ctx
            .exec(self.adb().args(["reverse", &endpoint, &endpoint]))
            .await?;
        let result = work(url).await;
        cleanup(
            result,
            cleanup_command(
                self.ctx,
                self.adb().args(["reverse", "--remove", &endpoint]),
            )
            .await,
        )
    }
    async fn apps(&self, starter: &Path, todo: bool, exe: &Path) -> TaskResult {
        let projects = self.root.join("examples/ui_todo/android");
        if todo {
            self.install(projects.join("todo_on_device/app/build/outputs/apk/debug/app-debug.apk"))
                .await?;
            self.install(projects.join("todo_server/app/build/outputs/apk/debug/app-debug.apk"))
                .await?;
            self.install(projects.join(
                "todo_server/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk",
            ))
            .await?;
            self.ctx
                .exec(
                    self.adb()
                        .args(["shell", "am", "force-stop", "ewe.todo.local"]),
                )
                .await?;
            self.ctx
                .exec(self.adb().args([
                    "shell",
                    "am",
                    "start",
                    "-n",
                    "ewe.todo.local/.MainActivity",
                ]))
                .await?;
            self.ui("offline").await?;
        }
        device::server(self.ctx, self.root, exe, false, |url| {
            self.connected(url, |url| async move {
                if !todo {
                    self.install(starter.join("app/build/outputs/apk/debug/app-debug.apk"))
                        .await?;
                    self.ctx
                        .exec(self.adb().args([
                            "shell",
                            "am",
                            "start",
                            "-n",
                            "ewe.starter/.MainActivity",
                            "--es",
                            "server",
                            &url,
                        ]))
                        .await?;
                    return self.screen("ewe.starter")?.starter(&url).await;
                }
                self.ctx
                    .exec(self.adb().args(["shell", "am", "force-stop", "ewe.todo"]))
                    .await?;
                self.start(&url).await?;
                self.ui("server").await?;
                device::server(self.ctx, self.root, exe, true, |data| {
                    self.connected(data, |data| async move {
                        self.instrument(
                            &["-e", "data", "true", "-e", "server", &data],
                            "GetTodo returned the requested UUID, Buy milk, and NotFound.",
                        )
                        .await
                    })
                })
                .await?;
                self.instrument(
                    &["-e", "subscriptions", "true", "-e", "server", &url],
                    "delivered targeted expiry.",
                )
                .await?;
                self.instrument(&["-e", "server", &url], "Second delete refused:")
                    .await?;
                self.start(&url).await?;
                self.ui("server-delete").await?;
                self.ctx
                    .exec(self.adb().args([
                        "shell",
                        "am",
                        "start",
                        "-n",
                        "ewe.todo.local/.MainActivity",
                    ]))
                    .await?;
                self.ui("isolation").await?;
                if self.options.get("EWE_THEME_CAPTURES").is_some() {
                    self.ui("theme-review").await?;
                }
                Ok(())
            })
        })
        .await
    }
}

async fn run(ctx: &TaskContext, args: Options, todo: bool) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let root = env.root().await?;
    let sdk = PathBuf::from(
        env.devices
            .get("ANDROID_HOME")
            .ok_or_else(|| TaskError::operation("Set ANDROID_HOME to the Android SDK"))?,
    );
    let abi = match std::env::consts::ARCH {
        "aarch64" => "arm64-v8a",
        "x86_64" => "x86_64",
        arch => {
            return Err(TaskError::operation(format!(
                "Unsupported emulator architecture: {arch}"
            )))
        }
    };
    if cfg!(target_os = "linux") {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .map_err(TaskError::operation)?;
    }
    let kind = if env.devices.get("EWE_THEME_CAPTURES").is_some() {
        "google_apis"
    } else {
        "aosp_atd"
    };
    let image = format!("system-images;android-36;{kind};{abi}");
    let image_dir = format!("system-images/android-36/{kind}/{abi}/");
    let emulator = sdk.join("emulator/emulator");
    if !emulator.is_file() || !sdk.join(&image_dir).join("source.properties").is_file() {
        let mut setting = OsString::from("--sdk_root=");
        setting.push(&sdk);
        ctx.exec(
            ProcessCommand::new("sdkmanager")
                .current_dir(&root)
                .arg(setting)
                .args(["--install", "emulator", &image]),
        )
        .await?;
    }
    let temp = tempfile::tempdir().map_err(TaskError::operation)?;
    let starter = temp.path().join("starter");
    let config = stage::config(&root)?;
    let mut gradle = ProcessCommand::new(root.join("mobile/android/gradlew"))
        .current_dir(&root)
        .env("ANDROID_SDK_ROOT", &sdk)
        .arg(format!("-PeweVersion={}", config.version));
    if let Some(repo) = args.maven {
        let mut arg = OsString::from("-PeweRepository=");
        arg.push(
            root.join(repo)
                .canonicalize()
                .map_err(TaskError::operation)?,
        );
        gradle = gradle.arg(arg);
    } else {
        gradle = gradle.env("GRADLE_USER_HOME", temp.path().join("gradle"));
    }
    if todo {
        let projects = root.join("examples/ui_todo/android");
        for app in ["todo_on_device", "todo_server"] {
            ctx.exec(
                gradle
                    .clone()
                    .arg("-p")
                    .arg(projects.join(app))
                    .arg(":app:assembleDebug"),
            )
            .await?;
        }
        ctx.exec(
            gradle
                .arg("-p")
                .arg(projects.join("todo_server"))
                .arg(":app:assembleDebugAndroidTest"),
        )
        .await?;
    } else {
        fs::create_dir(&starter).map_err(TaskError::operation)?;
        ctx.exec(
            ProcessCommand::new("rsync")
                .current_dir(&root)
                .args([
                    "-a",
                    "--exclude",
                    "build",
                    "--exclude",
                    ".gradle",
                    "--exclude",
                    ".kotlin",
                    "--exclude",
                    "local.properties",
                    "examples/mobile_starter/android/",
                ])
                .arg(&starter),
        )
        .await?;
        ctx.exec(gradle.arg("-p").arg(&starter).arg(":app:assembleDebug"))
            .await?;
    }
    let exe = device::host(ctx, &root).await?;
    let adb = sdk.join("platform-tools/adb");
    if let Some(serial) = env.devices.get("EWE_ANDROID_SERIAL") {
        return Device {
            ctx,
            root: &root,
            adb,
            serial: serial.to_string_lossy().into_owned(),
            options: &env.devices,
        }
        .apps(&starter, todo, &exe)
        .await;
    }
    let avd = temp.path().join("avd");
    fs::create_dir(&avd).map_err(TaskError::operation)?;
    let command = |exe| {
        ProcessCommand::new(exe)
            .current_dir(&root)
            .env("ANDROID_AVD_HOME", &avd)
            .env("ANDROID_SDK_ROOT", &sdk)
    };
    ctx.exec(
        command("avdmanager")
            .args([
                "create",
                "avd",
                "--name",
                "ewe-release",
                "--package",
                &image,
                "--device",
                "pixel",
            ])
            .private_input(b"no\n".to_vec()),
    )
    .await?;
    let ini = avd.join("ewe-release.avd/config.ini");
    let contents = fs::read_to_string(&ini).map_err(TaskError::operation)?;
    let mut settings: BTreeMap<_, _> = contents
        .lines()
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    settings.insert("image.sysdir.1", &image_dir);
    settings.insert("disk.dataPartition.size", "2G");
    fs::write(
        ini,
        settings
            .into_iter()
            .map(|(k, v)| format!("{k}={v}\n"))
            .collect::<String>(),
    )
    .map_err(TaskError::operation)?;
    let port = (5554..5682)
        .step_by(2)
        .find(|p| {
            TcpListener::bind(("127.0.0.1", *p))
                .is_ok_and(|_console| TcpListener::bind(("127.0.0.1", *p + 1)).is_ok())
        })
        .ok_or_else(|| TaskError::operation("No available emulator port pair"))?;
    let device = Device {
        ctx,
        root: &root,
        adb,
        serial: format!("emulator-{port}"),
        options: &env.devices,
    };
    ctx.exec(ProcessCommand::new(&device.adb).arg("start-server"))
        .await?;
    let start = ProcessCommand::new(emulator)
        .current_dir(&root)
        .env("ANDROID_AVD_HOME", avd)
        .env("ANDROID_SDK_ROOT", sdk)
        .args([
            "-avd",
            "ewe-release",
            "-port",
            &port.to_string(),
            "-no-window",
            "-no-audio",
            "-no-snapshot",
            "-no-boot-anim",
            "-partition-size",
            "2048",
            "-gpu",
            "swiftshader_indirect",
        ]);
    let timeout = env
        .devices
        .get("EWE_ANDROID_BOOT_TIMEOUT")
        .map(|v| v.to_string_lossy().parse::<u64>())
        .transpose()
        .map_err(TaskError::operation)?
        .unwrap_or(240);
    device::background(
        ctx,
        start,
        ctx.require::<TaskEnv>()?.output.clone(),
        async {
            device::wait_for(
                ctx,
                Duration::from_secs(timeout),
                Duration::from_secs(2),
                "the Android emulator",
                || async {
                    Ok(text(
                        ctx,
                        device
                            .adb()
                            .args(["shell", "getprop", "sys.boot_completed"]),
                    )
                    .await?
                        == "1")
                },
            )
            .await?;
            device.apps(&starter, todo, &exe).await
        },
    )
    .await
}
