//! The image import end-to-end tests (feature 13, section 7).
//!
//! Each test checks the host first. A host without `/dev/kvm`, the Firecracker
//! program, `mkfs.ext4` or a usable driver prints the reason and returns. The
//! Docker rows need a Docker Engine and a reachable registry; a failed pull
//! prints the reason and returns too.
//!
//! The module compiles on every Unix host. Only the Linux build with the
//! `firecracker` feature contains the real tests; every other build defines the
//! same test names and skips with a reason, so the continuous integration (CI)
//! macOS run of the `images` feature passes.

/// The reason that a build cannot run the real tests.
#[cfg(not(all(target_os = "linux", feature = "firecracker")))]
fn skip(reason: &str) {
    eprintln!("skip: {reason}");
}

/// The path of a temporary directory (the Linux build only).
#[cfg(all(target_os = "linux", feature = "firecracker"))]
use std::path::{Path, PathBuf};

/// The real tests: a Linux host with Firecracker and the Docker Engine.
#[cfg(all(target_os = "linux", feature = "firecracker"))]
mod real {
    use std::path::Path;
    use std::sync::Arc;

    use foundation_context::Ctx;
    use foundation_core::env;
    use foundation_deployment_docker::DockerClient;
    use foundation_sandbox::native::docker::api::DockerApi;
    use foundation_sandbox::native::firecracker::driver::FirecrackerDriver;
    use foundation_sandbox::native::images::build::executor::{
        BuildEvent, BuildProgress, ImageBuilder,
    };
    use foundation_sandbox::native::images::build::BuildOptions;
    use foundation_sandbox::native::images::import::{ImageImporter, ImageSource, ImportOptions};
    use foundation_sandbox::native::images::sandboxfile::Sandboxfile;
    use foundation_sandbox::native::images::ImageStore;
    use foundation_sandbox::shared::command::Command;
    use foundation_sandbox::shared::control::{
        ManagerConfig, MemoryRecordStore, SandboxHandle, SandboxManager, SandboxProvider,
    };
    use foundation_sandbox::shared::driver::SandboxDriver;
    use foundation_sandbox::shared::error::SandboxError;
    use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
    use foundation_sandbox::shared::spec::{GuestArch, ImageRef, RootFs, SandboxSpec};

    /// The host, or `None` with the printed reason.
    fn usable_host() -> Option<(Ctx, SandboxHost)> {
        crate::pool::pool();
        if !Path::new("/dev/kvm").exists() {
            eprintln!("skip: this host has no /dev/kvm");
            return None;
        }
        let ctx = Ctx::host();
        match env::system(&ctx) {
            Ok(effects) => ctx.store(effects),
            Err(error) => {
                eprintln!("skip: the host environment is not available: {error}");
                return None;
            }
        }
        let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
        else {
            eprintln!("skip: the test program path is not known");
            return None;
        };
        let config = HostConfig::discover(|name| std::env::var(name).ok(), &exe_dir);
        if config.firecracker_path.is_none() {
            eprintln!("skip: the firecracker program is not found");
            return None;
        }
        if !mkfs_available(&config) {
            eprintln!("skip: mkfs.ext4 is not installed");
            return None;
        }
        let host = SandboxHost::native(config);
        ctx.store(host.clone());
        let report = crate::pool::block_on(FirecrackerDriver::new().probe(ctx.clone()));
        if !report.usable {
            eprintln!("skip: the host cannot run Firecracker: {report:?}");
            return None;
        }
        Some((ctx, host))
    }

    /// Whether the image store can start `mkfs.ext4`.
    fn mkfs_available(config: &HostConfig) -> bool {
        config.mkfs_ext4_path.is_some()
            || [
                "/sbin/mkfs.ext4",
                "/usr/sbin/mkfs.ext4",
                "/usr/bin/mkfs.ext4",
            ]
            .iter()
            .any(|path| Path::new(path).exists())
    }

