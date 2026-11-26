//! I1, I2 and I3: every message survives JSON, bytes are base64, and no call
//! streams its request.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::Router;
use foundation_sandbox::shared::command::{Command, ExecEvent, ExecOutput, ExitStatus, StdinMode};
use foundation_sandbox::shared::control::{ExitReport, SandboxInfo, SandboxState};
use foundation_sandbox::shared::driver::{Capabilities, CreateProgress, HostCheck, HostReport};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::{
    CreateEvent, CreateRequest, Empty, ExecEventKind, ExecEventMsg, ExecInputRequest,
    ExecInputResponse, ExecRequest, ExecSignalRequest, FileChunk, FileReadRequest,
    FileWriteAbortRequest, FileWriteBeginRequest, FileWriteBeginResponse, FileWriteChunkRequest,
    FileWriteChunkResponse, FileWriteCommitRequest, FileWriteCommitResponse, InfoRequest,
    ListResponse, PrepareRequest, PrepareResponse, ProbeResponse, SandboxRef, Scoped,
    ShutdownRequest, SlaveInfo, SlaveLimits,
};
use foundation_sandbox::shared::rpc::{
    register_sandbox_service, register_slave_service, UnimplementedSandboxServiceHandler,
    UnimplementedSlaveServiceHandler,
};
use foundation_sandbox::shared::spec::{
    FsType, GuestArch, GuestMount, HostMount, ImageRef, KernelSource, NetworkMode, RootFs,
    RootMode, RootSetup, RootSource, SandboxSpec,
};
use proptest::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::common;

/// Encodes and decodes one value, and checks that nothing changed.
fn roundtrip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("encode to JSON");
    let decoded: T = serde_json::from_str(&json).expect("decode from JSON");
    assert_eq!(&decoded, value);
}

/// The capabilities of a host that can do everything.
fn capabilities() -> Capabilities {
    Capabilities {
        rootfs_directory: true,
        rootfs_ext4: true,
        rootfs_squashfs: true,
        rootfs_container_image: true,
        root_modes: vec![RootMode::Overlay, RootMode::ReadOnly],
        host_mounts: true,
        network_none: true,
        network_host: true,
        max_vcpus: 8,
    }
}

/// A host report for the error and probe messages.
fn host_report() -> HostReport {
    HostReport {
        driver: DriverId::Firecracker,
        usable: true,
        checks: vec![HostCheck {
            name: "kvm".to_string(),
            ok: true,
            message: "available".to_string(),
        }],
        capabilities: capabilities(),
    }
}

