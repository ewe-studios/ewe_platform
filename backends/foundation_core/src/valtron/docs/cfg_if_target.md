# Cfg_if for OS Targets

See [source](https://doc.rust-lang.org/src/std/sys/thread_local/mod.rs.html)

```rust
/// The native TLS implementation needs a way to register destructors for its data.
/// This module contains platform-specific implementations of that register.
///
/// It turns out however that most platforms don't have a way to register a
/// destructor for each variable. On these platforms, we keep track of the
/// destructors ourselves and register (through the [`guard`] module) only a
/// single callback that runs all of the destructors in the list.
#[cfg(all(target_thread_local, not(all(target_family = "wasm", not(target_feature = "atomics")))))]
pub(crate) mod destructors {
    cfg_if::cfg_if! {
        if #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "fuchsia",
            target_os = "redox",
            target_os = "hurd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))] {
            mod linux_like;
            mod list;
            pub(super) use linux_like::register;
            pub(super) use list::run;
        } else {
            mod list;
            pub(super) use list::register;
            pub(crate) use list::run;
        }
    }
}

```
