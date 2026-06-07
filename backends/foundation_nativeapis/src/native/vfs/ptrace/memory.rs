/// Tracee memory helpers — read/write the traced process's memory via `process_vm_readv`/`process_vm_writev`.
///
/// These are significantly faster than `ptrace(PTRACE_PEEKDATA/POKEDATA)` for bulk transfers
/// because they don't require a context switch per word.

use std::io::IoSlice;

use nix::libc::{iovec, process_vm_readv, process_vm_writev};

use crate::shared::vfs::error::{VfsError, VfsResult};

/// Read a NUL-terminated string from the tracee's memory.
///
/// Reads in 64-byte chunks until a NUL byte is found. Returns the decoded string.
/// Fails after `max_len` bytes to prevent runaway reads.
pub fn read_tracee_string(pid: nix::unistd::Pid, addr: u64, max_len: usize) -> VfsResult<String> {
    let mut buf = Vec::with_capacity(256);
    let mut pos = addr;
    let chunk_size = 64usize;

    loop {
        if buf.len() >= max_len {
            return Err(VfsError::Backend {
                message: format!("string at {addr:#x} exceeds max_len={max_len}"),
            }.into());
        }

        let remaining = max_len - buf.len();
        let to_read = remaining.min(chunk_size);
        let mut chunk = vec![0u8; to_read];
        let n = read_tracee_buf_raw(pid, pos, &mut chunk)?;
        chunk.truncate(n);

        // Check for NUL terminator
        if let Some(nul_pos) = chunk.iter().position(|&b| b == 0) {
            buf.extend_from_slice(&chunk[..nul_pos]);
            break;
        }

        buf.extend_from_slice(&chunk);
        pos += chunk.len() as u64;
    }

    String::from_utf8(buf).map_err(|e| VfsError::Backend {
        message: format!("invalid UTF-8 in tracee string: {e}"),
    }.into())
}

/// Read a buffer from the tracee's memory.
pub fn read_tracee_buf(pid: nix::unistd::Pid, addr: u64, len: usize) -> VfsResult<Vec<u8>> {
    let mut buf = vec![0u8; len];
    read_tracee_buf_raw(pid, addr, &mut buf)?;
    Ok(buf)
}

/// Internal: read into a pre-allocated buffer.
fn read_tracee_buf_raw(pid: nix::unistd::Pid, addr: u64, buf: &mut [u8]) -> VfsResult<usize> {
    if buf.is_empty() {
        return Ok(0);
    }

    let remote_iov = iovec {
        iov_base: addr as *mut _,
        iov_len: buf.len(),
    };
    let local_iov = IoSlice::new(buf);
    let local_iov_c = iovec {
        iov_base: local_iov.as_ptr() as *mut _,
        iov_len: local_iov.len(),
    };

    let ret = unsafe {
        process_vm_readv(
            pid.as_raw(),
            &local_iov_c,
            1,
            &remote_iov,
            1,
            0, // flags
        )
    };

    match ret {
        -1 => Err(VfsError::Backend {
            message: format!("process_vm_readv failed at {addr:#x}: len={}", buf.len()),
        }.into()),
        n if n < 0 => Err(VfsError::Backend {
            message: format!("process_vm_readv returned negative: {n}"),
        }.into()),
        n => Ok(n as usize),
    }
}

/// Write a buffer to the tracee's memory.
pub fn write_tracee_buf(pid: nix::unistd::Pid, addr: u64, data: &[u8]) -> VfsResult<()> {
    if data.is_empty() {
        return Ok(());
    }

    let remote_iov = iovec {
        iov_base: addr as *mut _,
        iov_len: data.len(),
    };
    let local_iov = IoSlice::new(data);
    let local_iov_c = iovec {
        iov_base: local_iov.as_ptr() as *mut _,
        iov_len: local_iov.len(),
    };

    let ret = unsafe {
        process_vm_writev(
            pid.as_raw(),
            &local_iov_c,
            1,
            &remote_iov,
            1,
            0,
        )
    };

    match ret {
        -1 => Err(VfsError::Backend {
            message: format!("process_vm_writev failed at {addr:#x}: len={}", data.len()),
        }.into()),
        n if (n as usize) < data.len() => Err(VfsError::Backend {
            message: format!("process_vm_writev partial write: wrote {n} of {}", data.len()),
        }.into()),
        _ => Ok(()),
    }
}

