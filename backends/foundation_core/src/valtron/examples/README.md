# Valtron examples

Readable, in-module example code showing the state-task surface — a transition
**function** (`fn` or `async fn`) turned into a `TaskIterator` by `#[task]`,
instead of a hand-written `impl TaskIterator`.

| File | What it shows |
|------|---------------|
| [`state_task_sync.rs`](./state_task_sync.rs) | `#[task] fn(ctx, state) -> Option<(T, TaskStatus<R, P>)>` — a multi-step synchronous machine driven to `Ready`. |
| [`state_task_async.rs`](./state_task_async.rs) | `#[task] async fn(ctx, state) -> Option<(T, TaskStatus<R, P>)>` — the same machine with `.await` inside the transition. |

These files are *reference copies* meant to be read. The compiled, runnable
versions (which `cargo run`) live in `foundation_core/examples/`:

```sh
cargo run -p foundation_core --example valtron_state_task
cargo run -p foundation_core --example valtron_state_task_async
```

See [`../README.md`](../README.md) for the full walkthrough.
