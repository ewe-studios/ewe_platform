//! The foreign function interface (FFI) of libkrun 1.x.
//!
//! The runner loads the library at run time with `libloading`, so no build
//! needs the library, `pkg-config` or `clang` (feature 09, section 3.2). Every
//! `unsafe` block in this module has a `SAFETY` comment.
//!
//! Only this module and `src/bin/sandbox_krun.rs` name a `krun_*` symbol
//! (invariant I1).

use std::ffi::{c_char, CStr, CString, OsString};
use std::path::Path;

use libloading::Library;

use crate::shared::krun_config::{KrunDisk, KrunProbe, KrunRunnerConfig};

/// The tag of the root virtio-fs device (`KRUN_FS_ROOT_TAG`).
pub const KRUN_FS_ROOT_TAG: &CStr = c"/dev/root";
/// The guest path of the slave, inside the boot root.
pub const SLAVE_GUEST_PATH: &CStr = c"/.ewe-slave";
/// The first argument of the slave: the listen option name.
pub const LISTEN_FLAG: &CStr = c"--listen";
/// The Transparent Socket Impersonation (TSI) bit for Internet connections
/// (`KRUN_TSI_HIJACK_INET`). The Unix bit is never set.
pub const KRUN_TSI_HIJACK_INET: u32 = 1;
/// The file mode of the overlay slave file: a regular program that every user
/// may run.
pub const SLAVE_FILE_MODE: u32 = 0o100_755;
/// The libkrun disk format number of a raw image (`KRUN_DISK_FORMAT_RAW`).
pub const KRUN_DISK_FORMAT_RAW: u32 = 0;
/// The libkrun feature number of block device support (`KRUN_FEATURE_BLK`).
pub const KRUN_FEATURE_BLK: u32 = 1;

type SetLogLevel = unsafe extern "C" fn(u32) -> i32;
type CreateCtx = unsafe extern "C" fn() -> i32;
type SetVmConfig = unsafe extern "C" fn(u32, u8, u32) -> i32;
type AddVirtiofs3 = unsafe extern "C" fn(u32, *const c_char, *const c_char, u64, bool) -> i32;
type AddDisk2 = unsafe extern "C" fn(u32, *const c_char, *const c_char, u32, bool) -> i32;
type HasFeature = unsafe extern "C" fn(u32) -> bool;
type FsAddOverlayFile =
    unsafe extern "C" fn(u32, *const c_char, *const c_char, *const u8, usize, u32, bool) -> i32;
type DisableImplicitVsock = unsafe extern "C" fn(u32) -> i32;
type AddVsock = unsafe extern "C" fn(u32, u32) -> i32;
type AddVsockPort2 = unsafe extern "C" fn(u32, u32, *const c_char, bool) -> i32;
type SetPortMap = unsafe extern "C" fn(u32, *const *const c_char) -> i32;
type SetExec =
    unsafe extern "C" fn(u32, *const c_char, *const *const c_char, *const *const c_char) -> i32;
type StartEnter = unsafe extern "C" fn(u32) -> i32;
type GetMaxVcpus = unsafe extern "C" fn() -> u8;

/// The symbols that one run needs.
struct Symbols {
    set_log_level: SetLogLevel,
    create_ctx: CreateCtx,
    set_vm_config: SetVmConfig,
    add_virtiofs3: AddVirtiofs3,
    /// `krun_add_disk2`, resolved only when the configuration has a disk.
    add_disk2: Option<AddDisk2>,
    fs_add_overlay_file: FsAddOverlayFile,
    disable_implicit_vsock: DisableImplicitVsock,
    add_vsock: AddVsock,
    add_vsock_port2: AddVsockPort2,
    set_port_map: SetPortMap,
    set_exec: SetExec,
    start_enter: StartEnter,
}

/// Why the library did not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// No library file loaded from any candidate name.
    NotFound {
        /// The names that the runner tried.
        tried: Vec<String>,
        /// The loader message of the last attempt.
        message: String,
    },
    /// The library loaded, but it has no named symbol.
    MissingSymbol(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { message, .. } => {
                write!(formatter, "libkrun not found: {message}")
            }
            Self::MissingSymbol(name) => {
                write!(formatter, "libkrun has no symbol {name}")
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Why one run stopped before it became the virtual machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// A host input or output call failed.
    Io {
        /// The operation.
        op: &'static str,
        /// The message.
        message: String,
    },
    /// A libkrun call returned a negative error number.
    Call {
        /// The function name.
        function: &'static str,
        /// The error number, as a positive value.
        errno: i32,
    },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { op, message } => write!(formatter, "{op} failed: {message}"),
            Self::Call { function, errno } => {
                write!(formatter, "{function} failed: errno {errno}")
            }
        }
    }
}

