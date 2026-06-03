# Learnings

## Restructure (spec-34/08)

1. **`FdMonitorTask` should use `TaskStatus::Depends`, not `Delayed`** — `RegisteredFd` implements `EventReadiness` via the poll selector, so the executor can park the task and only wake it when the OS signals fd readiness (epoll/kqueue). Using `Delayed` wastes CPU by waking on a fixed timer even when nothing happened. The `poll_interval` field is unnecessary once `Depends` is used.

2. **`EventReadiness` requires `Send + Sync + 'static` for trait object casting** — `RegisteredFd<T: AsRawFd>` only implements `EventReadiness` when `T: Send + Sync`. To cast `Arc<RegisteredFd<T>>` to `Arc<dyn EventReadiness>`, the impl block also needs `T: 'static` (required for `dyn EventReadiness` inside an `Arc`).

3. **Cargo can't do target-gated features in `default`** — you can use `#[cfg(...)]` for dependencies, but `default = [...]` doesn't support per-target feature lists. The standard pattern is: `default = ["native"]` and users opt into the platform-specific watcher via `native-linux`, `native-macos`, or `native-windows`.

4. **Sed-based import migration is fragile** — replacing `crate::poll::` with `crate::native::poll::` also affected internal poll module imports (`poll/event/` files). Better to use `grep -rn 'crate::'` to audit first, then apply targeted fixes.

5. **`valtron/` should split shared vs native** — only `FdMonitorTask` depends on `native::fd` and `native::poll`. The rest (`broadcaster`, `file_watcher`, `stop_signal`) are purely shared types using only `foundation_core::valtron` and `shared/`. Moving `fd_monitor.rs` to `valtron/native/` behind `#[cfg(feature = "fd")]` means future consumers of the shared valtron types don't need to pull in the fd layer.

6. **`FdMonitorTask` with Depends: `Arc` + `'static`** — to hand an `Arc<RegisteredFd<T>>` to `TaskStatus::Depends` as `Arc<dyn EventReadiness>`, need `T: AsRawFd + Send + Sync + 'static`. The `'static` is required for trait objects inside `Arc` — the executor may hold the `Arc` indefinitely. The `fd` field must be `Arc<RegisteredFd<T>>` (not owned) so we can `Arc::clone` it for the `Depends` while still owning the original for `poll_readable()`.

7. **`FdState` belongs in `shared/` not `native/fd/`** — it's just an enum (`Readable`, `Writable`, `Both`) with no OS dependencies. Putting it in `native/fd` forced a native import on anything using it. Moving to `shared/fd_state.rs` keeps it universally available. `FdState::from_ready()` lives in the `FdMonitorTask` module since it bridges `native::fd::Ready` → `shared::FdState`.

8. **`ReadyGuard::try_io_read` returns `(R, usize)` for EOF detection** — we can't inspect the generic `R` to check if a read returned 0 bytes. Returning `(R, usize)` lets the guard auto-clear readiness on `bytes_read == 0`, preventing busy loops on closed connections.

9. **`task` feature flag removed** — valtron types (`FileWatcherTask`, `EventBroadcaster`, `StopSignal`, `CompositeReadiness`, `FdState`) are always available. Only `FdMonitorTask` needs `#[cfg(feature = "fd")]` since it depends on `native::fd::RegisteredFd`.

10. **`foundation_core` multi feature gated by target** — `foundation_core = { features = ["multi"] }` is only enabled on Linux/macOS/iOS via `[target.'cfg(...)'.dependencies]`. On unsupported platforms (WASM, etc.), the base crate still compiles without the multi-threaded executor.