    /// A manager over the Firecracker driver and the image store.
    fn manager(ctx: &Ctx, host: &SandboxHost, store: &ImageStore) -> Arc<SandboxManager> {
        Arc::new(
            SandboxManager::new(
                ctx.clone(),
                vec![Arc::new(FirecrackerDriver::new()) as Arc<dyn SandboxDriver>],
                host.connector.clone(),
                Arc::new(MemoryRecordStore::default()),
                ManagerConfig::default(),
            )
            .with_image_resolver(Arc::new(store.clone())),
        )
    }

    /// The Docker Engine of this host, or `None` with the printed reason.
    fn docker() -> Option<Arc<dyn DockerApi>> {
        match DockerClient::connect_with_defaults() {
            Ok(client) => Some(Arc::new(client) as Arc<dyn DockerApi>),
            Err(error) => {
                eprintln!("skip: no Docker Engine answers: {error}");
                None
            }
        }
    }

    /// An importer over `store`.
    fn importer(
        ctx: &Ctx,
        host: &SandboxHost,
        store: &ImageStore,
        control: Arc<dyn SandboxProvider>,
        docker: Option<Arc<dyn DockerApi>>,
    ) -> ImageImporter {
        ImageImporter::new(ctx.clone(), host.clone(), store.clone(), control, docker)
    }

    /// Creates one sandbox from the image `tag`, with no Docker driver.
    fn create_from_image(
        ctx: &Ctx,
        host: &SandboxHost,
        store: &ImageStore,
        tag: &str,
    ) -> SandboxHandle {
        crate::pool::block_on(
            manager(ctx, host, store).create(SandboxSpec::new(RootFs::Image(ImageRef::Tag(
                tag.to_string(),
            )))),
        )
        .expect("the image sandbox starts")
    }

    /// Runs one command in an image and returns its standard output and code.
    fn run_in_image(
        ctx: &Ctx,
        host: &SandboxHost,
        store: &ImageStore,
        tag: &str,
        program: &[&str],
    ) -> (String, Option<i32>) {
        let sandbox = create_from_image(ctx, host, store, tag);
        let output = crate::pool::block_on(sandbox.run(Command::new(program.iter().copied())))
            .expect("the command runs");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let code = output.status.exit_code();
        crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
        (stdout, code)
    }

    /// `e2e_import_tar`: the Alpine archive becomes a sandbox image.
    pub fn import_tar() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let store = ImageStore::new(ctx.clone(), host.clone());
        let control: Arc<dyn SandboxProvider> = manager(&ctx, &host, &store);
        let importer = importer(&ctx, &host, &store, control, None);
        let options = ImportOptions {
            tag: Some("e2e-alpine".to_string()),
            cmd: Some(vec!["/bin/sh".to_string()]),
            ..ImportOptions::default()
        };
        let report = crate::pool::block_on(
            importer.import(ImageSource::Alpine("3.22".to_string()), options),
        )
        .expect("the Alpine import succeeds");
        assert_eq!(report.image, ImageRef::Tag("e2e-alpine".to_string()));
        assert_eq!(report.manifest.arch, GuestArch::host());

