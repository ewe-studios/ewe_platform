//! The command-line tests of feature 06.
//!
//! The argument tests need no sandbox. The two slave tests start the real
//! `agent-slave` program; they need Linux and the `slave` feature.

use clap::Parser as _;

use foundation_sandbox::cli::sandbox::{run, SandboxCli, SandboxCommand};
use foundation_sandbox::cli::{parse_connect, parse_listen};

#[test]
fn run_parses_a_program_after_two_hyphens() {
    let cli = SandboxCli::try_parse_from([
        "agent-sandbox",
        "run",
        "--rootfs",
        "/images/root.ext4",
        "--",
        "echo",
        "hello",
    ])
    .expect("the run command parses");
    let SandboxCommand::Run(args) = cli.command else {
        panic!("the command is not run");
    };
    assert_eq!(args.program, vec!["echo", "hello"]);
    assert_eq!(
        args.spec.rootfs.expect("the rootfs"),
        std::path::PathBuf::from("/images/root.ext4")
    );
}

#[test]
fn every_subcommand_parses() {
    for command in [
        vec!["agent-sandbox", "serve", "--listen", "unix:/tmp/sb.sock"],
        vec!["agent-sandbox", "create", "--connect", "tcp:127.0.0.1:9"],
        vec!["agent-sandbox", "list", "--connect", "unix:/tmp/sb.sock"],
        vec![
            "agent-sandbox",
            "stop",
            "--connect",
            "unix:/tmp/sb.sock",
            "aaaaaaaaaaaa",
        ],
        vec![
            "agent-sandbox",
            "kill",
            "--connect",
            "unix:/tmp/sb.sock",
            "aaaaaaaaaaaa",
        ],
        vec!["agent-sandbox", "doctor"],
    ] {
        SandboxCli::try_parse_from(command).expect("the sub-command parses");
    }
    SandboxCli::try_parse_from([
        "agent-sandbox",
        "exec",
        "--connect",
        "unix:/tmp/sb.sock",
        "aaaaaaaaaaaa",
        "--",
        "echo",
        "ok",
    ])
    .expect("the exec command parses");
}

#[test]
fn listen_and_connect_forms() {
    assert!(matches!(
        parse_listen("unix:/tmp/sb.sock"),
        Ok(foundation_sandbox::shared::listen::ListenAddress::Unix { .. })
    ));
    assert!(matches!(
        parse_listen("tcp:127.0.0.1:9"),
        Ok(foundation_sandbox::shared::listen::ListenAddress::Tcp { .. })
    ));
    assert!(matches!(
        parse_listen("vsock:1024"),
        Ok(foundation_sandbox::shared::listen::ListenAddress::Vsock { port: 1024 })
    ));
    assert!(parse_listen("nonsense").is_err());

    assert!(parse_connect("unix:/tmp/sb.sock").is_ok());
    assert!(parse_connect("tcp:127.0.0.1:9").is_ok());
    assert!(parse_connect("https://control.example.com").is_ok());
    assert!(parse_connect("nonsense").is_err());
}

#[test]
fn tcp_serve_without_a_token_is_usage() {
    let cli = SandboxCli::try_parse_from(["agent-sandbox", "serve", "--listen", "tcp:127.0.0.1:0"])
        .expect("the serve command parses");
    let code = run(foundation_context::Ctx::host(), cli);
    assert_eq!(code, 64);
}

#[test]
fn empty_token_file_is_usage() {
    let path = std::env::temp_dir().join(format!(
        "ewe-f06-empty-token-{}-{}",
        std::process::id(),
        std::line!()
    ));
    std::fs::write(&path, b"").expect("the empty token file");
    let cli = SandboxCli::try_parse_from([
        "agent-sandbox",
        "serve",
        "--listen",
        "tcp:127.0.0.1:0",
        "--token-file",
        path.to_str().expect("the path is text"),
    ])
    .expect("the serve command parses");
    let code = run(foundation_context::Ctx::host(), cli);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 64);
}

#[cfg(all(feature = "slave", target_os = "linux"))]
mod slave_program {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::time::Duration;

