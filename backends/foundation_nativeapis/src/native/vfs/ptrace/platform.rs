//! Platform abstraction for the ptrace interceptor.
//!
//! The errno translation table and the `openat2` containment probe were dead
//! code: no dispatch path called them. The dispatch maps VFS errors to Linux
//! errno values in `syscall_dispatch::vfs_error_to_errno`, which uses the
//! `libc` errno constants directly.