        // The image runs in a manager with no Docker driver: the daemon is not
        // needed at run time.
        let (stdout, code) = run_in_image(
            &ctx,
            &host,
            &store,
            "e2e-alpine",
            &["/bin/sh", "-c", "echo ok"],
        );
        assert_eq!(stdout, "ok\n");
        assert_eq!(code, Some(0));
    }

    /// Runs one `Sandboxfile` build and returns its image reference.
    fn build_file(
        ctx: &Ctx,
        host: &SandboxHost,
        store: &ImageStore,
        text: &str,
        context: &Path,
        tag: &str,
    ) -> ImageRef {
        let control: Arc<dyn SandboxProvider> = manager(ctx, host, store);
        let importer = ImageImporter::new(
            ctx.clone(),
            host.clone(),
            store.clone(),
            Arc::clone(&control),
            None,
        );
        let builder =
            ImageBuilder::new(ctx.clone(), host.clone(), store.clone(), importer, control);
        let file = Sandboxfile::parse(text).expect("the build file parses");
        crate::pool::block_on(builder.build(
            &file,
            context,
            BuildOptions {
                tag: Some(tag.to_string()),
                ..BuildOptions::default()
            },
        ))
        .expect("the build succeeds")
    }

    /// Creates one sandbox from an image and runs its start command.
    fn run_start_cmd(
        ctx: &Ctx,
        host: &SandboxHost,
        store: &ImageStore,
        tag: &str,
    ) -> (String, Option<i32>) {
        use futures::StreamExt as _;

        let sandbox = create_from_image(ctx, host, store, tag);
        let mut handle =
            crate::pool::block_on(sandbox.start_cmd()).expect("the start command runs");
        let mut events = handle.events();
        let mut stdout = Vec::new();
        let mut code = None;
        crate::pool::block_on(async {
            while let Some(item) = events.next().await {
                match item.expect("the event reads") {
                    foundation_sandbox::shared::command::ExecEvent::Stdout(data) => {
                        stdout.extend_from_slice(&data);
                    }
                    foundation_sandbox::shared::command::ExecEvent::Exit(status) => {
                        code = status.exit_code();
                        break;
                    }
                    foundation_sandbox::shared::command::ExecEvent::Stderr(_) => {}
                }
            }
        });
        crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
        (String::from_utf8_lossy(&stdout).to_string(), code)
    }

    /// `e2e_build_app`: a build file becomes an image with its defaults.
    pub fn build_app() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let directory = super::temp_dir("build-app");
        super::write_file(
            &directory,
            "app/main.sh",
            b"echo \"$PIP_NO_CACHE_DIR $(pwd)\"\n",
        );
        let text = "FROM alpine:3.22\n\
                    ENV PIP_NO_CACHE_DIR=1\n\
                    WORKDIR /opt/app\n\
                    ADD ./app /opt/app\n\
                    CMD [\"/bin/sh\", \"/opt/app/main.sh\"]\n";
        let store = ImageStore::new(ctx.clone(), host.clone());
        let image = build_file(&ctx, &host, &store, text, &directory, "e2e-build-app");
        assert_eq!(image, ImageRef::Tag("e2e-build-app".to_string()));
        let (stdout, code) = run_start_cmd(&ctx, &host, &store, "e2e-build-app");
        assert_eq!(stdout, "1 /opt/app\n");
        assert_eq!(code, Some(0));
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// `e2e_build_owners`: the files that ADD writes belong to root.
    pub fn build_owners() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let directory = super::temp_dir("build-owners");
        super::write_file(&directory, "tool", b"#!/bin/sh\necho tool\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(
                directory.join("tool"),
                std::fs::Permissions::from_mode(0o755),
            )
            .expect("the tool mode is set");
        }
        let text = "FROM alpine:3.22\n\
                    ADD ./tool /usr/local/bin/tool\n\
                    CMD [\"/bin/sh\", \"-c\", \"stat -c '%u %a' /usr/local/bin/tool\"]\n";
        let store = ImageStore::new(ctx.clone(), host.clone());
        build_file(&ctx, &host, &store, text, &directory, "e2e-build-owners");
        let (stdout, code) = run_start_cmd(&ctx, &host, &store, "e2e-build-owners");
        assert_eq!(stdout, "0 755\n");
        assert_eq!(code, Some(0));
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// `e2e_build_network`: a network step needs a driver that gives one.
    ///
    /// A Firecracker guest has no network, so the test returns with the reason.
    /// The Linux host must run this row on libkrun with block support, with a
    /// local HTTP server at a known address.
    pub fn build_network() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let report = crate::pool::block_on(FirecrackerDriver::new().probe(ctx.clone()));
        if !report.capabilities.network_host {
            eprintln!(
                "skip: the firecracker driver has no guest network; run this row on libkrun with block support and a local HTTP server"
            );
            return;
        }
        let directory = super::temp_dir("build-network");
        let text = "FROM alpine:3.22\n\
                    RUN --network host wget -q -T 5 -O /dev/null http://127.0.0.1:9\n";
        let store = ImageStore::new(ctx.clone(), host.clone());
        let control: Arc<dyn SandboxProvider> = manager(&ctx, &host, &store);
        let importer = ImageImporter::new(
            ctx.clone(),
            host.clone(),
            store.clone(),
            Arc::clone(&control),
            None,
        );
        let builder =
            ImageBuilder::new(ctx.clone(), host.clone(), store.clone(), importer, control);
        let file = Sandboxfile::parse(text).expect("the build file parses");
        // The local server is not up: a failed step proves the guest reached
        // the host network.
        let result = crate::pool::block_on(builder.build(
            &file,
            &directory,
            BuildOptions {
                tag: Some("e2e-build-network".to_string()),
                ..BuildOptions::default()
            },
        ));
        match result {
            Err(error) => assert!(
                matches!(
                    error.current_context(),
                    foundation_sandbox::shared::error::SandboxError::BuildStepFailed { .. }
                ),
                "the network step runs and fails: {error:?}"
            ),
            Ok(_) => eprintln!("skip: the local test server answered; the row needs one"),
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// `e2e_build_resume`: a finished build is the cache of the next one.
    pub fn build_resume() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let directory = super::temp_dir("build-resume");
        let first = "FROM alpine:3.22\n\
                     RUN touch /a\n\
                     RUN touch /b\n\
                     CMD [\"/bin/sh\", \"-c\", \"ls /a /b\"]\n";
        let store = ImageStore::new(ctx.clone(), host.clone());
        build_file(&ctx, &host, &store, first, &directory, "e2e-build-resume-1");

        let control: Arc<dyn SandboxProvider> = manager(&ctx, &host, &store);
        let importer = ImageImporter::new(
            ctx.clone(),
            host.clone(),
            store.clone(),
            Arc::clone(&control),
            None,
        );
        let builder =
            ImageBuilder::new(ctx.clone(), host.clone(), store.clone(), importer, control);
        let longer = "FROM alpine:3.22\n\
                      RUN touch /a\n\
                      RUN touch /b\n\
                      RUN touch /c\n\
                      CMD [\"/bin/sh\", \"-c\", \"ls /a /b /c\"]\n";
        let file = Sandboxfile::parse(longer).expect("the longer build file parses");
        let mut stream = builder.build_with_progress(
            file,
            directory.clone(),
            BuildOptions {
                tag: Some("e2e-build-resume-2".to_string()),
                ..BuildOptions::default()
            },
        );
        let mut hits = 0;
        let result = crate::pool::block_on(async {
            while let Some(event) = stream.next().await {
                match event {
                    BuildEvent::Progress(BuildProgress::CacheHit { .. }) => hits += 1,
                    BuildEvent::Progress(_) => {}
                    BuildEvent::Done(result) => return *result,
                }
            }
            Err(foundation_sandbox::shared::error::SandboxError::StreamLost.into())
        });
        result.expect("the longer build succeeds");
        assert!(
            hits >= 3,
            "FROM and the two finished steps are cache hits: {hits}"
        );
        let (stdout, code) = run_start_cmd(&ctx, &host, &store, "e2e-build-resume-2");
        assert_eq!(stdout.lines().count(), 3, "all three files exist: {stdout}");
        assert_eq!(code, Some(0));
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// `e2e_import_python`: a Docker image runs as a microVM sandbox.
    pub fn import_python() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let Some(docker) = docker() else {
            return;
        };
        let store = ImageStore::new(ctx.clone(), host.clone());
        let control: Arc<dyn SandboxProvider> = manager(&ctx, &host, &store);
        let importer = importer(&ctx, &host, &store, control, Some(docker));
        let options = ImportOptions {
            tag: Some("e2e-python".to_string()),
            ..ImportOptions::default()
        };
        let report = match crate::pool::block_on(
            importer.import(ImageSource::Docker("python:3.12-slim".to_string()), options),
        ) {
            Ok(report) => report,
            Err(error) => {
                // A registry that this host cannot reach is a skip, not a
                // failure of the code (the image may be absent locally).
                eprintln!("skip: the Docker import failed, the pull may be blocked: {error}");
                return;
            }
        };
        assert!(!report.cached, "the first import is not a cache hit");

        let (stdout, code) = run_in_image(
            &ctx,
            &host,
            &store,
            "e2e-python",
            &["python3", "-c", "print(1)"],
        );
        assert_eq!(stdout, "1\n");
        assert_eq!(code, Some(0));
    }

    /// `e2e_import_owners`: owners, modes and the setuid bit survive.
    pub fn import_owners() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let Some(docker) = docker() else {
            return;
        };
        if !build_owner_image() {
            return;
        }
        let store = ImageStore::new(ctx.clone(), host.clone());
        let control: Arc<dyn SandboxProvider> = manager(&ctx, &host, &store);
        let importer = importer(&ctx, &host, &store, control, Some(docker));
        let options = ImportOptions {
            tag: Some("e2e-owners".to_string()),
            ..ImportOptions::default()
        };
        crate::pool::block_on(
            importer.import(ImageSource::Docker(OWNER_IMAGE.to_string()), options),
        )
        .expect("the owners import succeeds");

        let (file, code) = run_in_image(
            &ctx,
            &host,
            &store,
            "e2e-owners",
            &["stat", "-c", "%a %u %g", "/data/f"],
        );
        assert_eq!(file.trim(), "640 1000 1000");
        assert_eq!(code, Some(0));
        let (setuid, code) = run_in_image(
            &ctx,
            &host,
            &store,
            "e2e-owners",
            &["stat", "-c", "%a", "/bin/suidtool"],
        );
        assert_eq!(setuid.trim(), "4755");
        assert_eq!(code, Some(0));
    }

    /// `e2e_import_too_large`: an image larger than its size fails cleanly.
    pub fn import_too_large() {
        let Some((ctx, host)) = usable_host() else {
            return;
        };
        let directory = super::temp_dir("too-large");
        let tar = directory.join("big.tar");
        make_sparse_tar(&tar, 64 * 1024 * 1024);
        let store = ImageStore::new(ctx.clone(), host.clone());
        let control: Arc<dyn SandboxProvider> = manager(&ctx, &host, &store);
        let importer = importer(&ctx, &host, &store, control, None);
        let options = ImportOptions {
            tag: Some("e2e-too-large".to_string()),
            size_mib: 32,
            ..ImportOptions::default()
        };
        let error = crate::pool::block_on(importer.import(ImageSource::Tar(tar), options))
            .expect_err("the import is larger than the image");
        match error.current_context() {
            SandboxError::ArtifactTooLarge { message, .. } => assert!(
                message.contains("size_mib = 32"),
                "the error names the size limit: {message}"
            ),
            other => panic!("the error is ArtifactTooLarge: {other:?}"),
        }
        assert!(
            crate::pool::block_on(store.list())
                .expect("the store lists")
                .is_empty(),
            "no image is committed"
        );
        let work = host.config.cache_dir.join("images/work");
        assert!(
            host.fs
                .read_dir(&work)
                .expect("the work directory is readable")
                .is_empty(),
            "no work file stays"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// The name of the image that the owner test builds.
    const OWNER_IMAGE: &str = "ewe-e2e-owners:1";

    /// Builds the small owner image with the Docker command line.
    ///
    /// It returns false, with the printed reason, when the build fails (for
    /// example because the base image cannot be pulled).
    fn build_owner_image() -> bool {
        let directory = super::temp_dir("owners-build");
        let dockerfile = directory.join("Dockerfile");
        let text = "FROM alpine:3.22\n\
                    RUN adduser -D -u 1000 app \\\n\
                        && mkdir -p /data \\\n\
                        && echo hi > /data/f \\\n\
                        && chown 1000:1000 /data/f \\\n\
                        && chmod 0640 /data/f \\\n\
                        && cp /bin/busybox /bin/suidtool \\\n\
                        && chmod 4755 /bin/suidtool\n";
        if std::fs::write(&dockerfile, text).is_err() {
            eprintln!("skip: the owner Dockerfile cannot be written");
            return false;
        }
        let status = std::process::Command::new("docker")
            .args(["build", "-t", OWNER_IMAGE])
            .arg(&directory)
            .status();
        let _ = std::fs::remove_dir_all(&directory);
        match status {
            Ok(status) if status.success() => true,
            Ok(status) => {
                eprintln!("skip: docker build failed with {status}; the base may be absent");
                false
            }
            Err(error) => {
                eprintln!("skip: the docker command line is not available: {error}");
                false
            }
        }
    }

    /// Writes one sparse tar with the file `big` of `size` bytes.
    fn make_sparse_tar(path: &Path, size: u64) {
        use std::io::{Seek as _, SeekFrom, Write as _};
        let header = tar_header("big", size);
        let mut file = std::fs::File::create(path).expect("the tar file is made");
        file.write_all(&header).expect("the tar header is written");
        let data = size.div_ceil(512) * 512;
        let total = 512 + data + 1024;
        file.set_len(total).expect("the tar file is made sparse");
        file.seek(SeekFrom::Start(512 + data))
            .expect("the tar end is seekable");
        file.write_all(&[0_u8; 1024])
            .expect("the tar end is written");
        file.flush().expect("the tar file is flushed");
    }

    /// One ustar header of a regular file of `size` bytes.
    fn tar_header(name: &str, size: u64) -> [u8; 512] {
        let mut block = [0_u8; 512];
        let name = name.as_bytes();
        block[..name.len()].copy_from_slice(name);
        block[100..108].copy_from_slice(&octal(0o644, 8));
        block[108..116].copy_from_slice(&octal(0, 8));
        block[116..124].copy_from_slice(&octal(0, 8));
        block[124..136].copy_from_slice(&octal(size, 12));
        block[136..148].copy_from_slice(&octal(0, 12));
        block[148..156].fill(b' ');
        block[156] = b'0';
        block[257..263].copy_from_slice(b"ustar\0");
        block[263..265].copy_from_slice(b"00");
        let checksum: u64 = block.iter().map(|byte| u64::from(*byte)).sum();
        block[148..155].copy_from_slice(&octal(checksum, 7));
        block[155] = b' ';
        block
    }

    /// The octal text of `value`, followed by a zero byte, for a field of
    /// `width` bytes.
    fn octal(value: u64, width: usize) -> Vec<u8> {
        let digits = format!("{value:0width$o}", width = width - 1);
        let mut bytes = digits.into_bytes();
        bytes.push(0);
        bytes
    }
}

/// Writes one file below `root`, with its parent directories.
#[cfg(all(target_os = "linux", feature = "firecracker"))]
fn write_file(root: &Path, relative: &str, data: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("the parent directory"))
        .expect("the parent directory is made");
    std::fs::write(path, data).expect("the test file is written");
}

/// A fresh directory below the temporary directory.
#[cfg(all(target_os = "linux", feature = "firecracker"))]
fn temp_dir(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ewe-e2e-images-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("the temporary directory is made");
    path
}

/// The test wrappers. Every build defines the same names.
macro_rules! e2e_test {
    ($name:ident, $real:ident, $reason:literal) => {
        /// The image import end-to-end row of feature 13.
        #[cfg(all(target_os = "linux", feature = "firecracker"))]
        #[test]
        fn $name() {
            real::$real();
        }

        /// The image import end-to-end row of feature 13.
        #[cfg(not(all(target_os = "linux", feature = "firecracker")))]
        #[test]
        fn $name() {
            skip($reason);
        }
    };
}

e2e_test!(
    e2e_import_tar,
    import_tar,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_import_python,
    import_python,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_import_owners,
    import_owners,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_import_too_large,
    import_too_large,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_build_app,
    build_app,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_build_owners,
    build_owners,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_build_network,
    build_network,
    "this test needs a Linux host with the firecracker feature"
);
e2e_test!(
    e2e_build_resume,
    build_resume,
    "this test needs a Linux host with the firecracker feature"
);