    use foundation_connectrpc::shared::transport::Transport;
    use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx};
    use foundation_netio::HttpClientBuilder;

    use foundation_sandbox::shared::rpc::messages::{InfoRequest, SlaveInfo};
    use foundation_sandbox::shared::rpc::SlaveServiceClient;

    /// A short socket path below `/tmp`.
    fn socket_path(tag: &str) -> PathBuf {
        PathBuf::from(format!(
            "/tmp/ewe-f06-slave-{tag}-{}.sock",
            std::process::id()
        ))
    }

    /// Calls `Info` once.
    fn info(socket: &Path) -> Result<SlaveInfo, String> {
        let http = HttpClientBuilder::new()
            .unix_socket(socket)
            .read_timeout(Duration::from_secs(10))
            .build();
        let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http));
        let client = SlaveServiceClient::new(
            transport,
            "http://slave.local",
            ClientOptions::new().with_codec("json"),
        )
        .map_err(|error| error.to_string())?;
        let client = Arc::new(client);
        crate::pool::block_on(async move {
            client
                .info(
                    RpcCtx::background().with_deadline(Duration::from_secs(10)),
                    Request::new(InfoRequest {}),
                )
                .await
        })
        .map(|response| response.msg)
        .map_err(|error| error.to_string())
    }

    #[test]
    fn slave_cli_listen_unix() {
        let socket = socket_path("listen");
        let mut child = Command::new(env!("CARGO_BIN_EXE_agent-slave"))
            .arg("--listen")
            .arg(format!("unix:{}", socket.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("agent-slave starts");
        let mut attempts = 0;
        loop {
            match info(&socket) {
                Ok(info) => {
                    assert!(!info.is_pid1, "the test slave is not process identifier 1");
                    break;
                }
                Err(_) => {
                    attempts += 1;
                    assert!(attempts < 500, "the slave did not answer within 10 s");
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&socket);
    }

    #[test]
    fn slave_cli_bad_arg() {
        let status = Command::new(env!("CARGO_BIN_EXE_agent-slave"))
            .arg("--listen")
            .arg("nonsense")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("agent-slave starts");
        assert_eq!(status.code(), Some(64));
    }
}

#[test]
fn cli_image_build_parse() {
    let cli = SandboxCli::try_parse_from([
        "agent-sandbox",
        "image",
        "build",
        "-f",
        "/tmp/Sandboxfile",
        "--tag",
        "app",
        "--size-mib",
        "512",
        "--no-cache",
        "/tmp/context",
    ])
    .expect("the image build command parses");
    let SandboxCommand::Image(image) = cli.command else {
        panic!("the command is not image");
    };
    let foundation_sandbox::cli::sandbox::ImageCommand::Build(args) = image.command else {
        panic!("the sub-command is not build");
    };
    assert_eq!(
        args.file,
        Some(std::path::PathBuf::from("/tmp/Sandboxfile"))
    );
    assert_eq!(args.tag.as_deref(), Some("app"));
    assert_eq!(args.size_mib, Some(512));
    assert!(args.no_cache);
    assert_eq!(args.context, std::path::PathBuf::from("/tmp/context"));

    // The defaults: the file beside the context, the current directory.
    let cli = SandboxCli::try_parse_from(["agent-sandbox", "image", "build"])
        .expect("the image build defaults parse");
    let SandboxCommand::Image(image) = cli.command else {
        panic!("the command is not image");
    };
    let foundation_sandbox::cli::sandbox::ImageCommand::Build(args) = image.command else {
        panic!("the sub-command is not build");
    };
    assert_eq!(args.file, None);
    assert_eq!(args.context, std::path::PathBuf::from("."));
}

#[test]
fn cli_image_build_flow() {
    use foundation_sandbox::cli::sandbox::{run, SandboxCli};

    let directory = std::env::temp_dir().join(format!(
        "ewe-f14-cli-{}-{}",
        std::process::id(),
        std::line!()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the test directory is made");

    // A syntax error names its line and exits with 125.
    let file = directory.join("Sandboxfile");
    std::fs::write(&file, "FROM alpine:3.22\nADD onlyone\n").expect("the file is written");
    let cli = SandboxCli::try_parse_from([
        "agent-sandbox",
        "image",
        "build",
        "-f",
        file.to_str().expect("the file path is text"),
        directory.to_str().expect("the directory path is text"),
    ])
    .expect("the command parses");
    assert_eq!(run(foundation_context::Ctx::host(), cli), 125);

    // A missing file exits with 125.
    let cli = SandboxCli::try_parse_from([
        "agent-sandbox",
        "image",
        "build",
        "-f",
        directory.join("absent").to_str().expect("the path is text"),
        directory.to_str().expect("the directory path is text"),
    ])
    .expect("the command parses");
    assert_eq!(run(foundation_context::Ctx::host(), cli), 125);
    let _ = std::fs::remove_dir_all(&directory);
}
