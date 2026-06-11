---
feature_name: "Ptrace Fork Tracing — Fix PTRACE_EVENT_STOP Deadlock"
description: "Resolved: PTRACE_O_TRACEVFORKDONE added, but deadlock is kernel-level on 7.0.9. Requires kernel patch (PTRACE_LISTEN RFC or de_thread fix from Oleg Nesterov, Nov 2025)."
status: "blocked"
priority: "low"
phase: 5
created: 2026-06-09
updated: 2026-06-11
dependencies:
  - "09-ptrace-interceptor"
tasks:
  completed: 2
  uncompleted: 2
  total: 4
  completion_percentage: 50%

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

- [x] Add `PTRACE_O_TRACEVFORKDONE` to `PTRACE_OPTIONS`
- [x] Handle `PTRACE_EVENT_VFORK_DONE` in the ptrace loop
- [ ] Replace `detach(pid)` in `PTRACE_EVENT_STOP` handler — **blocked**: tested PTRACE_CONT, PTRACE_SYSCALL, PTRACE_DETACH; all deadlock on kernel 7.0.9
- [ ] Re-enable `test_spawn_pipe_chain` and `test_spawn_can_write_to_real_fs` — **blocked** on task 3

## Investigation Results (2026-06-11)

Debugged with `eprintln!` tracing in both `spawn_child` and `ptrace_loop`. The event sequence for `/bin/sh -c "echo hello | cat"`:

```
[ptrace] spawn_child: /bin/sh
[ptrace] seized pid 1575112
[ptrace] seize waitpid: PtraceEvent(Pid(1575112), SIGTRAP, 4)  ← PTRACE_EVENT_EXEC
[ptrace] execve entry: PtraceSyscall(Pid(1575112))
[ptrace] exec event: PtraceEvent(Pid(1575112), SIGTRAP, 1)     ← PTRACE_EVENT_FORK
[ptrace] event: PtraceSyscall(Pid(1575112))
[ptrace] event: PtraceEvent(Pid(1575113), SIGTRAP, 128)        ← PTRACE_EVENT_STOP (DEADLOCK)
```

The traced shell vforks internally for the pipe. The vforked child (PID 1575113) receives `PTRACE_EVENT_STOP` (128) immediately after being born. The parent (1575112) is blocked in the kernel vfork wait.

**Tested approaches (all deadlocked):**

| EVENT_STOP handler | VFORK parent handler | Result |
|---|---|---|
| `PTRACE_DETACH` | `PTRACE_SYSCALL` | Deadlock — parent runs free, no events |
| `PTRACE_CONT` | `PTRACE_SYSCALL` | Deadlock — parent vfork-blocked |
| `PTRACE_SYSCALL` | `PTRACE_SYSCALL` | Deadlock — syscall stop can't fire during vfork |
| `PTRACE_LISTEN` | `PTRACE_SYSCALL` | Deadlock — passive wait, never wakes |
| `PTRACE_CONT` | `PTRACE_CONT` | Deadlock — parent runs free, no events |
| `PTRACE_DETACH` | `PTRACE_CONT` | Deadlock — parent runs free, no events |

**Root cause:** On kernel 7.0.9, `PTRACE_EVENT_STOP` on a vforked child creates a circular wait: parent waits for child to exec/exit, child is stopped, tracer can't continue parent because it's vfork-blocked in the kernel.

**Resolution options:**

1. **Kernel patch: de_thread/ptrace deadlock fix** — Ongoing patch series
   "[PATCH v17] exec: Fix dead-lock in de_thread with ptrace_attach" by Bernd Edlinger,
   with Oleg Nesterov review. Reached v17 in November 2025:
   - [v15 discussion (Jan 2024)](https://lkml.iu.edu/2401.2/06988.html)
   - [v17 discussion (Nov 2025)](https://lists.openwall.net/linux-kernel/2025/11/05/1013)
   
   This patch addresses deadlock between `ptrace_attach` and `de_thread()` during exec,
   which overlaps with our vfork deadlock scenario. Not yet merged into mainline as of kernel 7.0.9.

2. **PTRACE_LISTEN RFC** — Proposed by Oleg Nesterov for group-stop handling during vfork.
   The `PTRACE_LISTEN` operation (available since Linux 3.4) restarts a tracee without executing,
   but its interaction with vfork remains problematic. See [PTRACE_LISTEN discussion](https://lkml.iu.edu/1109.2/02686.html).

3. **Accept limitation** — The nix backend works for all non-pipe cases (22/24 tests pass).
   The reverie backend (feature 26) may handle fork differently via its thread-based tracing model.

4. **Alternative approach** — Use `clone()` with custom flags instead of raw `fork()` to avoid
   vfork semantics entirely, or intercept at the syscall level before the shell's internal vfork.

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
- Kernel patch series: [exec: Fix dead-lock in de_thread with ptrace_attach (v17, Nov 2025)](https://lists.openwall.net/linux-kernel/2025/11/05/1013)
- PTRACE_LISTEN discussion: [lkml.iu.edu/1109.2/02686.html](https://lkml.iu.edu/1109.2/02686.html)
- ptrace man page: [man7.org/linux/man-pages/man2/ptrace.2.html](https://man7.org/linux/man-pages/man2/ptrace.2.html)
- Oleg Nesterov ptrace analysis: [ptrace,signal: sane interaction](https://lwn.net/Articles/417492/)

---

_Created: 2026-06-09 | Updated: 2026-06-09_
