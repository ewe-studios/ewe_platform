//! The root set-up order, its failures, and the end of a process identifier 1
//! slave. These tests use a recording fake: a mount needs root rights.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use foundation_context::Ctx;
use foundation_http::ShutdownSignal;

use foundation_sandbox::native::slave::pid1;
use foundation_sandbox::native::slave::Slave;
use foundation_sandbox::shared::error::{SandboxError, SlaveErrorCode};
use foundation_sandbox::shared::host::{GuestMounts, GuestPower, SlaveHost};
use foundation_sandbox::shared::rpc::messages::{
    GuestMount, PrepareRequest, PrepareResponse, SlaveLimits,
};
use foundation_sandbox::shared::spec::{FsType, RootMode, RootSetup, RootSource};

/// A fake that records every guest call and can fail one of them.
#[derive(Clone, Default)]
struct Recorder {
    calls: Arc<Mutex<Vec<String>>>,
    fail_at: Arc<Mutex<Option<usize>>>,
}

impl Recorder {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("the call list").clone()
    }

    fn set_failure(&self, index: usize) {
        *self.fail_at.lock().expect("the failure slot") = Some(index);
    }

    fn record(&self, call: String) -> io::Result<()> {
        let mut calls = self.calls.lock().expect("the call list");
        let index = calls.len();
        calls.push(call);
        if *self.fail_at.lock().expect("the failure slot") == Some(index) {
            return Err(io::Error::from_raw_os_error(libc::EPERM));
        }
        Ok(())
    }
}

impl GuestMounts for Recorder {
    fn mount(
        &self,
        source: &str,
        target: &Path,
        fstype: &str,
        flags: u64,
        data: &str,
    ) -> io::Result<()> {
        self.record(format!(
            "mount {source} {} {fstype} {flags} {data}",
            target.display()
        ))
    }

    fn mkdir_p(&self, path: &Path, _mode: u32) -> io::Result<()> {
        self.record(format!("mkdir {}", path.display()))
    }

    fn write_file(&self, path: &Path, _data: &[u8]) -> io::Result<()> {
        self.record(format!("write {}", path.display()))
    }
}

impl GuestPower for Recorder {
    fn sync(&self) {
        let _ = self.record("sync".to_string());
    }

    fn power_off(&self) -> io::Result<std::convert::Infallible> {
        let _ = self.record("power_off".to_string());
        Err(io::Error::from_raw_os_error(libc::EIO))
    }
}

fn host(recorder: &Recorder) -> Arc<SlaveHost> {
    Arc::new(SlaveHost {
        mounts: Arc::new(recorder.clone()),
        power: Arc::new(recorder.clone()),
    })
}

fn limits() -> SlaveLimits {
    SlaveLimits {
        chunk_bytes: 64 * 1024,
        max_exec_sessions: 128,
        output_drain_timeout_ms: 2_000,
        upload_idle_timeout_ms: 60_000,
        input_queue_bytes: 4 * 64 * 1024,
        max_message_bytes: 4 * 1024 * 1024,
        event_queue_len: 4,
    }
}

fn request(source: RootSource, mode: RootMode) -> PrepareRequest {
    PrepareRequest {
        root: RootSetup { source, mode },
        workdir: "/work".to_string(),
        env: Vec::new(),
        mounts: Vec::new(),
        dns: Vec::new(),
        limits: limits(),
    }
}

fn slave(recorder: &Recorder, plain_root: &str) -> Arc<Slave> {
    Slave::with_host(
        Ctx::host(),
        host(recorder),
        PathBuf::from(plain_root),
        Arc::new(ShutdownSignal::new()),
    )
}

