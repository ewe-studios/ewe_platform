//! End-to-end tests for F04 — image building (Decision 05).
//!
//! WHY: `DockerFileConfig` had no consumer and no test anywhere in the
//! workspace, and `build_once` shelled out to the `docker` **CLI** — which needs
//! the CLI installed, cannot report structured errors, and bypasses the
//! bollard-free API client that decision 01 exists for. Nothing exercised it, so
//! none of that showed.
//!
//! WHAT is proven, against a real daemon:
//! - an inline Dockerfile builds through `POST /build` and the image is usable
//!   afterwards (a container is run from it and its output read back);
//! - a context directory's files are uploaded and `COPY`-able;
//! - `build_once` is once: a second call reports `was_cached`;
//! - build args reach the build;
//! - a failing Dockerfile is an error, not a silent success;
//! - a missing tag is rejected up front.
//!
//! The BuildKit backend lives in `buildkit_backend` below and needs both the
//! `buildkit` feature and a reachable `buildkitd` (it skips otherwise).
//!
//! Run: `cargo test -p foundation_deployment_platform --test
//! image_build_integration -- --ignored` (needs a Docker daemon).

use foundation_core::valtron::initialize_pool;
use foundation_deployment_platform::docker::{
    ContainerConfig, ContainerHandle, DockerClient, DockerError, DockerFileConfig,
};

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

/// Unique-ish tag so parallel runs never collide on a daemon-global image name.
fn tag(name: &str) -> String {
    format!("ewe-f04-{name}:{}", std::process::id())
}

async fn remove_image(t: &str) {
    if let Ok(c) = DockerClient::connect_with_defaults() {
        let _ = c.image_delete(t, Some(true), Some(false)).await;
    }
}

#[test]
#[ignore = "requires Docker daemon running"]
fn builds_an_inline_dockerfile_into_a_runnable_image() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    foundation_core::valtron::block_on_future(async {
        let t = tag("inline");
        remove_image(&t).await;

        let built = DockerFileConfig::new(
            "FROM alpine:3\nRUN echo built-natively > /marker\nCMD [\"cat\", \"/marker\"]\n",
        )
        .tag(&t)
        .build_once()
        .await
        .expect("inline build should succeed");

        assert_eq!(built.image_tag, t);
        assert!(!built.sha256.is_empty(), "a built image should report a digest");
        assert!(!built.was_cached, "the first build is not a cache hit");

        // The real proof: dockerd can run what we built, and the file we RUN-created
        // is in the image.
        let handle = ContainerHandle::start_async(
            ContainerConfig::new(&t).stop_timeout_secs(2),
        )
        .await
        .expect("the built image should run");
        let logs = handle.wait_for_exit_async().await.expect("logs");
        assert!(
            logs.contains("built-natively"),
            "the built image should carry what the Dockerfile made, got: {logs:?}"
        );
        drop(handle);

        // build_once is once: the second call finds the tag and skips the build.
        let again = DockerFileConfig::new("FROM alpine:3\nRUN echo ignored > /marker\n")
            .tag(&t)
            .build_once()
            .await
            .expect("second build_once should succeed");
        assert!(again.was_cached, "an existing tag must not be rebuilt");
        assert_eq!(again.sha256, built.sha256, "the cached digest is the built one");

        remove_image(&t).await;
    });
}