/// Write a `libc::stat` struct to the tracee's memory for `stat`/`fstat`/`lstat` syscalls.
///
/// Uses `nix::sys::stat::Mode` to build the stat struct in a platform-independent way.
pub fn write_tracee_stat(pid: nix::unistd::Pid, addr: u64, meta: &crate::shared::vfs::types::VfsMetadata) -> VfsResult<()> {
    use crate::shared::vfs::types::VfsFileType;

    // Build stat manually using known x86_64 Linux layout.
    // This is simpler than fighting libc's platform-specific stat struct.
    // x86_64 Linux stat layout (from <bits/stat.h>):
    //   st_dev (8), st_ino (8), st_nlink (8), st_mode (4), st_uid (4),
    //   st_gid (4), __pad0 (4), st_rdev (8), st_size (8), st_blksize (8),
    //   st_blocks (8), st_atime (8), st_atime_nsec (8),
    //   st_mtime (8), st_mtime_nsec (8),
    //   st_ctime (8), st_ctime_nsec (8),
    //   __unused[3] (8*3) = total 144 bytes

    let mode = match meta.file_type {
        VfsFileType::Regular => libc::S_IFREG,
        VfsFileType::Directory => libc::S_IFDIR,
        VfsFileType::Symlink => libc::S_IFLNK,
    } as u32 | meta.permissions;

    let (mtime_sec, mtime_nsec) = meta.modified
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| (d.as_secs() as i64, d.subsec_nanos() as i64))
        .unwrap_or((0, 0));

    let (atime_sec, atime_nsec) = meta.accessed
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| (d.as_secs() as i64, d.subsec_nanos() as i64))
        .unwrap_or((0, 0));

    let (ctime_sec, ctime_nsec) = meta.created
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| (d.as_secs() as i64, d.subsec_nanos() as i64))
        .unwrap_or((0, 0));

    let size = meta.size as i64;
    let blocks = (meta.size + 511) / 512;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct X86Stat {
        st_dev: u64,
        st_ino: u64,
        st_nlink: u64,
        st_mode: u32,
        st_uid: u32,
        st_gid: u32,
        _pad0: u32,
        st_rdev: u64,
        st_size: i64,
        st_blksize: i64,
        st_blocks: i64,
        st_atime: i64,
        st_atime_nsec: i64,
        st_mtime: i64,
        st_mtime_nsec: i64,
        st_ctime: i64,
        st_ctime_nsec: i64,
        _unused: [i64; 3],
    }

    let stat = X86Stat {
        st_dev: 0,
        st_ino: meta.inode,
        st_nlink: 1,
        st_mode: mode,
        st_uid: meta.owner.0,
        st_gid: meta.owner.1,
        _pad0: 0,
        st_rdev: 0,
        st_size: size,
        st_blksize: 4096,
        st_blocks: blocks as i64,
        st_atime: atime_sec,
        st_atime_nsec: atime_nsec,
        st_mtime: mtime_sec,
        st_mtime_nsec: mtime_nsec,
        st_ctime: ctime_sec,
        st_ctime_nsec: ctime_nsec,
        _unused: [0; 3],
    };

    let stat_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            &stat as *const _ as *const u8,
            std::mem::size_of::<X86Stat>(),
        )
    };
    write_tracee_buf(pid, addr, stat_bytes)
}

/// Write a NUL-terminated string to the tracee's memory.
pub fn write_tracee_string(pid: nix::unistd::Pid, addr: u64, s: &str) -> VfsResult<()> {
    let bytes = s.as_bytes();
    let mut buf = Vec::with_capacity(bytes.len() + 1);
    buf.extend_from_slice(bytes);
    buf.push(0); // NUL terminator
    write_tracee_buf(pid, addr, &buf)
}