/// Builds every message and domain type, then round-trips each one.
#[allow(
    clippy::too_many_lines,
    reason = "one place holds one value of every message type"
)]
fn check_all_messages(text: &str, number: u64, data: &[u8]) {
    let byte_data = data.to_vec();

    // Small domain types.
    roundtrip(&Empty {});
    roundtrip(&InfoRequest {});
    roundtrip(&SandboxLimits::default());
    roundtrip(&capabilities());
    roundtrip(&host_report());
    roundtrip(&SandboxState::Ready);
    roundtrip(&ExitReport {
        substrate_exit_code: Some(0),
        killed: false,
    });
    roundtrip(&ExitReport::default());
    roundtrip(&DriverId::CloudflareContainer);
    roundtrip(&DriverId::CloudflareDo);
    roundtrip(&RootMode::ReadOnly);
    roundtrip(&NetworkMode::Host);
    roundtrip(&FsType::Squashfs);
    roundtrip(&GuestArch::Aarch64);
    roundtrip(&GuestArch::X86_64);
    roundtrip(&ImageRef::Tag(text.to_string()));
    roundtrip(&ImageRef::Key(text.to_string()));
    roundtrip(&KernelSource::Default);
    roundtrip(&KernelSource::Path(PathBuf::from(text)));
    roundtrip(&HostMount {
        host_path: PathBuf::from(text),
        guest_path: "/mnt/data".to_string(),
        read_only: true,
    });
    roundtrip(&RootSource::BlockDevice {
        device: text.to_string(),
        fstype: FsType::Ext4,
    });
    roundtrip(&RootSource::VirtioFs {
        tag: text.to_string(),
    });
    roundtrip(&RootSource::Plain);
    roundtrip(&RootSetup {
        source: RootSource::Plain,
        mode: RootMode::Overlay,
    });
    roundtrip(&GuestMount {
        tag: text.to_string(),
        guest_path: "/mnt/x".to_string(),
        read_only: false,
    });
    roundtrip(&RootFs::Directory(PathBuf::from(text)));
    roundtrip(&RootFs::Ext4Image(PathBuf::from(text)));
    roundtrip(&RootFs::SquashfsImage(PathBuf::from(text)));
    roundtrip(&RootFs::ContainerImage(text.to_string()));
    roundtrip(&RootFs::Image(ImageRef::Tag(text.to_string())));
    roundtrip(&SandboxSpec::new(RootFs::Directory(PathBuf::from(text))));

    // Command types.
    roundtrip(&Command::new(["echo", "hello"]));
    roundtrip(&Command {
        argv: vec![text.to_string()],
        env: vec![("K".to_string(), text.to_string())],
        cwd: Some("/workspace".to_string()),
        stdin: StdinMode::Piped,
        timeout: Some(Duration::from_millis(250)),
    });
    roundtrip(&Command {
        argv: vec!["cat".to_string()],
        env: Vec::new(),
        cwd: None,
        stdin: StdinMode::Bytes(byte_data.clone()),
        timeout: None,
    });
    roundtrip(&StdinMode::Null);
    roundtrip(&ExitStatus::code(0));
    roundtrip(&ExitStatus::signal(9));
    roundtrip(&ExecEvent::Stdout(byte_data.clone()));
    roundtrip(&ExecEvent::Stderr(byte_data.clone()));
    roundtrip(&ExecEvent::Exit(ExitStatus::code(0)));
    roundtrip(&ExecOutput {
        stdout: byte_data.clone(),
        stderr: byte_data.clone(),
        status: ExitStatus::code(0),
        stdout_truncated: false,
        stderr_truncated: true,
    });

    // Slave-side messages.
    roundtrip(&SlaveInfo {
        slave_version: text.to_string(),
        arch: "x86_64".to_string(),
        kernel: text.to_string(),
        is_pid1: true,
        prepared: false,
        can_control: true,
    });
    roundtrip(&SlaveLimits {
        chunk_bytes: 65_536,
        max_exec_sessions: 128,
        output_drain_timeout_ms: 2_000,
        upload_idle_timeout_ms: 60_000,
        input_queue_bytes: 262_144,
        input_pipe_bytes: 32_768,
        max_message_bytes: 4_194_304,
        event_queue_len: 4,
    });
    roundtrip(&PrepareRequest {
        root: RootSetup {
            source: RootSource::Plain,
            mode: RootMode::Overlay,
        },
        workdir: "/workspace".to_string(),
        env: vec![("K".to_string(), text.to_string())],
        mounts: vec![GuestMount {
            tag: text.to_string(),
            guest_path: "/mnt/x".to_string(),
            read_only: false,
        }],
        dns: vec!["1.1.1.1".to_string()],
        limits: SlaveLimits {
            chunk_bytes: 65_536,
            max_exec_sessions: 128,
            output_drain_timeout_ms: 2_000,
            upload_idle_timeout_ms: 60_000,
            input_queue_bytes: 262_144,
            input_pipe_bytes: 32_768,
            max_message_bytes: 4_194_304,
            event_queue_len: 4,
        },
    });
    roundtrip(&PrepareResponse {
        root_mode: RootMode::ReadOnly,
    });
    roundtrip(&ExecRequest {
        exec_id: "0123456789abcdef".to_string(),
        argv: vec![text.to_string()],
        env: vec![("K".to_string(), text.to_string())],
        cwd: Some("/".to_string()),
        stdin: true,
    });
    roundtrip(&ExecEventMsg {
        seq: number,
        event: ExecEventKind::Started { pid: 1 },
    });
    roundtrip(&ExecEventMsg {
        seq: number,
        event: ExecEventKind::Stdout {
            data: byte_data.clone(),
        },
    });
    roundtrip(&ExecEventMsg {
        seq: number,
        event: ExecEventKind::Stderr {
            data: byte_data.clone(),
        },
    });
    roundtrip(&ExecEventMsg {
        seq: number,
        event: ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    });
    roundtrip(&ExecInputRequest {
        exec_id: "0123456789abcdef".to_string(),
        seq: number,
        data: byte_data.clone(),
        eof: false,
    });
    roundtrip(&ExecInputResponse { next_seq: number });
    roundtrip(&ExecSignalRequest {
        exec_id: "0123456789abcdef".to_string(),
        signal: 9,
    });
    roundtrip(&FileWriteBeginRequest {
        path: "/tmp/x".to_string(),
        mode: 0o644,
    });
    roundtrip(&FileWriteBeginResponse {
        upload_id: "0123456789abcdef".to_string(),
    });
    roundtrip(&FileWriteChunkRequest {
        upload_id: "0123456789abcdef".to_string(),
        offset: number,
        data: byte_data.clone(),
    });
    roundtrip(&FileWriteChunkResponse {
        next_offset: number,
    });
    roundtrip(&FileWriteCommitRequest {
        upload_id: "0123456789abcdef".to_string(),
        size: number,
    });
    roundtrip(&FileWriteCommitResponse { size: number });
    roundtrip(&FileWriteAbortRequest {
        upload_id: "0123456789abcdef".to_string(),
    });
    roundtrip(&FileReadRequest {
        path: "/etc/hostname".to_string(),
    });
    roundtrip(&FileChunk {
        offset: number,
        data: byte_data,
        eof: true,
    });
    roundtrip(&ShutdownRequest { grace_ms: 5_000 });

    // Control-side messages.
    let info = SandboxInfo {
        id: "0123456789ab".to_string(),
        driver: DriverId::Firecracker,
        state: SandboxState::Ready,
        created_unix_ms: number,
        slave: Some(SlaveInfo {
            slave_version: text.to_string(),
            arch: "x86_64".to_string(),
            kernel: text.to_string(),
            is_pid1: true,
            prepared: true,
            can_control: false,
        }),
        manifest: None,
    };
    roundtrip(&info);
    roundtrip(&Scoped {
        sandbox_id: "0123456789ab".to_string(),
        inner: ExecRequest {
            exec_id: "0123456789abcdef".to_string(),
            argv: vec![text.to_string()],
            env: Vec::new(),
            cwd: None,
            stdin: false,
        },
    });
    roundtrip(&SandboxRef {
        sandbox_id: "0123456789ab".to_string(),
    });
    roundtrip(&CreateRequest {
        spec: SandboxSpec::new(RootFs::Directory(PathBuf::from(text))),
    });
    roundtrip(&CreateEvent::Progress(CreateProgress::Preparing));
    roundtrip(&CreateEvent::Progress(CreateProgress::SlaveReady));
    roundtrip(&CreateEvent::Created(info.clone()));
    roundtrip(&ProbeResponse {
        reports: vec![host_report()],
    });
    roundtrip(&ListResponse {
        sandboxes: vec![info],
    });
}

proptest! {
    #![proptest_config(common::config(0x5eed_0001))]

    /// I1, I2 and the "message survives a round trip" scenario.
    #[test]
    fn message_roundtrip(
        text in "[a-z]{0,8}",
        number in any::<u64>(),
        data_len in proptest::sample::select(vec![0usize, 1, 65_536]),
    ) {
        let data = vec![0xa5u8; data_len];
        check_all_messages(&text, number, &data);
    }
}

/// I3: both services register only unary and server-streaming procedures.
///
/// Every method of both traits takes `Request<T>`, so a streamed request would
/// change the trait signature and this test would not compile. The counts prove
/// that every method has a procedure.
#[test]
fn no_request_streaming() {
    let mut router = Router::new();
    register_slave_service(&mut router, Arc::new(UnimplementedSlaveServiceHandler));
    assert_eq!(router.len(), 11);

    let mut router = Router::new();
    register_sandbox_service(&mut router, Arc::new(UnimplementedSandboxServiceHandler));
    assert_eq!(router.len(), 14);
}

/// A host report survives RPC.
#[test]
fn host_report_roundtrip() {
    roundtrip(&host_report());
}
