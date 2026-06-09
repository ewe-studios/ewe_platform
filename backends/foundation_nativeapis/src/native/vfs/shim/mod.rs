/// LD_PRELOAD VFS shim — libc interposition.
///
/// Compiled as a `cdylib` when `vfs-preload` feature is enabled.
/// Intercepts libc filesystem calls and routes virtual paths through
/// the real VFS infrastructure (`OverlayFileSystem<NativeFs, MemoryDelta>`).
///
/// Usage:
/// ```bash
/// LD_PRELOAD=libfoundation_nativeapis.so \
///   FOUNDATION_VFS_PREFIX=/virtual \
///   your_command
/// ```

#[cfg(feature = "vfs-preload")]
mod config;
#[cfg(feature = "vfs-preload")]
mod fd_table;
#[cfg(feature = "vfs-preload")]
mod real;

#[cfg(feature = "vfs-preload")]
pub use shim_exports::*;

#[cfg(feature = "vfs-preload")]
mod shim_exports {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};
    use std::sync::Arc;

    use once_cell::sync::Lazy;

    use foundation_core::valtron::{initialize_pool, PoolGuard};

    use crate::native::vfs::native_fs::NativeFs;
    use crate::shared::vfs::dynfs::DynFs;
    use crate::shared::vfs::memory_delta::MemoryDelta;
    use crate::shared::vfs::overlay_fs::OverlayFileSystem;
    use crate::shared::vfs::types::OpenMode;

    use super::config::ShimConfig;
    use super::fd_table::VirtualFdTable;

    static CONFIG: Lazy<ShimConfig> = Lazy::new(ShimConfig::from_env);

    /// VFS runtime — holds the type-erased VFS stack and the valtron pool guard.
    /// The pool guard must be kept alive so worker threads stay running for
    /// LibsqlDelta/TursoDelta async-through-sync execution.
    struct VfsRuntime {
        fs: DynFs,
        #[allow(dead_code)]
        guard: Option<PoolGuard>,
    }

    /// Type-erased VFS stack — supports any delta store at runtime.
    ///
    /// The delta backend is selected via `FOUNDATION_VFS_DELTA` env var.
    /// Supported: `memory` (default), `sqlite`, `turso`, `dir`, `d1`, `r2`.
    /// Each falls back to `memory` if the required feature flag is not enabled
    /// or if initialization fails.
    static VFS: Lazy<VfsRuntime> = Lazy::new(|| {
        let root = CONFIG.root_dir.clone().unwrap_or_else(|| ".".to_string());
        let base = NativeFs::new(root.clone()).unwrap_or_else(|_| NativeFs::new(".").unwrap());

        macro_rules! mem_fallback {
            ($msg:expr) => {{
                eprintln!("[vfs-shim] {} — falling back to memory", $msg);
                let delta = MemoryDelta::new();
                let overlay = OverlayFileSystem::new(base, delta);
                return VfsRuntime {
                    fs: DynFs::new(Arc::new(overlay)),
                    guard: None,
                };
            }};
        }

        match &CONFIG.delta_backend {
            super::config::DeltaBackend::Memory => {
                let delta = MemoryDelta::new();
                let overlay = OverlayFileSystem::new(base, delta);
                VfsRuntime {
                    fs: DynFs::new(Arc::new(overlay)),
                    guard: None,
                }
            }
            super::config::DeltaBackend::Sqlite => {
                #[cfg(feature = "vfs-sqlite")]
                {
                    // Initialize valtron pool for async-through-sync execution
                    let guard = initialize_pool(42, Some(4));
                    use crate::shared::vfs::libsql_delta::{LibsqlDelta, SyncLibsqlDelta};
                    let path = CONFIG.delta_path.clone().unwrap_or_else(|| {
                        let dir = std::env::temp_dir();
                        dir.join("vfs-delta.db").to_string_lossy().to_string()
                    });
                    let async_delta = match LibsqlDelta::new(&path) {
                        Ok(d) => d,
                        Err(e) => {
                            drop(guard);
                            mem_fallback!(format!("sqlite delta failed: {e}"))
                        }
                    };
                    let delta = SyncLibsqlDelta::new(async_delta);
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::new(Arc::new(overlay)),
                        guard: Some(guard),
                    }
                }
                #[cfg(not(feature = "vfs-sqlite"))]
                mem_fallback!("vfs-sqlite feature not enabled");
            }
            super::config::DeltaBackend::Turso => {
                #[cfg(feature = "vfs-turso")]
                {
                    // Initialize valtron pool for async-through-sync execution
                    let guard = initialize_pool(42, Some(4));
                    use crate::shared::vfs::turso_delta::TursoDelta;
                    let url = std::env::var("FOUNDATION_VFS_TURSO_URL")
                        .unwrap_or_else(|_| "libsql://localhost".to_string());
                    let token = std::env::var("FOUNDATION_VFS_TURSO_TOKEN").ok();
                    let delta = match TursoDelta::new(&url, token.as_deref()) {
                        Ok(d) => d,
                        Err(e) => {
                            drop(guard);
                            mem_fallback!(format!("turso delta failed: {e}"))
                        }
                    };
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::new(Arc::new(overlay)),
                        guard: Some(guard),
                    }
                }
                #[cfg(not(feature = "vfs-turso"))]
                mem_fallback!("vfs-turso feature not enabled");
            }
            super::config::DeltaBackend::Directory => {
                use crate::native::vfs::dir_delta::DirectoryDelta;
                let path = CONFIG.delta_path.clone().unwrap_or_else(|| {
                    let dir = std::env::temp_dir();
                    dir.join("vfs-delta").to_string_lossy().to_string()
                });
                let delta = match DirectoryDelta::new(&path) {
                    Ok(d) => d,
                    Err(e) => mem_fallback!(format!("directory delta failed: {e}")),
                };
                let overlay = OverlayFileSystem::new(base, delta);
                VfsRuntime {
                    fs: DynFs::new(Arc::new(overlay)),
                    guard: None,
                }
            }
            super::config::DeltaBackend::D1 => {
                // D1Delta implementation pending — types scaffolded in shared/vfs/d1_delta/
                #[cfg(feature = "vfs-d1")]
                {
                    use crate::shared::vfs::d1_delta::types::D1FsConfig;
                    let account_id = std::env::var("FOUNDATION_VFS_D1_ACCOUNT_ID")
                        .unwrap_or_else(|_| "your-account-id".to_string());
                    let api_token = std::env::var("FOUNDATION_VFS_D1_API_TOKEN")
                        .unwrap_or_else(|_| "your-api-token".to_string());
                    let database_id = std::env::var("FOUNDATION_VFS_D1_DATABASE_ID")
                        .unwrap_or_else(|_| "your-database-id".to_string());
                    let _config = D1FsConfig {
                        account_id, database_id, api_token,
                        chunk_size: 4096,
                    };
                    eprintln!("[vfs-shim] D1Delta: types scaffolded, implementation pending — using memory delta");
                    let delta = MemoryDelta::new();
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::new(Arc::new(overlay)),
                        guard: None,
                    }
                }
                #[cfg(not(feature = "vfs-d1"))]
                mem_fallback!("vfs-d1 feature not enabled");
            }
            super::config::DeltaBackend::R2 => {
                // R2Delta implementation pending — types scaffolded in shared/vfs/r2_delta/
                #[cfg(feature = "vfs-r2")]
                {
                    use crate::shared::vfs::r2_delta::types::R2FsConfig;
                    let account_id = std::env::var("FOUNDATION_VFS_R2_ACCOUNT_ID")
                        .unwrap_or_else(|_| "your-account-id".to_string());
                    let access_key = std::env::var("FOUNDATION_VFS_R2_ACCESS_KEY")
                        .unwrap_or_else(|_| "your-access-key".to_string());
                    let secret_key = std::env::var("FOUNDATION_VFS_R2_SECRET_KEY")
                        .unwrap_or_else(|_| "your-secret-key".to_string());
                    let bucket = std::env::var("FOUNDATION_VFS_R2_BUCKET")
                        .unwrap_or_else(|_| "your-bucket".to_string());
                    let _config = R2FsConfig {
                        account_id, bucket, access_key, secret_key,
                        chunk_size: 4096,
                    };
                    eprintln!("[vfs-shim] R2Delta: types scaffolded, implementation pending — using memory delta");
                    let delta = MemoryDelta::new();
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::new(Arc::new(overlay)),
                        guard: None,
                    }
                }
                #[cfg(not(feature = "vfs-r2"))]
                mem_fallback!("vfs-r2 feature not enabled");
            }
        }
    });

    static FD_TABLE: Lazy<VirtualFdTable> = Lazy::new(VirtualFdTable::new);

    const VIRTUAL_FD_BASE: c_int = 10_000;

    fn is_virtual_fd(fd: c_int) -> bool {
        fd >= VIRTUAL_FD_BASE
    }

    fn path_matches_prefix(path: &str) -> bool {
        CONFIG.prefixes.iter().any(|p| path.starts_with(p))
    }

    /// Strip the configured prefix from a path to get the VFS key.
    fn strip_prefix(path: &str) -> String {
        for prefix in &CONFIG.prefixes {
            if let Some(rest) = path.strip_prefix(prefix) {
                return if rest.is_empty() { "/".to_string() } else { rest.to_string() };
            }
        }
        path.to_string()
    }

    unsafe fn c_path_to_str(path: *const c_char) -> Option<&'static str> {
        if path.is_null() {
            return None;
        }
        unsafe { CStr::from_ptr(path) }.to_str().ok()
    }

    fn set_errno(err: c_int) {
        unsafe { *libc::__errno_location() = err };
    }

    // ── open / openat ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn open(pathname: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_open(path, flags, mode);
            }
        }
        unsafe { super::real::open(pathname, flags, mode) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn open64(pathname: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
        open(pathname, flags, mode)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if dirfd == libc::AT_FDCWD || path.starts_with('/') {
                if path_matches_prefix(path) {
                    return handle_virtual_open(path, flags, mode);
                }
            }
        }
        unsafe { super::real::openat(dirfd, pathname, flags, mode) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __openat64_time64(dirfd: c_int, pathname: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
        openat(dirfd, pathname, flags, mode)
    }

    // ── close ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn close(fd: c_int) -> c_int {
        if is_virtual_fd(fd) {
            FD_TABLE.remove(fd);
            return 0;
        }
        unsafe { super::real::close(fd) }
    }

    // ── read / write ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn read(fd: c_int, buf: *mut c_void, count: libc::size_t) -> libc::ssize_t {
        if is_virtual_fd(fd) {
            return handle_virtual_read(fd, buf, count);
        }
        unsafe { super::real::read(fd, buf, count) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn write(fd: c_int, buf: *const c_void, count: libc::size_t) -> libc::ssize_t {
        if is_virtual_fd(fd) {
            return handle_virtual_write(fd, buf, count);
        }
        unsafe { super::real::write(fd, buf, count) }
    }

    // ── stat ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn stat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_stat(path, statbuf);
            }
        }
        unsafe { super::real::stat(pathname, statbuf) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn lstat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
        stat(pathname, statbuf)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn fstat(fd: c_int, statbuf: *mut libc::stat) -> c_int {
        if is_virtual_fd(fd) {
            if let Some(path) = FD_TABLE.path(fd) {
                return handle_virtual_stat(&path, statbuf);
            }
            set_errno(libc::EBADF);
            return -1;
        }
        unsafe { super::real::fstat(fd, statbuf) }
    }

    // ── access ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn access(pathname: *const c_char, mode: c_int) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_access(path, mode);
            }
        }
        unsafe { super::real::access(pathname, mode) }
    }

    // ── lseek ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn lseek(fd: c_int, offset: libc::off_t, whence: c_int) -> libc::off_t {
        if is_virtual_fd(fd) {
            return handle_virtual_lseek(fd, offset, whence);
        }
        unsafe { super::real::lseek(fd, offset, whence) }
    }

    // ── mkdir / unlink / rename / rmdir ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mkdir(pathname: *const c_char, mode: libc::mode_t) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_mkdir(path, mode);
            }
        }
        unsafe { super::real::mkdir(pathname, mode) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unlink(pathname: *const c_char) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_unlink(path);
            }
        }
        unsafe { super::real::unlink(pathname) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rename(oldpath: *const c_char, newpath: *const c_char) -> c_int {
        let old = unsafe { c_path_to_str(oldpath) };
        let new = unsafe { c_path_to_str(newpath) };
        if let (Some(o), Some(n)) = (old, new) {
            if path_matches_prefix(o) || path_matches_prefix(n) {
                return handle_virtual_rename(o, n);
            }
        }
        unsafe { super::real::rename(oldpath, newpath) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rmdir(pathname: *const c_char) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_rmdir(path);
            }
        }
        unsafe { super::real::rmdir(pathname) }
    }

    // ── opendir / readdir / closedir ──

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn opendir(pathname: *const c_char) -> *mut libc::DIR {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_opendir(path);
            }
        }
        unsafe { libc::opendir(pathname) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn readdir(dirp: *mut libc::DIR) -> *mut libc::dirent {
        unsafe { libc::readdir(dirp) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn closedir(dirp: *mut libc::DIR) -> c_int {
        unsafe { libc::closedir(dirp) }
    }

    // ── VFS operation handlers ──

    fn handle_virtual_open(path: &str, flags: c_int, _mode: libc::mode_t) -> c_int {
        let key = strip_prefix(path);
        let creat = flags & libc::O_CREAT != 0;
        let trunc = flags & libc::O_TRUNC != 0;
        let excl = flags & libc::O_EXCL != 0;

        let mode = if flags & libc::O_ACCMODE == libc::O_RDONLY {
            OpenMode::Read
        } else if flags & libc::O_ACCMODE == libc::O_WRONLY {
            OpenMode::Write
        } else {
            OpenMode::ReadWrite
        };

        // Check existence via stat
        let exists = VFS.fs.exists(&key).unwrap_or(false);

        if exists {
            if creat && excl {
                set_errno(libc::EEXIST);
                return -1;
            }
            if trunc {
                // Truncate by writing empty
                let _ = VFS.fs.write_file(&key, b"");
            }
        } else if !creat {
            set_errno(libc::ENOENT);
            return -1;
        } else {
            // Create empty file
            let _ = VFS.fs.write_file(&key, b"");
        }

        // Open the file — DynFs.open_file returns ErasedFile directly
        match VFS.fs.open_file(&key, mode) {
            Ok(file) => {
                let vfd = FD_TABLE.alloc_fd();
                FD_TABLE.insert(vfd, key, file, flags);
                vfd
            }
            Err(_) => {
                set_errno(libc::EIO);
                -1
            }
        }
    }

    fn handle_virtual_read(fd: c_int, buf: *mut c_void, count: libc::size_t) -> libc::ssize_t {
        let buf_slice = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, count) };
        match FD_TABLE.read(fd, buf_slice) {
            Some(n) => n as libc::ssize_t,
            None => { set_errno(libc::EBADF); -1 }
        }
    }

    fn handle_virtual_write(fd: c_int, buf: *const c_void, count: libc::size_t) -> libc::ssize_t {
        let data = unsafe { std::slice::from_raw_parts(buf as *const u8, count) };
        match FD_TABLE.write(fd, data) {
            Some(n) => n as libc::ssize_t,
            None => { set_errno(libc::EBADF); -1 }
        }
    }

    fn handle_virtual_stat(path: &str, statbuf: *mut libc::stat) -> c_int {
        let key = strip_prefix(path);
        match VFS.fs.stat(&key) {
            Ok(meta) => {
                unsafe {
                    std::ptr::write_bytes(statbuf, 0, 1);
                    (*statbuf).st_size = meta.size as libc::off_t;
                    (*statbuf).st_mode = meta.permissions as libc::mode_t;
                    (*statbuf).st_uid = meta.owner.0;
                    (*statbuf).st_gid = meta.owner.1;
                    if let Some(t) = meta.modified {
                        (*statbuf).st_mtime = t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
                    }
                    if let Some(t) = meta.created {
                        (*statbuf).st_ctime = t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
                    }
                    if let Some(t) = meta.accessed {
                        (*statbuf).st_atime = t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
                    }
                }
                0
            }
            Err(_) => {
                set_errno(libc::ENOENT);
                -1
            }
        }
    }

    fn handle_virtual_access(path: &str, _mode: c_int) -> c_int {
        let key = strip_prefix(path);
        if VFS.fs.stat(&key).is_ok() {
            0
        } else {
            set_errno(libc::ENOENT);
            -1
        }
    }

    fn handle_virtual_lseek(fd: c_int, offset: libc::off_t, whence: c_int) -> libc::off_t {
        let cur = FD_TABLE.get_offset(fd).unwrap_or(0) as libc::off_t;
        let file_len = FD_TABLE.path(fd)
            .and_then(|p| VFS.fs.stat(&p).ok())
            .map(|m| m.size as libc::off_t).unwrap_or(0);

        let new_offset = match whence {
            libc::SEEK_SET => offset,
            libc::SEEK_CUR => cur + offset,
            libc::SEEK_END => file_len + offset,
            _ => { set_errno(libc::EINVAL); return -1; },
        };

        if new_offset < 0 { set_errno(libc::EINVAL); return -1; }
        FD_TABLE.set_offset(fd, new_offset as u64);
        new_offset
    }

    fn handle_virtual_mkdir(path: &str, _mode: libc::mode_t) -> c_int {
        let key = strip_prefix(path);
        match VFS.fs.mkdir(&key) {
            Ok(_) => 0,
            Err(_) => { set_errno(libc::EIO); -1 }
        }
    }

    fn handle_virtual_unlink(path: &str) -> c_int {
        let key = strip_prefix(path);
        match VFS.fs.remove(&key) {
            Ok(_) => 0,
            Err(_) => { set_errno(libc::ENOENT); -1 }
        }
    }

    fn handle_virtual_rename(old: &str, new: &str) -> c_int {
        let old_key = strip_prefix(old);
        let new_key = strip_prefix(new);
        match VFS.fs.rename(&old_key, &new_key) {
            Ok(_) => 0,
            Err(_) => { set_errno(libc::ENOENT); -1 }
        }
    }

    fn handle_virtual_rmdir(path: &str) -> c_int {
        let key = strip_prefix(path);
        match VFS.fs.remove(&key) {
            Ok(_) => 0,
            Err(_) => { set_errno(libc::ENOENT); -1 }
        }
    }

    fn handle_virtual_opendir(path: &str) -> *mut libc::DIR {
        let key = strip_prefix(path);
        match VFS.fs.open_dir(&key) {
            Ok(dir) => {
                // Store the directory handle in a global table keyed by a virtual fd
                // For now, return null — full opendir support needs a DIR* wrapper
                // that bridges VfsDirectory → libc::dirent
                drop(dir);
                set_errno(libc::ENOSYS);
                std::ptr::null_mut()
            }
            Err(_) => {
                set_errno(libc::ENOENT);
                std::ptr::null_mut()
            }
        }
    }
}
