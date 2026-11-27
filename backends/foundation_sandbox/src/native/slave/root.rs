//! `Prepare`: the root that each program sees.
//!
//! The order of §6.5 of the feature is the order of this file. A failed step
//! stops the set-up and leaves the slave `Booted`. The mounts that it made stay
//! because the guest is thrown away.

use std::io;
use std::path::{Path, PathBuf};

use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::host::SlaveHost;
use crate::shared::rpc::messages::PrepareRequest;
use crate::shared::spec::{FsType, RootMode, RootSource};

use super::slave_error;

/// The directory that holds the lower layer and the writable layers.
const BASE: &str = "/run/ewe";
/// The mode of every directory that this file makes.
const DIR_MODE: u32 = 0o755;

/// Prepares the root of one `Prepare` request.
///
/// `plain_root` is the directory that [`RootSource::Plain`] uses as its root.
/// It is `/` in production. The tests use another directory for the file calls,
/// because a program cannot start under a root other than `/` without the right
/// to `chroot`.
///
/// # Errors
/// [`SlaveErrorCode::RootSetupFailed`] or [`SlaveErrorCode::MountFailed`], with
/// the step name and the operating system error.
pub fn prepare_root(
    host: &SlaveHost,
    request: &PrepareRequest,
    plain_root: &Path,
) -> SandboxResult<(RootMode, PathBuf)> {
    match &request.root.source {
        RootSource::Plain => prepare_plain(host, request, plain_root),
        RootSource::BlockDevice { .. } | RootSource::VirtioFs { .. } => {
            prepare_image(host, request)
        }
    }
}

/// The lower layer, the writable layer, the kernel file systems, the extra
/// mounts, the name servers and the work directory of an image root.
fn prepare_image(host: &SlaveHost, request: &PrepareRequest) -> SandboxResult<(RootMode, PathBuf)> {
    // The slave needs a writable directory for the layers, even when the root
    // itself is read-only.
    ensure_run_ewe(host)?;
    let root = Path::new(BASE).join("root");
    match request.root.mode {
        RootMode::Writable => prepare_writable(host, request, &root)?,
        RootMode::Overlay | RootMode::ReadOnly => {
            let lower = Path::new(BASE).join("lower");
            host.mounts.mkdir_p(&lower, DIR_MODE).map_err(|error| {
                root_setup_failure("mount point of the lower layer", &lower, &error)
            })?;
            match &request.root.source {
                RootSource::BlockDevice { device, fstype } => {
                    let fstype = match fstype {
                        FsType::Ext4 => "ext4",
                        FsType::Squashfs => "squashfs",
                    };
                    host.mounts
                        .mount(device, &lower, fstype, libc::MS_RDONLY, "")
                        .map_err(|error| mount_failure("lower layer", &lower, &error))?;
                }
                RootSource::VirtioFs { tag } => {
                    host.mounts
                        .mount(tag, &lower, "virtiofs", libc::MS_RDONLY, "")
                        .map_err(|error| mount_failure("lower layer", &lower, &error))?;
                }
                RootSource::Plain => {
                    return Err(slave_error(
                        SlaveErrorCode::Internal,
                        None,
                        "the plain root does not use the image set-up",
                    ));
                }
            }
            match request.root.mode {
                RootMode::Overlay => prepare_overlay(host, &lower, &root)?,
                RootMode::ReadOnly => prepare_read_only(host, request, &lower, &root)?,
                RootMode::Writable => {
                    return Err(slave_error(
                        SlaveErrorCode::Internal,
                        None,
                        "the writable root does not use a lower layer",
                    ));
                }
            }
        }
    }

    // Step 3: the kernel file systems.
    for (source, target) in [("proc", "/proc"), ("sysfs", "/sys"), ("dev", "/dev")] {
        let target = join_guest(&root, target);
        // The overlay makes a missing mount point; the read-only root fails on
        // one, because its own write is refused.
        host.mounts.mkdir_p(&target, DIR_MODE).map_err(|error| {
            root_setup_failure("kernel file system mount point", &target, &error)
        })?;
        host.mounts
            .mount(source, &target, "", libc::MS_BIND | libc::MS_REC, "")
            .map_err(|error| root_setup_failure("kernel file system mount", &target, &error))?;
    }

    // Step 4: the guest mounts.
    for mount in &request.mounts {
        let target = join_guest(&root, &mount.guest_path);
        host.mounts
            .mkdir_p(&target, DIR_MODE)
            .map_err(|error| mount_failure("mount point of a guest mount", &target, &error))?;
        let flags = if mount.read_only { libc::MS_RDONLY } else { 0 };
        host.mounts
            .mount(&mount.tag, &target, "virtiofs", flags, "")
            .map_err(|error| mount_failure("guest mount", &target, &error))?;
    }

    // Step 5: the name servers.
    write_name_servers(host, request, &root)?;

    // Step 6: the work directory.
    let workdir = join_guest(&root, &request.workdir);
    host.mounts
        .mkdir_p(&workdir, DIR_MODE)
        .map_err(|error| root_setup_failure("work directory", &workdir, &error))?;

    // Step 7: the result.
    Ok((request.root.mode, root))
}

