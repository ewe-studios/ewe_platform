---
feature_name: "Ptrace Fork Tracing — Fix PTRACE_EVENT_STOP Deadlock"
description: "Resolve the fork/vfork tracing deadlock caused by PTRACE_EVENT_STOP (128) handling. Three concrete approaches: PTRACE_LISTEN on group-stop, PTRACE_EVENT_VFORK_DONE deferral, and correct wait status decoding."
status: "pending"
priority: "low"
phase: 5
created: 2026-06-09
updated: 2026-06-09
dependencies:
  - "09-ptrace-interceptor"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

# Feature 27: Ptrace Fork Tracing Fix

## Overview

The nix_backend's fork tracing hangs on kernel 7.0.9 because `PTRACE_EVENT_STOP` (128) is mishandled. The previous implementation detached the tracee, which avoided the hang but broke fork tracing entirely. This feature implements the correct fix.

## What's Actually Happening

`PTRACE_EVENT_STOP` (value 128) is distinct from the numbered event stops — it's the stop delivered when a group-stop occurs, and with `PTRACE_SEIZE` it replaces `SIGSTOP` as the initial stop signal. The deadlock unfolds like this:

1. Parent calls `vfork()` and is suspended waiting for the child to `exec()` or `exit()`
2. The tracer receives a `PTRACE_EVENT_STOP` (128) on the child
3. The tracer blocks trying to handle that stop — but the parent is still waiting on the vfork, which can't complete until the child continues
4. Classic circular wait: parent ↔ child ↔ tracer

The previous code's `_ => { detach(pid) }` fallback was wrong — it just gave up.

## Approaches to Fix

### Approach 1: `PTRACE_EVENT_VFORK_DONE` (recommended — first to try)

Enable `PTRACE_O_TRACEVFORKDONE` and defer any parent-side action until `PTRACE_EVENT_VFORK_DONE`. This signals the child has completed exec/exit and the parent is unblocked — then it's safe to continue.

```rust
// In PTRACE_OPTIONS:
PTRACE_O_TRACEVFORKDONE
```

When `PTRACE_EVENT_STOP` arrives on the child during a vfork, immediately `PTRACE_CONT` it without blocking — then wait for `PTRACE_EVENT_VFORK_DONE` on the parent before proceeding.

### Approach 2: `PTRACE_LISTEN` on group-stop

Since Linux 3.4, `PTRACE_LISTEN` restarts a tracee without executing it, leaving it waiting for a new event (like `SIGCONT`), which avoids the deadlock caused by issuing a premature `PTRACE_CONT`.

When detecting a group-stop via `PTRACE_EVENT_STOP`, use `PTRACE_LISTEN` rather than `PTRACE_CONT`. This prevents the tracer from accidentally consuming the stop in a way that breaks the vfork wait.

### Approach 3: Discriminate `PTRACE_EVENT_STOP` from signal-stops correctly

A common source of this deadlock is misidentifying `PTRACE_EVENT_STOP` (128) in the wait status. The correct check extracts the event from bits 16-23, not bits 8-15:

```c
if (WIFSTOPPED(status)) {
    int event = (status >> 16) & 0xff;  // NOT status >> 8
    int sig   = WSTOPSIG(status);
    if (event == PTRACE_EVENT_STOP) {
        // group-stop or initial SEIZE stop — do NOT PTRACE_CONT
        // use PTRACE_LISTEN here
    }
}
```

Misreading the status word and issuing `PTRACE_CONT` on a `PTRACE_EVENT_STOP` during vfork is the most common trigger of this exact hang.

### Approach 4: Kernel-level fix (if we control the kernel)

A recent (Nov 2025) RFC patch from Oleg Nesterov addresses this by checking `signal->group_exec_task` inside `ptrace_stop()` and returning early if a `de_thread()` is waiting — preventing the tracee from entering `TASK_TRACED` at the wrong moment.

An earlier approach introduced `signal->unsafe_execve_in_progress` to detect when sibling threads are traced during `execve`, allowing `ptrace_attach` to proceed while `de_thread()` waits, and aborting/restarting `execve` with `-ERESTARTSYS` if the ptrace status changed at a critical point.

Not applicable on mainline Arch Linux — but relevant if we ever run on a custom kernel.

## Practical Decision Tree

| Situation | Best approach |
|---|---|
| Writing the tracer (us) | Use `PTRACE_LISTEN` on `EVENT_STOP`, wait for `VFORK_DONE` |
| Can't touch tracer or kernel | Set `PTRACE_O_TRACEVFORKDONE` and fast-pass `EVENT_STOP` signals |
| Custom/embedded kernel | Backport the Nesterov RFC patch (Nov 2025) |

## Tasks

- [ ] Add `PTRACE_O_TRACEVFORKDONE` to `PTRACE_OPTIONS`
- [ ] Handle `PTRACE_EVENT_VFORK_DONE` in the ptrace loop
- [ ] Replace `detach(pid)` in `PTRACE_EVENT_STOP` handler with `PTRACE_LISTEN(pid)`
- [ ] Re-enable `test_spawn_pipe_chain` and `test_spawn_can_write_to_real_fs` — verify tests pass

## Test Status

| Test | Status | Reason |
|------|--------|--------|
| `test_spawn_true_exits_zero` | ✅ passes | No fork |
| `test_spawn_echo_with_args` | ✅ passes | No fork |
| `test_spawn_false_exits_nonzero` | ✅ passes | No fork |
| `test_spawn_nonexistent_fails` | ✅ passes | No fork |
| `test_spawn_multiple_children` | ✅ passes | Independent children, no fork |
| `test_spawn_pipe_chain` | ❌ ignored | vfork deadlock (this feature) |
| `test_spawn_can_write_to_real_fs` | ❌ ignored | vfork deadlock (this feature) |

## References

- `PTRACE_EVENT_STOP` defined as 128 in `/usr/include/linux/ptrace.h`
- nix_backend: `backends/foundation_nativeapis/src/native/vfs/ptrace/nix_backend.rs`
- Tests: `backends/foundation_nativeapis/tests/ptrace_vfs/mod.rs`

---

_Created: 2026-06-09 | Updated: 2026-06-09_