/// The set-up order of an overlay root, a read-only root and a plain root is
/// the order of the feature document.
#[test]
fn root_setup_order() {
    // An ext4 lower layer in the overlay mode, with one guest mount and one
    // name server.
    let recorder = Recorder::default();
    let mut overlay = request(
        RootSource::BlockDevice {
            device: "/dev/vda".to_string(),
            fstype: FsType::Ext4,
        },
        RootMode::Overlay,
    );
    overlay.mounts = vec![GuestMount {
        tag: "share".to_string(),
        guest_path: "/mnt/share".to_string(),
        read_only: true,
    }];
    overlay.dns = vec!["1.1.1.1".to_string()];
    let answer: PrepareResponse = slave(&recorder, "/")
        .prepare_root(&overlay)
        .expect("the overlay root");
    assert_eq!(answer.root_mode, RootMode::Overlay);
    let read_only = libc::MS_RDONLY;
    let bind = libc::MS_BIND;
    let rec = libc::MS_REC;
    let expected = vec![
        "mkdir /run/ewe".to_string(),
        "mkdir /run/ewe/lower".to_string(),
        format!("mount /dev/vda /run/ewe/lower ext4 {read_only} "),
        "mkdir /run/ewe/rw".to_string(),
        "mount tmpfs /run/ewe/rw tmpfs 0 ".to_string(),
        "mkdir /run/ewe/rw/upper".to_string(),
        "mkdir /run/ewe/rw/work".to_string(),
        "mkdir /run/ewe/root".to_string(),
        "mount overlay /run/ewe/root overlay 0 lowerdir=/run/ewe/lower,upperdir=/run/ewe/rw/upper,workdir=/run/ewe/rw/work"
            .to_string(),
        "mkdir /run/ewe/root/proc".to_string(),
        format!("mount proc /run/ewe/root/proc  {} ", bind + rec),
        "mkdir /run/ewe/root/sys".to_string(),
        format!("mount sysfs /run/ewe/root/sys  {} ", bind + rec),
        "mkdir /run/ewe/root/dev".to_string(),
        format!("mount dev /run/ewe/root/dev  {} ", bind + rec),
        "mkdir /run/ewe/root/mnt/share".to_string(),
        format!("mount share /run/ewe/root/mnt/share virtiofs {read_only} "),
        "write /run/ewe/resolv.conf".to_string(),
        "write /run/ewe/root/etc/resolv.conf".to_string(),
        format!(
            "mount /run/ewe/resolv.conf /run/ewe/root/etc/resolv.conf  {bind} "
        ),
        "mkdir /run/ewe/root/work".to_string(),
    ];
    assert_eq!(recorder.calls(), expected);

    // A virtiofs lower layer in the read-only mode.
    let recorder = Recorder::default();
    let read_only_request = request(
        RootSource::VirtioFs {
            tag: "root".to_string(),
        },
        RootMode::ReadOnly,
    );
    let answer = slave(&recorder, "/")
        .prepare_root(&read_only_request)
        .expect("the read-only root");
    assert_eq!(answer.root_mode, RootMode::ReadOnly);
    let expected = vec![
        "mkdir /run/ewe".to_string(),
        "mkdir /run/ewe/lower".to_string(),
        format!("mount root /run/ewe/lower virtiofs {read_only} "),
        "mkdir /run/ewe/root".to_string(),
        format!(
            "mount /run/ewe/lower /run/ewe/root  {} ",
            bind + rec + read_only
        ),
        "mkdir /run/ewe/root/tmp".to_string(),
        "mount tmpfs /run/ewe/root/tmp tmpfs 0 ".to_string(),
        "mkdir /run/ewe/root/run".to_string(),
        "mount tmpfs /run/ewe/root/run tmpfs 0 ".to_string(),
        "mkdir /run/ewe/root/work".to_string(),
        "mount tmpfs /run/ewe/root/work tmpfs 0 ".to_string(),
        "mkdir /run/ewe/root/proc".to_string(),
        format!("mount proc /run/ewe/root/proc  {} ", bind + rec),
        "mkdir /run/ewe/root/sys".to_string(),
        format!("mount sysfs /run/ewe/root/sys  {} ", bind + rec),
        "mkdir /run/ewe/root/dev".to_string(),
        format!("mount dev /run/ewe/root/dev  {} ", bind + rec),
        "mkdir /run/ewe/root/work".to_string(),
    ];
    assert_eq!(recorder.calls(), expected);

    // The plain root does only the work directory.
    let recorder = Recorder::default();
    let plain = request(RootSource::Plain, RootMode::ReadOnly);
    let answer = slave(&recorder, "/a/root")
        .prepare_root(&plain)
        .expect("the plain root");
    assert_eq!(answer.root_mode, RootMode::Overlay);
    assert_eq!(recorder.calls(), vec!["mkdir /a/root/work".to_string()]);
}

/// A failed set-up step stops the set-up with a typed error.
#[test]
fn root_setup_failure() {
    let mut base = request(
        RootSource::BlockDevice {
            device: "/dev/vda".to_string(),
            fstype: FsType::Squashfs,
        },
        RootMode::Overlay,
    );
    base.dns = vec!["1.1.1.1".to_string()];
    // The count of the calls of a successful set-up.
    let recorder = Recorder::default();
    slave(&recorder, "/")
        .prepare_root(&base)
        .expect("the whole set-up");
    let total = recorder.calls().len();

    for index in 0..total {
        let recorder = Recorder::default();
        recorder.set_failure(index);
        let slave = slave(&recorder, "/");
        let error = slave
            .prepare_root(&base)
            .expect_err("the failed step stops the set-up");
        match error.current_context() {
            SandboxError::SlaveError { code, errno, .. } => {
                assert!(
                    matches!(
                        code,
                        SlaveErrorCode::RootSetupFailed | SlaveErrorCode::MountFailed
                    ),
                    "step {index} gives a set-up error, not {code}"
                );
                assert_eq!(*errno, Some(libc::EPERM));
            }
            other => panic!("step {index} gives an unexpected error: {other:?}"),
        }
        // The slave stays Booted.
        assert!(!slave.state().is_prepared());
    }
}

/// A process identifier 1 slave flushes the buffers and powers the machine off.
#[test]
fn pid1_never_returns() {
    let recorder = Recorder::default();
    let host = host(&recorder);
    let error = pid1::power_off(&host).expect_err("a power off that fails returns an error");
    assert_eq!(error.raw_os_error(), Some(libc::EIO));
    assert_eq!(
        recorder.calls(),
        vec!["sync".to_string(), "power_off".to_string()]
    );
}