/// The writable mode: the image itself is the root, mounted read-write.
///
/// There is no lower layer and no overlay. The mount is the image file, so a
/// program's write reaches the file and a clean unmount leaves it clean.
fn prepare_writable(host: &SlaveHost, request: &PrepareRequest, root: &Path) -> SandboxResult<()> {
    let RootSource::BlockDevice { device, fstype } = &request.root.source else {
        return Err(slave_error(
            SlaveErrorCode::RootSetupFailed,
            None,
            "the writable root needs a block device",
        ));
    };
    let fstype = match fstype {
        FsType::Ext4 => "ext4",
        FsType::Squashfs => {
            return Err(slave_error(
                SlaveErrorCode::RootSetupFailed,
                None,
                "the writable root needs an ext4 image, not squashfs",
            ));
        }
    };
    host.mounts
        .mkdir_p(root, DIR_MODE)
        .map_err(|error| root_setup_failure("root mount point", root, &error))?;
    host.mounts
        .mount(device, root, fstype, 0, "")
        .map_err(|error| mount_failure("writable root", root, &error))?;
    Ok(())
}

/// The overlay mode: a tmpfs for the writable layer, then the overlay mount.
fn prepare_overlay(host: &SlaveHost, lower: &Path, root: &Path) -> SandboxResult<()> {
    let rw = Path::new(BASE).join("rw");
    host.mounts
        .mkdir_p(&rw, DIR_MODE)
        .map_err(|error| root_setup_failure("writable layer directory", &rw, &error))?;
    host.mounts
        .mount("tmpfs", &rw, "tmpfs", 0, "")
        .map_err(|error| mount_failure("writable layer", &rw, &error))?;
    let upper = rw.join("upper");
    let work = rw.join("work");
    for directory in [&upper, &work] {
        host.mounts
            .mkdir_p(directory, DIR_MODE)
            .map_err(|error| root_setup_failure("overlay directory", directory, &error))?;
    }
    host.mounts
        .mkdir_p(root, DIR_MODE)
        .map_err(|error| root_setup_failure("root mount point", root, &error))?;
    let data = format!(
        "lowerdir={},upperdir={},workdir={}",
        lower.display(),
        upper.display(),
        work.display()
    );
    host.mounts
        .mount("overlay", root, "overlay", 0, &data)
        .map_err(|error| {
            if error.raw_os_error() == Some(libc::ENODEV) {
                slave_error(
                    SlaveErrorCode::MountFailed,
                    Some(libc::ENODEV),
                    format!(
                        "mount overlay on {}: the kernel has no overlayfs",
                        root.display()
                    ),
                )
            } else {
                mount_failure("overlay root", root, &error)
            }
        })?;
    Ok(())
}

/// The read-only mode: a read-only bind of the lower layer, and a tmpfs for
/// `/tmp`, `/run` and the work directory.
fn prepare_read_only(
    host: &SlaveHost,
    request: &PrepareRequest,
    lower: &Path,
    root: &Path,
) -> SandboxResult<()> {
    host.mounts
        .mkdir_p(root, DIR_MODE)
        .map_err(|error| root_setup_failure("root mount point", root, &error))?;
    host.mounts
        .mount(
            &lower.display().to_string(),
            root,
            "",
            libc::MS_BIND | libc::MS_REC | libc::MS_RDONLY,
            "",
        )
        .map_err(|error| mount_failure("read-only root", root, &error))?;

    let workdir = join_guest(root, &request.workdir);
    for target in [join_guest(root, "/tmp"), join_guest(root, "/run"), workdir] {
        // The root is read-only: an existing directory accepts `mkdir_p`, and a
        // missing one fails with the write error.
        host.mounts.mkdir_p(&target, DIR_MODE).map_err(|error| {
            root_setup_failure("writable directory of a read-only root", &target, &error)
        })?;
        host.mounts
            .mount("tmpfs", &target, "tmpfs", 0, "")
            .map_err(|error| mount_failure("writable directory", &target, &error))?;
    }
    Ok(())
}