#[test]
#[ignore = "requires Docker daemon running"]
fn build_context_files_are_uploaded_and_build_args_apply() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    // A context on disk, with a file the Dockerfile COPYs.
    let dir = std::env::temp_dir().join(format!("ewe-f04-ctx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create context dir");
    std::fs::write(dir.join("payload.txt"), b"file-from-the-context").expect("write context file");

    let ctx = dir.clone();
    foundation_core::valtron::block_on_future(async move {
        let t = tag("context");
        remove_image(&t).await;

        DockerFileConfig::new(
            "FROM alpine:3\n\
             ARG GREETING=unset\n\
             COPY payload.txt /payload.txt\n\
             RUN echo arg-was-$GREETING > /arg.txt\n\
             CMD [\"sh\", \"-c\", \"cat /payload.txt; echo; cat /arg.txt\"]\n",
        )
        .context(&ctx)
        .arg("GREETING", "delivered")
        .tag(&t)
        .build_once()
        .await
        .expect("context build should succeed");

        let handle = ContainerHandle::start_async(
            ContainerConfig::new(&t).stop_timeout_secs(2),
        )
        .await
        .expect("the built image should run");
        let logs = handle.wait_for_exit_async().await.expect("logs");

        assert!(
            logs.contains("file-from-the-context"),
            "the context file must have been uploaded and COPYed, got: {logs:?}"
        );
        assert!(
            logs.contains("arg-was-delivered"),
            "the build arg must have reached the build, got: {logs:?}"
        );
        drop(handle);

        remove_image(&t).await;
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "requires Docker daemon running"]
fn a_failing_dockerfile_is_reported_as_an_error() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }
    let _pool = initialize_pool(54, Some(4));

    foundation_core::valtron::block_on_future(async {
        let t = tag("failing");

        // dockerd streams this failure back inside a 200 response, so a client
        // that ignores the body reports success for a build that made nothing.
        let result = DockerFileConfig::new("FROM alpine:3\nRUN exit 7\n")
            .tag(&t)
            .build_once()
            .await;

        match result {
            Err(trace) => {
                let msg = format!("{trace}");
                assert!(
                    msg.contains("Image build failed"),
                    "a failing build should surface as ImageBuild, got: {msg}"
                );
            }
            Ok(built) => panic!("a failing Dockerfile must not report success: {built:?}"),
        }
    });
}

#[test]
fn an_untagged_build_is_rejected_up_front() {
    let _pool = initialize_pool(54, Some(4));

    foundation_core::valtron::block_on_future(async {
        // No daemon needed: the config is invalid before anything is sent.
        let result = DockerFileConfig::new("FROM alpine:3\n").build_once().await;

        match result {
            Err(trace) => {
                let msg = format!("{trace}");
                assert!(
                    msg.contains("tag is required"),
                    "the error should say what is missing, got: {msg}"
                );
            }
            Ok(built) => panic!("an untagged build must not succeed: {built:?}"),
        }
    });
}

/// The BuildKit backend: needs the `buildkit` feature and a live buildkitd.
///
/// Start one with:
/// ```sh
/// docker run -d --privileged -p 127.0.0.1:13434:1234 \
///   moby/buildkit:latest --addr tcp://0.0.0.0:1234
/// ```
#[cfg(feature = "buildkit")]
mod buildkit_backend {
    use super::*;
    use foundation_deployment_platform::docker::BuildBackend;

    fn buildkitd_addr() -> Option<String> {
        let addr = std::env::var("EWE_BUILDKITD_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:13434".to_string());
        match std::net::TcpStream::connect(&addr) {
            Ok(_) => Some(addr),
            Err(_) => {
                eprintln!("SKIP: no buildkitd at {addr} (set EWE_BUILDKITD_ADDR)");
                None
            }
        }
    }

    #[test]
    #[ignore = "requires a running buildkitd"]
    fn builds_an_inline_dockerfile_through_buildkitd() {
        let Some(addr) = buildkitd_addr() else { return };
        let _pool = initialize_pool(54, Some(4));

        foundation_core::valtron::block_on_future(async move {
            let t = tag("buildkit");

            let built = DockerFileConfig::new(
                "FROM alpine:3\nRUN echo built-by-buildkit > /marker\n",
            )
            .tag(&t)
            .backend(BuildBackend::BuildKit(addr))
            .build_once()
            .await
            .expect("buildkit solve should succeed");

            assert_eq!(built.image_tag, t);
            assert!(
                built.sha256.starts_with("sha256:"),
                "buildkitd should report the built image digest, got: {:?}",
                built.sha256
            );
        });
    }

    #[test]
    #[ignore = "requires a running buildkitd"]
    fn a_failing_dockerfile_fails_the_solve() {
        let Some(addr) = buildkitd_addr() else { return };
        let _pool = initialize_pool(54, Some(4));

        foundation_core::valtron::block_on_future(async move {
            let result = DockerFileConfig::new("FROM alpine:3\nRUN exit 7\n")
                .tag(tag("buildkit-fail"))
                .backend(BuildBackend::BuildKit(addr))
                .build_once()
                .await;

            match result {
                Err(trace) => {
                    let msg = format!("{trace}");
                    assert!(
                        msg.contains("Image build failed"),
                        "a failing solve should surface as ImageBuild, got: {msg}"
                    );
                }
                Ok(built) => panic!("a failing Dockerfile must not report success: {built:?}"),
            }
        });
    }
}