impl std::error::Error for RunError {}

/// The loaded libkrun library and its symbols.
pub struct KrunLibrary {
    /// The loaded library. The field keeps it loaded; the symbols point into it.
    library: Library,
    /// The path or name that loaded.
    loaded: String,
    symbols: Symbols,
}

/// The library names of this system, in the order to try.
fn library_names() -> &'static [&'static str] {
    #[cfg(target_os = "linux")]
    {
        &["libkrun.so.1", "libkrun.so"]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "libkrun.1.dylib",
            "libkrun.dylib",
            "/opt/homebrew/lib/libkrun.1.dylib",
        ]
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        &[]
    }
}

fn symbol<T: Copy>(library: &Library, name: &'static str) -> Result<T, LoadError> {
    // SAFETY: the loader returns a symbol of the requested type. The type is
    // the one in `include/libkrun.h` on `stable-1.19.x` (learnings section 3.1).
    let loaded: libloading::Symbol<T> = unsafe { library.get(name.as_bytes()) }
        .map_err(|_| LoadError::MissingSymbol(name.to_string()))?;
    Ok(*loaded)
}

impl KrunLibrary {
    /// Loads the library from `path`, or from the names of this system.
    ///
    /// # Errors
    /// [`LoadError::NotFound`] when no library loads, and
    /// [`LoadError::MissingSymbol`] when a required symbol is absent.
    pub fn load(path: Option<&Path>, with_disks: bool) -> Result<Self, LoadError> {
        let candidates: Vec<OsString> = match path {
            Some(path) => vec![path.as_os_str().to_os_string()],
            None => library_names().iter().map(OsString::from).collect(),
        };
        let mut tried = Vec::new();
        let mut last_message = String::from("no library name for this system");
        for candidate in candidates {
            tried.push(candidate.to_string_lossy().into_owned());
            // SAFETY: the loader opens a dynamic library. The runner does not
            // assume that the file is honest; a symbol of a different
            // signature is a library mismatch, which the version pin prevents.
            let loaded = match unsafe { Library::new(&candidate) } {
                Ok(library) => library,
                Err(error) => {
                    last_message = format!("{}: {error}", candidate.to_string_lossy());
                    continue;
                }
            };
            // The disk symbol is optional: a configuration with no disk needs
            // no block support. A configuration with a disk resolves it now, so
            // a library without `krun_add_disk2` fails before the VM starts
            // with the missing-symbol error and exit code 71.
            let add_disk2 = if with_disks {
                Some(symbol(&loaded, "krun_add_disk2")?)
            } else {
                None
            };
            let symbols = Symbols {
                set_log_level: symbol(&loaded, "krun_set_log_level")?,
                create_ctx: symbol(&loaded, "krun_create_ctx")?,
                set_vm_config: symbol(&loaded, "krun_set_vm_config")?,
                add_virtiofs3: symbol(&loaded, "krun_add_virtiofs3")?,
                add_disk2,
                fs_add_overlay_file: symbol(&loaded, "krun_fs_add_overlay_file")?,
                disable_implicit_vsock: symbol(&loaded, "krun_disable_implicit_vsock")?,
                add_vsock: symbol(&loaded, "krun_add_vsock")?,
                add_vsock_port2: symbol(&loaded, "krun_add_vsock_port2")?,
                set_port_map: symbol(&loaded, "krun_set_port_map")?,
                set_exec: symbol(&loaded, "krun_set_exec")?,
                start_enter: symbol(&loaded, "krun_start_enter")?,
            };
            return Ok(Self {
                library: loaded,
                loaded: candidate.to_string_lossy().into_owned(),
                symbols,
            });
        }
        Err(LoadError::NotFound {
            tried,
            message: last_message,
        })
    }

    /// The path or name that loaded.
    #[must_use]
    pub fn loaded_path(&self) -> &str {
        &self.loaded
    }

    /// The most virtual CPUs that this library gives one machine.
    ///
    /// # Errors
    /// [`LoadError::MissingSymbol`] when the library has no
    /// `krun_get_max_vcpus`.
    pub fn max_vcpus(&self) -> Result<u8, LoadError> {
        let get: GetMaxVcpus = symbol(&self.library, "krun_get_max_vcpus")?;
        // SAFETY: the symbol has the signature in `include/libkrun.h` line
        // 1140. It takes no argument and reads no state of the caller.
        Ok(unsafe { get() })
    }

    /// The one line of `sandbox-krun --probe`.
    ///
    /// # Errors
    /// [`LoadError::MissingSymbol`] when the library has no
    /// `krun_get_max_vcpus`.
    pub fn probe(&self) -> Result<KrunProbe, LoadError> {
        Ok(KrunProbe {
            library: self.loaded.clone(),
            max_vcpus: self.max_vcpus()?,
            blk: self.has_blk(),
        })
    }