/// The plain mode: the slave's own root, with the name servers and the work
/// directory only.
///
/// A container already holds the name server file that its runtime wrote, and a
/// container's process identifier (PID) 1 has no right to bind-mount over it.
/// The plain root in a container therefore writes no name server file (feature
/// 10).
fn prepare_plain(
    host: &SlaveHost,
    request: &PrepareRequest,
    plain_root: &Path,
) -> SandboxResult<(RootMode, PathBuf)> {
    if !super::pid1::in_container() {
        write_name_servers(host, request, plain_root)?;
    }
    let workdir = join_guest(plain_root, &request.workdir);
    host.mounts
        .mkdir_p(&workdir, DIR_MODE)
        .map_err(|error| root_setup_failure("work directory", &workdir, &error))?;
    // The mode is ignored: the caller already holds a private copy of its
    // image. The answer reports the mode that the slave applied.
    Ok((RootMode::Overlay, plain_root.to_path_buf()))
}

/// Makes `BASE` a writable directory. When `/run` is read-only, the slave puts
/// a tmpfs on it and tries again.
fn ensure_run_ewe(host: &SlaveHost) -> SandboxResult<()> {
    let base = Path::new(BASE);
    match host.mounts.mkdir_p(base, DIR_MODE) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::EROFS) => {
            host.mounts
                .mount("tmpfs", Path::new("/run"), "tmpfs", 0, "")
                .map_err(|error| mount_failure("tmpfs on /run", Path::new("/run"), &error))?;
            host.mounts
                .mkdir_p(base, DIR_MODE)
                .map_err(|error| root_setup_failure("run directory", base, &error))
        }
        Err(error) => Err(root_setup_failure("run directory", base, &error)),
    }
}

/// Writes the name server file and bind-mounts it into the root.
fn write_name_servers(
    host: &SlaveHost,
    request: &PrepareRequest,
    root: &Path,
) -> SandboxResult<()> {
    if request.dns.is_empty() {
        return Ok(());
    }
    let file = Path::new(BASE).join("resolv.conf");
    let mut body = String::new();
    for address in &request.dns {
        body.push_str("nameserver ");
        body.push_str(address);
        body.push('\n');
    }
    host.mounts
        .write_file(&file, body.as_bytes())
        .map_err(|error| root_setup_failure("name server file", &file, &error))?;

    let target = join_guest(root, "/etc/resolv.conf");
    match request.root.mode {
        RootMode::Overlay => {
            // The overlay makes the file when the image has none. A failure to
            // write it here means that the image path cannot hold the file.
            if let Err(error) = host.mounts.write_file(&target, b"") {
                if error.raw_os_error() != Some(libc::EROFS) {
                    return Err(root_setup_failure("name server target", &target, &error));
                }
            }
        }
        RootMode::Writable => {
            // The file is in the image itself. Make it only when it is missing:
            // an image that has its own name servers keeps them, and the bind
            // mount below hides them for this run. `symlink_metadata` does not
            // follow a link, so a link in the image cannot name another file.
            if std::fs::symlink_metadata(&target).is_err() {
                host.mounts
                    .write_file(&target, b"")
                    .map_err(|error| root_setup_failure("name server target", &target, &error))?;
            }
        }
        RootMode::ReadOnly => {
            // The read-only root must already hold the file. The bind mount
            // below reports a missing one.
        }
    }
    host.mounts
        .mount(&file.display().to_string(), &target, "", libc::MS_BIND, "")
        .map_err(|error| root_setup_failure("name server mount", &target, &error))
}

/// A guest path joined below the root. A leading `/` does not replace the root.
fn join_guest(root: &Path, guest: &str) -> PathBuf {
    root.join(guest.trim_start_matches('/'))
}

/// A `RootSetupFailed` error with the step name and the operating system error.
pub(crate) fn root_setup_failure(
    step: &str,
    path: &Path,
    error: &io::Error,
) -> foundation_errstacks::ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::RootSetupFailed,
        error.raw_os_error(),
        format!("{step} at {}: {error}", path.display()),
    )
}

/// A `MountFailed` error with the step name and the operating system error.
pub(crate) fn mount_failure(
    step: &str,
    path: &Path,
    error: &io::Error,
) -> foundation_errstacks::ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::MountFailed,
        error.raw_os_error(),
        format!("{step} at {}: {error}", path.display()),
    )
}
