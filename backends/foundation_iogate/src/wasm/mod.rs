//! Wasm placeholder.
//!
//! WHY: iogate's reason to exist — the io_uring completion read path — is a
//! native, Linux-first concern. There is no reactor and no `Connection` on wasm.
//!
//! WHAT: nothing. The module exists only so the `shared`/`native`/`wasm` layout
//! matches `foundation_http` and `foundation_netio`, and so `ServerIo` (from
//! `crate::shared`) remains nameable on wasm even though it is inert there.
//!
//! HOW: empty. The wasm build compiles `shared/` only.