    /// Whether the library has block device support.
    ///
    /// A library without `krun_has_feature` is an older library with no block
    /// support that this driver can trust, so the answer is `false`.
    fn has_blk(&self) -> bool {
        let Ok(has_feature) = symbol::<HasFeature>(&self.library, "krun_has_feature") else {
            return false;
        };
        // SAFETY: the symbol takes one feature number and reads no state of the
        // caller.
        unsafe { has_feature(KRUN_FEATURE_BLK) }
    }

    /// Configures one virtual machine and becomes it (feature 09, section 3.3).
    ///
    /// The call `krun_start_enter` does not return on success: the process
    /// becomes the virtual machine and later exits with the code of the guest
    /// program. A `Ok(code)` here means that the call returned without an
    /// error, which the caller treats as the exit code.
    ///
    /// # Errors
    /// [`RunError`] when a host call fails or a libkrun call returns a negative
    /// error number.
    #[allow(
        clippy::too_many_lines,
        reason = "the call sequence of section 3.3 is one ordered list; a split would hide the order"
    )]
    pub fn run(&self, config: &KrunRunnerConfig) -> Result<i32, RunError> {
        let symbols = &self.symbols;
        // SAFETY: each call uses the signature of `include/libkrun.h` on
        // `stable-1.19.x`. Every pointer argument points to a value that lives
        // until `krun_start_enter`, which does not return on success.
        unsafe {
            check(
                "krun_set_log_level",
                (symbols.set_log_level)(config.log_level),
            )?;
            let ctx = check("krun_create_ctx", (symbols.create_ctx)())?;
            let ctx = u32::try_from(ctx).map_err(|_| RunError::Io {
                op: "read the libkrun context",
                message: format!("the context identifier {ctx} is negative"),
            })?;
            check(
                "krun_set_vm_config",
                (symbols.set_vm_config)(ctx, config.vcpus, config.memory_mib),
            )?;

            let boot_root = cstring_path(&config.boot_root)?;
            check(
                "krun_add_virtiofs3",
                (symbols.add_virtiofs3)(
                    ctx,
                    KRUN_FS_ROOT_TAG.as_ptr(),
                    boot_root.as_ptr(),
                    0,
                    true,
                ),
            )?;

            let slave_data = std::fs::read(&config.slave_path).map_err(|error| RunError::Io {
                op: "read the slave program",
                message: format!("{}: {error}", config.slave_path.display()),
            })?;
            let slave_len = slave_data.len();
            // libkrun does not copy the data; it needs it until the guest
            // stops, so the runner leaks the one buffer for its life.
            let slave_data: &'static [u8] = Box::leak(slave_data.into_boxed_slice());
            check(
                "krun_fs_add_overlay_file",
                (symbols.fs_add_overlay_file)(
                    ctx,
                    KRUN_FS_ROOT_TAG.as_ptr(),
                    SLAVE_GUEST_PATH.as_ptr(),
                    slave_data.as_ptr(),
                    slave_len,
                    SLAVE_FILE_MODE,
                    false,
                ),
            )?;

            if let Some(user_root) = &config.user_root {
                add_mount(symbols, ctx, user_root)?;
            }
            for mount in &config.mounts {
                add_mount(symbols, ctx, mount)?;
            }
            if !config.disks.is_empty() {
                let Some(add_disk2) = symbols.add_disk2 else {
                    return Err(RunError::Io {
                        op: "add a disk",
                        message: "the library has no krun_add_disk2".to_string(),
                    });
                };
                for disk in &config.disks {
                    add_disk(ctx, add_disk2, disk)?;
                }
            }

            check(
                "krun_disable_implicit_vsock",
                (symbols.disable_implicit_vsock)(ctx),
            )?;
            let tsi = if config.tsi_inet {
                KRUN_TSI_HIJACK_INET
            } else {
                0
            };
            check("krun_add_vsock", (symbols.add_vsock)(ctx, tsi))?;

            let slave_socket = cstring_path(&config.slave_socket)?;
            check(
                "krun_add_vsock_port2",
                (symbols.add_vsock_port2)(ctx, config.slave_port, slave_socket.as_ptr(), true),
            )?;

            // An empty port map. `NULL` would expose every guest listening
            // port on the host (invariant I6).
            let port_map = [std::ptr::null::<c_char>()];
            check(
                "krun_set_port_map",
                (symbols.set_port_map)(ctx, port_map.as_ptr()),
            )?;

            let exec_path = SLAVE_GUEST_PATH;
            let listen_address =
                CString::new(format!("vsock:{}", config.slave_port)).map_err(|_| RunError::Io {
                    op: "encode the slave listen address",
                    message: "the address has a zero byte".to_string(),
                })?;
            let argv = [
                LISTEN_FLAG.as_ptr(),
                listen_address.as_ptr(),
                std::ptr::null::<c_char>(),
            ];
            // An explicit empty environment. `NULL` would copy the runner
            // environment into the guest (invariant I4).
            let envp = [std::ptr::null::<c_char>()];
            check(
                "krun_set_exec",
                (symbols.set_exec)(ctx, exec_path.as_ptr(), argv.as_ptr(), envp.as_ptr()),
            )?;

            start_lifeline(config.lifeline_fd)?;
            let code = (symbols.start_enter)(ctx);
            if code < 0 {
                return Err(RunError::Call {
                    function: "krun_start_enter",
                    errno: -code,
                });
            }
            Ok(code)
        }
    }
}

