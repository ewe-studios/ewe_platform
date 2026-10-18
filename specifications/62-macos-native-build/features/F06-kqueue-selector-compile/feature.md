---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F06-kqueue-selector-compile"
this_file: "specifications/62-macos-native-build/features/F06-kqueue-selector-compile/feature.md"

feature: "Kqueue selector compile fixes"
description: "Fix the 16 foundation_nativeapis kqueue compile errors so the macOS/BSD poll backend builds"
status: "complete"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-10-17
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# F06 — Kqueue selector compile fixes

[spec](../../requirements.md)

## Overview

`foundation_nativeapis` fails to compile on macOS with 16 errors, all in the
hand-written kqueue backend. The backend was written on Linux (epoll) and never
compiled on a BSD target. The errors are four distinct, fully-diagnosed problems
— no open questions remain.

## Error inventory (16)

| # | Error | Location | Cause |
|---|-------|----------|-------|
| 1 | E0432 unresolved import `...::event` | `poll/sys/unix/selector/kqueue.rs:3` | `super::super::super::event::Event` resolves to `sys::event`, which doesn't exist |
| 2 | E0432 unresolved import `...::Events` | `poll/sys/unix/selector/kqueue.rs:4` | `super::super::super::Events` resolves to `sys::Events`; `Events` lives at `crate::native::poll::Events` |
| 3–11 | E0425 `EV_SET` not in `libc` | `kqueue.rs:58,95,137,152,199,212,250,287,373` | `EV_SET` is a C macro in `<sys/event.h>`, not a libc function |
| 12–15 | E0283 type annotations needed | `poll/event/kqueue.rs:24,29,42,49` | `libc::EVFILT_READ/WRITE as _` is ambiguous (serde_json impls `PartialEq<Value> for i16`) |
| 16 | E0599 no `Selector::new` | `poll/mod.rs:125` | kqueue `Selector` only defines `new_with_registry()`; `Poll::new()` calls `new()` |

## Reference types (libc 0.2.189, verified)

`libc::kevent` fields: `ident: usize` (`uintptr_t`), `filter: i16`,
`flags: u16`, `fflags: u32`, `data: isize` (`intptr_t`), `udata: *mut c_void`.

Constants: `EVFILT_READ/WRITE/VNODE/USER: i16`; `EV_ADD/DELETE/CLEAR: u16`;
`NOTE_WRITE/DELETE/EXTEND/RENAME/REVOKE/TRIGGER: u32`.

## Plan

### Step 1 — fix imports (`kqueue.rs`)

Replace the two broken imports. `Event` is unused in this file, so drop it:

```rust
// before
use super::super::super::event::Event;
use super::super::super::Events;
use crate::native::poll::{Interest, Registry, Token};

// after
use crate::native::poll::{Events, Interest, Registry, Token};
```

### Step 2 — replace `libc::EV_SET` with an in-place helper (`kqueue.rs`)

`EV_SET` is a macro with no libc binding. Add a safe helper and call it instead:

```rust
/// `EV_SET` is a C preprocessor macro in `<sys/event.h>`, so `libc` has no
/// binding for it. Initialise the `kevent` in place instead.
#[inline]
fn ev_set(
    ev: &mut libc::kevent,
    ident: libc::uintptr_t,
    filter: i16,
    flags: u16,
    fflags: u32,
    data: libc::intptr_t,
    udata: *mut libc::c_void,
) {
    ev.ident = ident;
    ev.filter = filter;
    ev.flags = flags;
    ev.fflags = fflags;
    ev.data = data;
    ev.udata = udata;
}
```

The nine call sites pass arguments in exactly `EV_SET`'s order
(`kevp, ident, filter, flags, fflags, data, udata`), so a mechanical
`replace_all` of `libc::EV_SET(` → `ev_set(` is correct — the argument lists do
not change. Remove the now-redundant `unsafe { }` blocks that only wrapped the
`EV_SET` call (the helper is safe; leaving them would trigger `unused_unsafe`).

### Step 3 — add `Selector::new()` (`kqueue.rs`)

`Poll::new()` needs `new()`. Add it and make `new_with_registry()` delegate, so
the kqueue creation logic stays single-source:

```rust
/// Create a new kqueue selector.
pub fn new() -> io::Result<Self> {
    let kq_fd = unsafe {
        let fd = libc::kqueue();
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        OwnedFd::from_raw_fd(fd)
    };

    Ok(Self {
        kq: kq_fd,
        waker_token: Mutex::new(None),
    })
}

/// Create a kqueue selector wrapped in an `Arc`, with its `Registry`.
pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
    let selector = Arc::new(Self::new()?);
    let registry = Registry {
        selector: selector.clone(),
    };
    Ok((selector, registry))
}
```

### Step 4 — disambiguate the `as _` casts (`event/kqueue.rs`)

Change the four `as _` sites to an explicit `as i16`:

```rust
self.event.filter == libc::EVFILT_READ as i16   // lines 24, 42
self.event.filter == libc::EVFILT_WRITE as i16  // lines 29, 49
```

## Acceptance criteria

1. `cargo build -p foundation_nativeapis --profile uat` succeeds on
   `aarch64-apple-darwin` with zero errors.
2. No `libc::EV_SET`, `as _` on an `EVFILT_*`, or `new_with_registry`-only
   selector remains in the kqueue path.
3. `Poll::new()` works unchanged — the `Selector::new()` contract is honoured.

## Out of scope

- The `Events::as_mut_ptr_and_cap` → `*mut libc::kevent` cast is pre-existing and
  compiles; its soundness is tracked separately (see requirements.md).
- Windows/shell selectors have the same `new()` gap — that is F07, not here.

## Resolution

All 16 errors fixed as planned. `foundation_nativeapis` compiles on
`aarch64-apple-darwin` under both `uat` and `dev` (exit 0). The four steps were
applied mechanically:

1. Imports fixed — the unused `Event` import was dropped.
2. `libc::EV_SET(` → `ev_set(` at all nine call sites, backed by a new in-place
   helper. The helper is `unsafe fn` (not the safe `fn` in the plan) because the
   caller must uphold the `ident`/`udata` lifetimes the kernel keeps after the
   call; the now-redundant `unsafe {}` blocks at the call sites were removed.
3. `Selector::new()` added, with `new_with_registry()` delegating to it so the
   kqueue creation stays single-source.
4. The four `as _` casts made explicit `as i16`.
