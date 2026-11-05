//! `LD_PRELOAD` VFS shim — libc interposition.
//!
//! Compiled as a `cdylib` when `vfs-preload` feature is enabled.
//! Intercepts libc filesystem calls and routes virtual paths through
//! the real VFS infrastructure (`OverlayFileSystem<NativeFs, MemoryDelta>`).
//!
//! Usage:
//! ```bash
//! LD_PRELOAD=libfoundation_nativeapis.so \
//!   FOUNDATION_VFS_PREFIX=/virtual \
//!   your_command
//! ```

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

    use tracing;

    #[cfg(any(feature = "vfs-sqlite", feature = "vfs-turso"))]
    use foundation_core::valtron::initialize_pool;
    use foundation_core::valtron::PoolGuard;

    #[cfg(feature = "vfs-sqlite")]
    use crate::native::vfs::libsql_delta::LibsqlDelta;
    use crate::native::vfs::native_fs::NativeFs;
    #[cfg(feature = "vfs-turso")]
    use crate::native::vfs::turso_delta::TursoDelta;
    use crate::shared::vfs::dynfs::DynFs;
    #[cfg(any(feature = "vfs-sqlite", feature = "vfs-turso"))]
    use crate::shared::vfs::exec_async::exec_async;
    use crate::shared::vfs::memory_delta::MemoryDelta;
    use crate::shared::vfs::overlay_fs::OverlayFileSystem;
    use crate::shared::vfs::types::OpenMode;

    use super::config::ShimConfig;
    use super::fd_table::VirtualFdTable;

    static CONFIG: std::sync::LazyLock<ShimConfig> = std::sync::LazyLock::new(ShimConfig::from_env);

    /// VFS runtime — holds the type-erased VFS stack and the valtron pool guard.
    /// The pool guard must be kept alive so worker threads stay running for
    /// `LibsqlDelta` async-through-sync execution.
    struct VfsRuntime {
        fs: DynFs,
        #[allow(dead_code)]
        guard: Option<PoolGuard>,
    }

    /// Type-erased VFS stack — supports any delta store at runtime.
    ///
    /// The delta backend is selected via `FOUNDATION_VFS_DELTA` env var.
    /// Supported: `memory` (default), `sqlite`, `turso`, `dir`, `d1`, `r2`.
    /// `sqlite`, `turso` and `dir` fall back to `memory` if init fails. `d1`
    /// and `r2` never fall back; they panic if the feature is missing or init fails.
    static VFS: std::sync::LazyLock<VfsRuntime> = std::sync::LazyLock::new(|| {
        let root = CONFIG.root_dir.clone().unwrap_or_else(|| ".".to_string());
        let base = NativeFs::new(root.clone()).unwrap_or_else(|_| NativeFs::new(".").unwrap());

        macro_rules! mem_fallback {
            ($msg:expr) => {{
                tracing::warn!("[vfs-shim] {} — falling back to memory", $msg);
                let delta = MemoryDelta::new();
                let overlay = OverlayFileSystem::new(base, delta);
                return VfsRuntime {
                    fs: DynFs::from_async(overlay),
                    guard: None,
                };
            }};
        }

        match &CONFIG.delta_backend {
            super::config::DeltaBackend::Memory => {
                let delta = MemoryDelta::new();
                let overlay = OverlayFileSystem::new(base, delta);
                VfsRuntime {
                    fs: DynFs::from_async(overlay),
                    guard: None,
                }
            }
            super::config::DeltaBackend::Sqlite => {
                #[cfg(feature = "vfs-sqlite")]
                {
                    // Initialize valtron pool for async-through-sync execution
                    let guard = initialize_pool(42, Some(4));
                    let path = CONFIG.delta_path.clone().unwrap_or_else(|| {
                        let dir = std::env::temp_dir();
                        dir.join("vfs-delta.db").to_string_lossy().to_string()
                    });
                    let delta = match exec_async(async move { LibsqlDelta::new(&path).await }) {
                        Ok(d) => d,
                        Err(e) => {
                            drop(guard);
                            mem_fallback!(format!("sqlite delta failed: {e}"))
                        }
                    };
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::from_async(overlay),
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
                    let path = CONFIG.delta_path.clone().unwrap_or_else(|| {
                        let dir = std::env::temp_dir();
                        dir.join("vfs-turso.db").to_string_lossy().to_string()
                    });
                    let delta = match exec_async(async move { TursoDelta::new(&path).await }) {
                        Ok(d) => d,
                        Err(e) => {
                            drop(guard);
                            mem_fallback!(format!("turso delta failed: {e}"))
                        }
                    };
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::from_async(overlay),
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
                    fs: DynFs::from_async(overlay),
                    guard: None,
                }
            }
            super::config::DeltaBackend::D1 => {
                #[cfg(feature = "vfs-d1")]
                {
                    use foundation_core::valtron::initialize_pool;
                    let guard = initialize_pool(42, Some(4));
                    // Do not drop `guard` in the closure: the closure would move
                    // it and the runtime below could not store it. A panic unwinds
                    // this frame and drops the guard at once anyway.
                    let delta = build_d1_delta()
                        .unwrap_or_else(|e| panic!("[vfs-shim] D1 delta init failed: {e}"));
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::from_async(overlay),
                        guard: Some(guard),
                    }
                }
                #[cfg(not(feature = "vfs-d1"))]
                panic!("[vfs-shim] DeltaBackend::D1 needs the vfs-d1 feature");
            }
            super::config::DeltaBackend::R2 => {
                #[cfg(feature = "vfs-r2")]
                {
                    use foundation_core::valtron::initialize_pool;
                    let guard = initialize_pool(42, Some(4));
                    let delta = build_r2_delta()
                        .unwrap_or_else(|e| panic!("[vfs-shim] R2 delta init failed: {e}"));
                    let overlay = OverlayFileSystem::new(base, delta);
                    VfsRuntime {
                        fs: DynFs::from_async(overlay),
                        guard: Some(guard),
                    }
                }
                #[cfg(not(feature = "vfs-r2"))]
                panic!("[vfs-shim] DeltaBackend::R2 needs the vfs-r2 feature");
            }
        }
    });

    /// A D1 delta for the shim. Reads the Cloudflare credential variables.
    /// When `LOCAL_CF_API_BASE` is set (the API prefix of wrangler's local explorer
    /// API, `http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`), the database id
    /// may come from `LOCAL_D1_DATABASE_ID` and test credentials are defaulted.
    #[cfg(feature = "vfs-d1")]
    fn build_d1_delta(
    ) -> crate::shared::vfs::error::VfsResult<crate::native::vfs::d1_delta::D1Delta> {
        use crate::native::vfs::d1_delta::D1Delta;
        let Some(base) = std::env::var("LOCAL_CF_API_BASE").ok() else {
            return D1Delta::from_env();
        };
        let token = std::env::var("CF_API_TOKEN").unwrap_or_else(|_| "test-token".to_string());
        let account = std::env::var("CF_ACCOUNT_ID").unwrap_or_else(|_| "test-account".to_string());
        let db = std::env::var("LOCAL_D1_DATABASE_ID")
            .or_else(|_| std::env::var("CF_D1_DATABASE_ID"))
            .unwrap_or_else(|_| "test-db-preview".to_string());
        D1Delta::new_with_base_url(&token, &account, &db, Some(&base))
    }

    /// An R2 delta for the shim. Reads the Cloudflare credential variables.
    /// When `LOCAL_CF_API_BASE` is set (local emulator mode), test credentials
    /// are defaulted.
    #[cfg(feature = "vfs-r2")]
    fn build_r2_delta(
    ) -> crate::shared::vfs::error::VfsResult<crate::native::vfs::r2_delta::R2Delta> {
        use crate::native::vfs::r2_delta::R2Delta;
        let Some(base) = std::env::var("LOCAL_CF_API_BASE").ok() else {
            return R2Delta::from_env();
        };
        let token = std::env::var("CF_API_TOKEN").unwrap_or_else(|_| "test-token".to_string());
        let account = std::env::var("CF_ACCOUNT_ID").unwrap_or_else(|_| "test-account".to_string());
        let bucket =
            std::env::var("CF_R2_BUCKET").unwrap_or_else(|_| "test-bucket-preview".to_string());
        R2Delta::new_with_base_url(&token, &account, &bucket, Some(&base))
    }

    static FD_TABLE: std::sync::LazyLock<VirtualFdTable> =
        std::sync::LazyLock::new(VirtualFdTable::new);

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
                return if rest.is_empty() {
                    "/".to_string()
                } else {
                    rest.to_string()
                };
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
    //
    // The exported entry points below call these handlers, which decide
    // whether the path is virtual or must reach the real libc call.

    fn open_common(
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
        real: unsafe fn(*const c_char, c_int, libc::mode_t) -> c_int,
    ) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_open(path, flags, mode);
            }
        }
        unsafe { real(pathname, flags, mode) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn foundation_vfs_open(
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        open_common(pathname, flags, mode, super::real::open)
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn foundation_vfs_open64(
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        open_common(pathname, flags, mode, super::real::open64)
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn foundation_vfs_openat(
        dirfd: c_int,
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if (dirfd == libc::AT_FDCWD || path.starts_with('/')) && path_matches_prefix(path) {
                return handle_virtual_open(path, flags, mode);
            }
        }
        unsafe { super::real::openat(dirfd, pathname, flags, mode) }
    }

    // ── Exported libc entry points for the dynamic loader ──
    //
    // WHY these four entry points are defined in Rust and not in C: rustc links
    // a cdylib against a generated version script. That script exports only the
    // symbols the crate itself defines, and it ends with `local: *;`. A symbol
    // that comes from a linked C object is therefore localized and dropped from
    // the dynamic symbol table. `LD_PRELOAD` never saw a C `open`, so every
    // virtual open fell through to real files and failed with `ENOENT`.
    //
    // libc declares `open` and `openat` as variadic. Stable Rust cannot define a
    // variadic `extern "C"` function, so these entry points take the mode as a
    // fixed third argument. On x86-64, x86, AArch64 and RISC-V the caller writes
    // that value where a fixed third argument lives, so the value read is the
    // real one. When `flags` has neither `O_CREAT` nor `O_TMPFILE` the caller
    // writes no mode and the value is unused. 32-bit ARM passes variadic
    // arguments on the stack, so the shim does not support `armv7-linux`; see
    // `docs/bug-backlog.md`.

    /// `open(2)`: serve a virtual path from the VFS, else call real libc.
    ///
    /// # Safety
    /// `pathname` must be a valid NUL-terminated C string, or null.
    #[allow(invalid_runtime_symbol_definitions)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn open(
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        unsafe { foundation_vfs_open(pathname, flags, mode) }
    }

    /// `open64(2)`: serve a virtual path from the VFS, else call real libc.
    ///
    /// # Safety
    /// `pathname` must be a valid NUL-terminated C string, or null.
    #[allow(invalid_runtime_symbol_definitions)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn open64(
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        unsafe { foundation_vfs_open64(pathname, flags, mode) }
    }

    /// `openat(2)`: serve a virtual path from the VFS, else call real libc.
    ///
    /// # Safety
    /// `pathname` must be a valid NUL-terminated C string, or null.
    #[allow(invalid_runtime_symbol_definitions)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn openat(
        dirfd: c_int,
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        unsafe { foundation_vfs_openat(dirfd, pathname, flags, mode) }
    }

    /// `__openat64_time64`: glibc's 64-bit `time_t` alias of `openat(2)`.
    ///
    /// # Safety
    /// `pathname` must be a valid NUL-terminated C string, or null.
    #[allow(invalid_runtime_symbol_definitions)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __openat64_time64(
        dirfd: c_int,
        pathname: *const c_char,
        flags: c_int,
        mode: libc::mode_t,
    ) -> c_int {
        unsafe { foundation_vfs_openat(dirfd, pathname, flags, mode) }
    }

    // ── close ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn close(fd: c_int) -> c_int {
        if is_virtual_fd(fd) {
            FD_TABLE.remove(fd);
            return 0;
        }
        unsafe { super::real::close(fd) }
    }

    // ── read / write ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn read(
        fd: c_int,
        buf: *mut c_void,
        count: libc::size_t,
    ) -> libc::ssize_t {
        if is_virtual_fd(fd) {
            return handle_virtual_read(fd, buf, count);
        }
        unsafe { super::real::read(fd, buf, count) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn write(
        fd: c_int,
        buf: *const c_void,
        count: libc::size_t,
    ) -> libc::ssize_t {
        if is_virtual_fd(fd) {
            return handle_virtual_write(fd, buf, count);
        }
        unsafe { super::real::write(fd, buf, count) }
    }

    // ── stat ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn stat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_stat(path, statbuf);
            }
        }
        unsafe { super::real::stat(pathname, statbuf) }
    }

    /// `lstat` must *not* follow symlinks. Delegating to our `stat` sent
    /// non-virtual paths through `real::stat`, which does follow them.
    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn lstat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_stat(path, statbuf);
            }
        }
        unsafe { super::real::lstat(pathname, statbuf) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
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

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
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

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn lseek(fd: c_int, offset: libc::off_t, whence: c_int) -> libc::off_t {
        if is_virtual_fd(fd) {
            return handle_virtual_lseek(fd, offset, whence);
        }
        unsafe { super::real::lseek(fd, offset, whence) }
    }

    // ── mkdir / unlink / rename / rmdir ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn mkdir(pathname: *const c_char, mode: libc::mode_t) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_mkdir(path, mode);
            }
        }
        unsafe { super::real::mkdir(pathname, mode) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unlink(pathname: *const c_char) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_unlink(path);
            }
        }
        unsafe { super::real::unlink(pathname) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
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

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rmdir(pathname: *const c_char) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_rmdir(path);
            }
        }
        unsafe { super::real::rmdir(pathname) }
    }

    // ── pread / pwrite ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn pread(
        fd: c_int,
        buf: *mut c_void,
        count: libc::size_t,
        offset: libc::off_t,
    ) -> libc::ssize_t {
        if is_virtual_fd(fd) {
            let buf_slice = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), count) };
            return if let Some(n) = FD_TABLE.read_at(fd, buf_slice, offset.cast_unsigned()) {
                n.cast_signed()
            } else {
                set_errno(libc::EBADF);
                -1
            };
        }
        unsafe { super::real::pread(fd, buf, count, offset) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn pwrite(
        fd: c_int,
        buf: *const c_void,
        count: libc::size_t,
        offset: libc::off_t,
    ) -> libc::ssize_t {
        if is_virtual_fd(fd) {
            let data = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), count) };
            return if let Some(n) = FD_TABLE.write_at(fd, data, offset.cast_unsigned()) {
                n.cast_signed()
            } else {
                set_errno(libc::EBADF);
                -1
            };
        }
        unsafe { super::real::pwrite(fd, buf, count, offset) }
    }

    // ── readlink / symlink ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn readlink(
        pathname: *const c_char,
        buf: *mut c_char,
        bufsiz: libc::size_t,
    ) -> libc::ssize_t {
        // Virtual symlinks not implemented yet
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                set_errno(libc::EINVAL); // not a symlink
                return -1;
            }
        }
        unsafe { super::real::readlink(pathname, buf, bufsiz) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn symlink(target: *const c_char, linkpath: *const c_char) -> c_int {
        // Virtual symlinks not implemented yet
        if let Some(path) = unsafe { c_path_to_str(linkpath) } {
            if path_matches_prefix(path) {
                set_errno(libc::ENOSYS);
                return -1;
            }
        }
        unsafe { super::real::symlink(target, linkpath) }
    }

    // ── chmod / fchmod ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn chmod(pathname: *const c_char, mode: libc::mode_t) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                // chmod on virtual files is a no-op for now
                return 0;
            }
        }
        unsafe { super::real::chmod(pathname, mode) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn fchmod(fd: c_int, mode: libc::mode_t) -> c_int {
        if is_virtual_fd(fd) {
            // chmod on virtual files is a no-op for now
            return 0;
        }
        unsafe { super::real::fchmod(fd, mode) }
    }

    // ── truncate / ftruncate ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn truncate(pathname: *const c_char, length: libc::off_t) -> c_int {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                let key = strip_prefix(path);
                // Read existing content, truncate/pad to length
                let mut data = VFS.fs.read_file(&key).unwrap_or_default();
                data.resize(
                    usize::try_from(length.cast_unsigned()).unwrap_or(usize::MAX),
                    0u8,
                );
                let _ = VFS.fs.write_file(&key, &data);
                return 0;
            }
        }
        unsafe { super::real::truncate(pathname, length) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn ftruncate(fd: c_int, length: libc::off_t) -> c_int {
        if is_virtual_fd(fd) {
            if let Some(path) = FD_TABLE.path(fd) {
                let key = strip_prefix(&path);
                let mut data = VFS.fs.read_file(&key).unwrap_or_default();
                data.resize(
                    usize::try_from(length.cast_unsigned()).unwrap_or(usize::MAX),
                    0u8,
                );
                let _ = VFS.fs.write_file(&key, &data);
                return 0;
            }
            set_errno(libc::EBADF);
            return -1;
        }
        unsafe { super::real::ftruncate(fd, length) }
    }

    // ── fsync ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn fsync(fd: c_int) -> c_int {
        if is_virtual_fd(fd) {
            // In-memory / SQLite delta — data is already persisted
            return 0;
        }
        unsafe { super::real::fsync(fd) }
    }

    // ── opendir / readdir / closedir ──

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn opendir(pathname: *const c_char) -> *mut libc::DIR {
        if let Some(path) = unsafe { c_path_to_str(pathname) } {
            if path_matches_prefix(path) {
                return handle_virtual_opendir(path);
            }
        }
        unsafe { super::real::opendir(pathname) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn readdir(dirp: *mut libc::DIR) -> *mut libc::dirent {
        unsafe { super::real::readdir(dirp) }
    }

    ///
    /// # Safety
    /// The caller must pass valid pointers and open file descriptors that
    /// this C ABI entry point reads or writes.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn closedir(dirp: *mut libc::DIR) -> c_int {
        unsafe { super::real::closedir(dirp) }
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
        if let Ok(file) = VFS.fs.open_file(&key, mode) {
            let vfd = FD_TABLE.alloc_fd();
            FD_TABLE.insert(vfd, key, file, flags);
            vfd
        } else {
            set_errno(libc::EIO);
            -1
        }
    }

    fn handle_virtual_read(fd: c_int, buf: *mut c_void, count: libc::size_t) -> libc::ssize_t {
        let buf_slice = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), count) };
        if let Some(n) = FD_TABLE.read(fd, buf_slice) {
            n.cast_signed()
        } else {
            set_errno(libc::EBADF);
            -1
        }
    }

    fn handle_virtual_write(fd: c_int, buf: *const c_void, count: libc::size_t) -> libc::ssize_t {
        let data = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), count) };
        if let Some(n) = FD_TABLE.write(fd, data) {
            n.cast_signed()
        } else {
            set_errno(libc::EBADF);
            -1
        }
    }

    fn handle_virtual_stat(path: &str, statbuf: *mut libc::stat) -> c_int {
        let key = strip_prefix(path);
        if let Ok(meta) = VFS.fs.stat(&key) {
            unsafe {
                std::ptr::write_bytes(statbuf, 0, 1);
                (*statbuf).st_size = libc::off_t::try_from(meta.size).unwrap_or(libc::off_t::MAX);
                (*statbuf).st_mode = meta.permissions as libc::mode_t;
                (*statbuf).st_uid = meta.owner.0;
                (*statbuf).st_gid = meta.owner.1;
                if let Some(t) = meta.modified {
                    (*statbuf).st_mtime = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as libc::time_t);
                }
                if let Some(t) = meta.created {
                    (*statbuf).st_ctime = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as libc::time_t);
                }
                if let Some(t) = meta.accessed {
                    (*statbuf).st_atime = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as libc::time_t);
                }
            }
            0
        } else {
            set_errno(libc::ENOENT);
            -1
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
        let cur =
            libc::off_t::try_from(FD_TABLE.get_offset(fd).unwrap_or(0)).unwrap_or(libc::off_t::MAX);
        let file_len = FD_TABLE
            .path(fd)
            .and_then(|p| VFS.fs.stat(&p).ok())
            .map_or(0, |m| {
                libc::off_t::try_from(m.size).unwrap_or(libc::off_t::MAX)
            });

        let new_offset = match whence {
            libc::SEEK_SET => offset,
            libc::SEEK_CUR => cur + offset,
            libc::SEEK_END => file_len + offset,
            _ => {
                set_errno(libc::EINVAL);
                return -1;
            }
        };

        if new_offset < 0 {
            set_errno(libc::EINVAL);
            return -1;
        }
        FD_TABLE.set_offset(fd, new_offset.cast_unsigned());
        new_offset
    }

    fn handle_virtual_mkdir(path: &str, _mode: libc::mode_t) -> c_int {
        let key = strip_prefix(path);
        if let Ok(()) = VFS.fs.mkdir(&key) {
            0
        } else {
            set_errno(libc::EIO);
            -1
        }
    }

    fn handle_virtual_unlink(path: &str) -> c_int {
        let key = strip_prefix(path);
        if let Ok(()) = VFS.fs.remove(&key) {
            0
        } else {
            set_errno(libc::ENOENT);
            -1
        }
    }

    fn handle_virtual_rename(old: &str, new: &str) -> c_int {
        let old_key = strip_prefix(old);
        let new_key = strip_prefix(new);
        if let Ok(()) = VFS.fs.rename(&old_key, &new_key) {
            0
        } else {
            set_errno(libc::ENOENT);
            -1
        }
    }

    fn handle_virtual_rmdir(path: &str) -> c_int {
        let key = strip_prefix(path);
        if let Ok(()) = VFS.fs.remove(&key) {
            0
        } else {
            set_errno(libc::ENOENT);
            -1
        }
    }

    fn handle_virtual_opendir(path: &str) -> *mut libc::DIR {
        let key = strip_prefix(path);
        if let Ok(dir) = VFS.fs.open_dir(&key) {
            // Store the directory handle in a global table keyed by a virtual fd
            // For now, return null — full opendir support needs a DIR* wrapper
            // that bridges VfsDirectory → libc::dirent
            drop(dir);
            set_errno(libc::ENOSYS);
            std::ptr::null_mut()
        } else {
            set_errno(libc::ENOENT);
            std::ptr::null_mut()
        }
    }
}
