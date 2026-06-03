# Learnings

## Restructure (spec-34/08)

1. **`FdMonitorTask` should use `TaskStatus::Depends`, not `Delayed`** — `RegisteredFd` implements `EventReadiness` via the poll selector, so the executor can park the task and only wake it when the OS signals fd readiness (epoll/kqueue). Using `Delayed` wastes CPU by waking on a fixed timer even when nothing happened. The `poll_interval` field is unnecessary once `Depends` is used.

2. **`EventReadiness` requires `Send + Sync + 'static` for trait object casting** — `RegisteredFd<T: AsRawFd>` only implements `EventReadiness` when `T: Send + Sync`. To cast `Arc<RegisteredFd<T>>` to `Arc<dyn EventReadiness>`, the impl block also needs `T: 'static` (required for `dyn EventReadiness` inside an `Arc`).

3. **Cargo can't do target-gated features in `default`** — you can use `#[cfg(...)]` for dependencies, but `default = [...]` doesn't support per-target feature lists. The standard pattern is: `default = ["task", "native"]` and users opt into the platform-specific watcher via `native-linux`, `native-macos`, or `native-windows`.

4. **Sed-based import migration is fragile** — replacing `crate::poll::` with `crate::native::poll::` also affected internal poll module imports (`poll/event/` files). Better to use `grep -rn 'crate::'` to audit first, then apply targeted fixes.

5. **`valtron/` should split shared vs native** — only `FdMonitorTask` depends on `native::fd` and `native::poll`. The rest (`broadcaster`, `file_watcher`, `stop_signal`) are purely shared types using only `foundation_core::valtron` and `shared/`. Moving `fd_monitor.rs` to `valtron/native/` behind `#[cfg(feature = "fd")]` means future consumers of the shared valtron types don't need to pull in the fd layer.

6. **`FdMonitorTask` with Depends: `Arc` + `'static`** — to hand an `Arc<RegisteredFd<T>>` to `TaskStatus::Depends` as `Arc<dyn EventReadiness>`, need `T: AsRawFd + Send + Sync + 'static`. The `'static` is required for trait objects inside `Arc` — the executor may hold the `Arc` indefinitely. The `fd` field must be `Arc<RegisteredFd<T>>` (not owned) so we can `Arc::clone` it for the `Depends` while still owning the original for `poll_readable()`.
