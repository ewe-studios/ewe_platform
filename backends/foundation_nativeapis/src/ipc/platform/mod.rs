/// Platform-specific IPC transports.
///
/// | Platform | Transport | Mechanism |
/// |----------|-----------|-----------|
/// | Linux | Unix domain sockets (`SOCK_SEQPACKET`) | Abstract socket addresses, `SCM_RIGHTS` for FD passing |
/// | macOS | Mach ports | `mach_msg`, `mach_port` operations (stub) |
/// | Windows | Named pipes | `CreateNamedPipe`, `ConnectNamedPipe` (stub) |

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub use linux::{Fd, Local, Remote, EncodedMessage, IoMultiplexing, MemoryRegion, Object};

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub use macos::{Fd, Local, Remote, EncodedMessage, IoMultiplexing, MemoryRegion, Object};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::{Fd, Local, Remote, EncodedMessage, IoMultiplexing, MemoryRegion, Object};

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("IPC is not supported on this platform. Use Linux, macOS, or Windows.");
