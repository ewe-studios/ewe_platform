//! Source adapters: each gives the right artifact, and a bad checksum leaves
//! no file.

mod support;

use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_installer::{
    manifest::{AssetSpec, SourceSpec},
    source::{
        apt::AptSource, cargo::CargoSource, docker_image::DockerImageSource,
        github_release::GithubReleaseSource, mise::MiseSource, url::UrlSource, SourceAdapter,
    },
    Artifact,
};
use support::{
    deploy_none, entry, fake_context, sha256_hex, test_config, FakeCommands, FakeDocker,
    FakeHttpClient,
};

#[valtron_test]
async fn url_download_writes_the_file_and_checks_its_sha() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let body = b"hello world".to_vec();
    let http = FakeHttpClient::ok(body.clone());
    let ctx = fake_context(
        config.clone(),
        Arc::clone(&http),
        FakeCommands::new(),
        FakeDocker::new("sha256:x"),
    );
    let source = SourceSpec::Url {
        url: "https://example.com/agent-rpc".to_string(),
        sha256: sha256_hex(&body),
    };
    let item = entry("agent-rpc", "0.4.0", source, deploy_none());

    let artifact = UrlSource
        .resolve(&item, &ctx)
        .await
        .expect("the URL source resolves");

    let Artifact::File { path, sha256 } = &artifact else {
        panic!("expected a file artifact, got {artifact:?}");
    };
    assert_eq!(sha256, &sha256_hex(&body));
    assert_eq!(std::fs::read(path).expect("the file exists"), body);
    assert!(path.starts_with(config.artifact_dir.join("agent-rpc").join("0.4.0")));
    assert_eq!(
        http.seen(),
        vec!["https://example.com/agent-rpc".to_string()]
    );
}

#[valtron_test]
async fn a_bad_checksum_leaves_no_file() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let http = FakeHttpClient::ok(b"hello world".to_vec());
    let ctx = fake_context(
        config.clone(),
        http,
        FakeCommands::new(),
        FakeDocker::new("sha256:x"),
    );
    let source = SourceSpec::Url {
        url: "https://example.com/agent-rpc".to_string(),
        sha256: sha256_hex(b"other bytes"),
    };
    let item = entry("agent-rpc", "0.4.0", source, deploy_none());

    let error = UrlSource
        .resolve(&item, &ctx)
        .await
        .expect_err("the checksum is wrong");
    assert!(matches!(
        error,
        foundation_installer::InstallError::ChecksumMismatch { .. }
    ));

    let version_directory = config.artifact_dir.join("agent-rpc").join("0.4.0");
    let leftovers: Vec<_> = std::fs::read_dir(&version_directory)
        .expect("the version directory exists")
        .map(|item| item.expect("a directory item").file_name())
        .collect();
    assert!(leftovers.is_empty(), "expected no files, got {leftovers:?}");
}

#[valtron_test]
async fn github_release_picks_the_asset_of_the_target() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let body = b"release bytes".to_vec();
    let http = FakeHttpClient::ok(body.clone());
    let ctx = fake_context(
        config,
        Arc::clone(&http),
        FakeCommands::new(),
        FakeDocker::new("sha256:x"),
    )
    .with_target("x86_64-test-linux");
    let mut assets = std::collections::BTreeMap::new();
    assets.insert(
        "x86_64-test-linux".to_string(),
        AssetSpec {
            name: "agent-rpc-linux".to_string(),
            sha256: sha256_hex(&body),
        },
    );
    let source = SourceSpec::GithubRelease {
        repo: "ewe-studios/ewe_platform".to_string(),
        tag: "agent-rpc-v0.4.0".to_string(),
        assets,
    };
    let item = entry("agent-rpc", "0.4.0", source, deploy_none());

    let artifact = GithubReleaseSource
        .resolve(&item, &ctx)
        .await
        .expect("the release asset resolves");
    assert!(matches!(artifact, Artifact::File { .. }));
    assert_eq!(
        http.seen(),
        vec![
            "https://github.com/ewe-studios/ewe_platform/releases/download/agent-rpc-v0.4.0/agent-rpc-linux"
                .to_string()
        ]
    );
}

#[valtron_test]
async fn github_release_without_the_target_is_a_resolve_error() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        FakeDocker::new("sha256:x"),
    )
    .with_target("aarch64-test-linux");
    let mut assets = std::collections::BTreeMap::new();
    assets.insert(
        "x86_64-test-linux".to_string(),
        AssetSpec {
            name: "agent-rpc-linux".to_string(),
            sha256: sha256_hex(b"unused"),
        },
    );
    let source = SourceSpec::GithubRelease {
        repo: "ewe-studios/ewe_platform".to_string(),
        tag: "v1".to_string(),
        assets,
    };
    let item = entry("agent-rpc", "0.4.0", source, deploy_none());

    let error = GithubReleaseSource
        .resolve(&item, &ctx)
        .await
        .expect_err("the target has no asset");
    assert!(matches!(
        error,
        foundation_installer::InstallError::Resolve { .. }
    ));
}

