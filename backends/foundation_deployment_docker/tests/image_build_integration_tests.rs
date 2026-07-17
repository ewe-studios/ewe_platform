//! End-to-end tests for `POST /build` against a real Docker daemon.
//!
//! WHY: `image_build` had never built anything. The generated request it used
//! set query parameters and sent **no body**, but `/build` takes the build
//! context as a tar stream in the body — so there was nothing to build. It also
//! threw the response away (`Ok(Vec::new())`), which matters because the daemon
//! reports **build failures inside a 200 response**: a `{"error": …}` object
//! part-way down the progress stream. Nothing called it, so none of this showed.
//!
//! WHAT is proven: an inline Dockerfile builds and is tagged; a context
//! directory's files are really uploaded (the image can `COPY` them and the
//! content comes back out); `build_args` reach the build; and a Dockerfile that
//! fails is surfaced as an error rather than a success.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test image_build_integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::client::build_context::ContextTar;
use foundation_deployment_docker::client::images::ImageBuildOptions;
use foundation_deployment_docker::{DockerClient, DockerError};

fn client() -> DockerClient {
    DockerClient::connect_unix("/var/run/docker.sock")
}

/// Unique-ish tag so parallel runs never collide on a daemon-global image name.
fn tag(name: &str) -> String {
    format!("ewe-build-it-{name}:{}", std::process::id())
}

async fn remove_image(c: &DockerClient, t: &str) {
    let _ = c.image_delete(t, Some(true), Some(false)).await;
}

#[valtron_test]
async fn builds_an_inline_dockerfile_and_tags_it() {
    let c = client();
    let t = tag("inline");
    remove_image(&c, &t).await;

    let context = ContextTar::from_inline_dockerfile(
        "Dockerfile",
        "FROM alpine:3\nRUN echo built-by-the-native-client > /marker\n",
    )
    .expect("tar the inline Dockerfile");

    let outcome = c
        .image_build(
            &context,
            &ImageBuildOptions {
                tag: Some(t.clone()),
                ..Default::default()
            },
        )
        .await
        .expect("build should succeed");

    assert!(
        outcome.logs.contains("FROM alpine:3"),
        "the build log should carry the daemon's steps, got: {:?}",
        outcome.logs
    );

    // The tag exists now — proof the build really happened, not just that the
    // request returned.
    let inspect = c.image_inspect(&t).await.expect("built image should exist");
    assert!(inspect.id.is_some(), "built image should report an id");

    remove_image(&c, &t).await;
}

#[valtron_test]
async fn context_directory_files_are_uploaded_and_copyable() {
    let c = client();
    let t = tag("context");
    remove_image(&c, &t).await;

    // A context on disk with a file the Dockerfile COPYs. If the tar body were
    // missing or malformed, the COPY fails and the build errors.
    let dir = std::env::temp_dir().join(format!("ewe-build-ctx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create context dir");
    std::fs::write(dir.join("payload.txt"), b"file-from-the-context").expect("write context file");

    let context = ContextTar::from_dir_with_dockerfile(
        &dir,
        "Dockerfile",
        "FROM alpine:3\nCOPY payload.txt /payload.txt\nRUN cat /payload.txt\n",
    )
    .expect("tar the context dir");

    let outcome = c
        .image_build(
            &context,
            &ImageBuildOptions {
                tag: Some(t.clone()),
                ..Default::default()
            },
        )
        .await
        .expect("build with a context should succeed");

    // `RUN cat` echoes the file's content into the build log, so this proves the
    // bytes made it from the host into the image.
    assert!(
        outcome.logs.contains("file-from-the-context"),
        "the context file's content should appear in the build log, got: {:?}",
        outcome.logs
    );

    remove_image(&c, &t).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn build_args_reach_the_build() {
    let c = client();
    let t = tag("args");
    remove_image(&c, &t).await;

    let context = ContextTar::from_inline_dockerfile(
        "Dockerfile",
        "FROM alpine:3\nARG GREETING=unset\nRUN echo arg-was-$GREETING\n",
    )
    .expect("tar the inline Dockerfile");

    let outcome = c
        .image_build(
            &context,
            &ImageBuildOptions {
                tag: Some(t.clone()),
                build_args: vec![("GREETING".to_string(), "delivered".to_string())],
                no_cache: true,
                ..Default::default()
            },
        )
        .await
        .expect("build should succeed");

    assert!(
        outcome.logs.contains("arg-was-delivered"),
        "the build arg should have reached the build, got: {:?}",
        outcome.logs
    );

    remove_image(&c, &t).await;
}

#[valtron_test]
async fn a_failing_build_is_an_error_not_a_success() {
    let c = client();
    let t = tag("failing");

    // The daemon streams this failure back inside a **200** response, so a client
    // that ignores the body (as this one used to) reports success for a build
    // that never produced an image.
    let context = ContextTar::from_inline_dockerfile(
        "Dockerfile",
        "FROM alpine:3\nRUN exit 7\n",
    )
    .expect("tar the inline Dockerfile");

    let result = c
        .image_build(
            &context,
            &ImageBuildOptions {
                tag: Some(t.clone()),
                no_cache: true,
                ..Default::default()
            },
        )
        .await;

    match result {
        Err(DockerError::BuildFailed(msg)) => {
            assert!(
                msg.contains('7') || msg.to_lowercase().contains("non-zero"),
                "the failure should name the failing command's exit, got: {msg:?}"
            );
        }
        Err(other) => panic!("expected BuildFailed, got: {other:?}"),
        Ok(outcome) => panic!("a failing Dockerfile must not report success: {outcome:?}"),
    }

    remove_image(&c, &t).await;
}