/// Adds one virtio-fs share, read-only or writable.
fn add_mount(
    symbols: &Symbols,
    ctx: u32,
    mount: &crate::shared::krun_config::KrunMount,
) -> Result<(), RunError> {
    let tag = CString::new(mount.tag.as_str()).map_err(|_| RunError::Io {
        op: "encode a mount tag",
        message: format!("the tag {} has a zero byte", mount.tag),
    })?;
    let path = cstring_path(&mount.host_path)?;
    // SAFETY: `tag` and `path` live until the end of the call, and the function
    // reads them only during the call. The signature is the one of
    // `include/libkrun.h` line 364.
    check("krun_add_virtiofs3", unsafe {
        (symbols.add_virtiofs3)(ctx, tag.as_ptr(), path.as_ptr(), 0, mount.read_only)
    })
    .map(|_| ())
}

/// Attaches one raw disk image.
fn add_disk(ctx: u32, add: AddDisk2, disk: &KrunDisk) -> Result<(), RunError> {
    let id = CString::new(disk.id.as_str()).map_err(|_| RunError::Io {
        op: "encode a disk identifier",
        message: format!("the identifier {} has a zero byte", disk.id),
    })?;
    let path = cstring_path(&disk.path)?;
    // SAFETY: `id` and `path` live until the end of the call, and the function
    // reads them only during the call. The signature is the one of
    // `include/libkrun.h`.
    check("krun_add_disk2", unsafe {
        add(
            ctx,
            id.as_ptr(),
            path.as_ptr(),
            KRUN_DISK_FORMAT_RAW,
            disk.read_only,
        )
    })
    .map(|_| ())
}

/// Returns `value` when it is not negative, or the call error.
fn check(function: &'static str, value: i32) -> Result<i32, RunError> {
    if value < 0 {
        Err(RunError::Call {
            function,
            errno: -value,
        })
    } else {
        Ok(value)
    }
}

/// Encodes one path for a C call.
fn cstring_path(path: &Path) -> Result<CString, RunError> {
    CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| RunError::Io {
        op: "encode a path",
        message: format!("{}: the path has a zero byte", path.display()),
    })
}

/// Starts the macOS lifeline thread on `fd`.
///
/// The parent holds the write end of a pipe. The thread reads the read end. The
/// read returns 0 when the parent process is gone, and the thread then ends the
/// process with code 137 (invariant I8).
#[cfg(target_os = "macos")]
fn start_lifeline(fd: Option<i32>) -> Result<(), RunError> {
    let Some(fd) = fd else {
        return Ok(());
    };
    std::thread::Builder::new()
        .name("krun-lifeline".to_string())
        .spawn(move || {
            let mut byte = 0_u8;
            loop {
                // SAFETY: `fd` is the read end that the driver passed as
                // descriptor 3. Only this thread reads it, and no thread closes
                // it. The buffer is one valid byte.
                let read = unsafe { libc::read(fd, std::ptr::addr_of_mut!(byte).cast(), 1) };
                if read == 0 {
                    break;
                }
                if read < 0 {
                    // SAFETY: `last_os_error` reads the thread-local error
                    // number and has no side effect.
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }
                    break;
                }
            }
            // SAFETY: `_exit` ends the process at once. The virtual machine
            // must not outlive its parent, and no destructor may run.
            unsafe { libc::_exit(137) }
        })
        .map(|_thread| ())
        .map_err(|error| RunError::Io {
            op: "start the lifeline thread",
            message: error.to_string(),
        })
}

/// A host without the macOS lifeline refuses a configuration that names one.
#[cfg(not(target_os = "macos"))]
fn start_lifeline(fd: Option<i32>) -> Result<(), RunError> {
    if fd.is_some() {
        return Err(RunError::Io {
            op: "start the lifeline",
            message: "a lifeline descriptor needs a macOS host".to_string(),
        });
    }
    Ok(())
}