#[valtron_test]
async fn cargo_installs_into_the_installer_root() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let bin_directory = config
        .artifact_dir
        .join("ripgrep")
        .join("14.1.0")
        .join("bin");
    std::fs::create_dir_all(&bin_directory).expect("the fake cargo root");
    std::fs::write(bin_directory.join("rg"), b"binary").expect("the fake rg");
    let commands = FakeCommands::new();
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        Arc::clone(&commands),
        FakeDocker::new("sha256:x"),
    );
    let source = SourceSpec::Cargo {
        krate: "ripgrep".to_string(),
        program: Some("rg".to_string()),
    };
    let item = entry("ripgrep", "14.1.0", source, deploy_none());

    let artifact = CargoSource
        .resolve(&item, &ctx)
        .await
        .expect("cargo resolves");
    let Artifact::Tool { program, path } = artifact else {
        panic!("expected a tool artifact");
    };
    assert_eq!(program, "rg");
    assert_eq!(path, bin_directory.join("rg"));
    let calls = commands.calls();
    assert_eq!(calls[0].0, "cargo");
    assert!(calls[0].1.contains(&"install".to_string()));
    assert!(calls[0].1.contains(&"--locked".to_string()));
    assert!(calls[0].1.contains(&"14.1.0".to_string()));
}

#[valtron_test]
async fn apt_finds_the_installed_program() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut config = test_config(directory.path());
    let program_directory = directory.path().join("usr-bin");
    std::fs::create_dir_all(&program_directory).expect("the program directory");
    std::fs::write(program_directory.join("rg"), b"binary").expect("the fake rg");
    config.program_search_dirs = vec![program_directory.clone()];
    let commands = FakeCommands::new();
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        Arc::clone(&commands),
        FakeDocker::new("sha256:x"),
    );
    let source = SourceSpec::Apt {
        package: "ripgrep".to_string(),
        program: Some("rg".to_string()),
    };
    let item = entry("ripgrep", "14.1.0", source, deploy_none());

    let artifact = AptSource.resolve(&item, &ctx).await.expect("apt resolves");
    let Artifact::Tool { program, path } = artifact else {
        panic!("expected a tool artifact");
    };
    assert_eq!(program, "rg");
    assert_eq!(path, program_directory.join("rg"));
    let calls = commands.calls();
    assert_eq!(calls[0].0, "apt-get");
    assert!(calls[0].1.contains(&"ripgrep=14.1.0".to_string()));
}

#[valtron_test]
async fn mise_asks_which_program_it_installed() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let program_path = directory.path().join("tools").join("rg");
    std::fs::create_dir_all(program_path.parent().expect("a parent")).expect("the tool dir");
    std::fs::write(&program_path, b"binary").expect("the fake tool");
    let commands = FakeCommands::new();
    commands.respond(
        "mise",
        &["which"],
        true,
        &program_path.display().to_string(),
    );
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        Arc::clone(&commands),
        FakeDocker::new("sha256:x"),
    );
    let source = SourceSpec::Mise {
        tool: "ripgrep".to_string(),
        program: Some("rg".to_string()),
    };
    let item = entry("ripgrep", "14.1.0", source, deploy_none());

    let artifact = MiseSource
        .resolve(&item, &ctx)
        .await
        .expect("mise resolves");
    let Artifact::Tool { program, path } = artifact else {
        panic!("expected a tool artifact");
    };
    assert_eq!(program, "rg");
    assert_eq!(path, program_path);
    let calls = commands.calls();
    assert!(calls.iter().any(|(program, args)| program == "mise"
        && args.contains(&"install".to_string())
        && args.contains(&"ripgrep@14.1.0".to_string())));
}

#[valtron_test]
async fn docker_image_records_the_digest() {
    let digest = format!("sha256:{}", "ab".repeat(32));
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let docker = FakeDocker::new(&digest);
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        Arc::clone(&docker),
    );
    let source = SourceSpec::DockerImage {
        repository: "ghcr.io/example/vector-db".to_string(),
        tag: "1.9.0".to_string(),
        digest: None,
    };
    let item = entry("vector-db", "1.9.0", source, deploy_none());

    let artifact = DockerImageSource
        .resolve(&item, &ctx)
        .await
        .expect("the image resolves");
    let Artifact::Image(image) = artifact else {
        panic!("expected an image artifact");
    };
    assert_eq!(image.repository, "ghcr.io/example/vector-db");
    assert_eq!(image.tag, "1.9.0");
    assert_eq!(image.digest, digest);
    assert_eq!(
        docker.pulls(),
        vec![("ghcr.io/example/vector-db".to_string(), "1.9.0".to_string())]
    );
}

#[valtron_test]
async fn docker_image_with_a_wrong_digest_is_refused() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let docker = FakeDocker::new(&format!("sha256:{}", "ab".repeat(32)));
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        docker,
    );
    let source = SourceSpec::DockerImage {
        repository: "ghcr.io/example/vector-db".to_string(),
        tag: "1.9.0".to_string(),
        digest: Some(format!("sha256:{}", "00".repeat(32))),
    };
    let item = entry("vector-db", "1.9.0", source, deploy_none());

    let error = DockerImageSource
        .resolve(&item, &ctx)
        .await
        .expect_err("the digest is wrong");
    assert!(matches!(
        error,
        foundation_installer::InstallError::ChecksumMismatch { .. }
    ));
}
