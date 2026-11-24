# Foundation tasks

Foundation tasks runs named package operations from a command-line interface (CLI).
Register a typed function, its argument structure, and a short summary in a task set.
The runner parses the arguments before it starts the function.

```rust
use clap::Args;
use foundation_tasks::{task, TaskContext, TaskResult, TaskRunner, TaskSet};

#[derive(Args)]
struct Echo {
    message: String,
}

async fn echo(ctx: TaskContext, args: Echo) -> TaskResult {
    let output = ctx.require::<foundation_tasks::TaskEnv>()?.output.clone();
    output.write(foundation_tasks::OutputStream::Stdout, args.message.as_bytes())
        .await.map_err(foundation_tasks::TaskError::operation)
}

#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    let mut set = TaskSet::new("example").with_namespace("demo");
    set.add("echo", "Print a message", echo)?;
    runner.register(set)?;
    runner.run().await
}
```

Use `task ls` to list registered names, `task help NAME` for help, and `task run NAME` to run one.
A package can expose an existing command path with `TaskSet::expose`.
Each invocation uses the executable's context and its injected process and output services.

Use `ProcessCommand::arg` to keep a path or value in one argument.
Pass private child input through `ProcessCommand::private_input`.
The native process waits for the child and drains both output streams with bounded buffers.
It returns the child's failure status and stops the owned process tree on cancellation.

Use `#[tasks("./tasks")]` to register task files from a directory.
Add `foundation_tasks::build::task_dir` to the binary package's build script.
The helper records build dependencies for new, updated, renamed, and removed files.
Each source file supplies one visible asynchronous function named `task` with a documentation summary.

### Embedded interpreter scripts

Enable the `scripts` feature when a task requires an interpreter.
Store the script and its companion files under the task package's `src/tasks/scripts` directory.
Derive `EmbedDirectoryAs` with `#[always_embed]` and use uncompressed bytes.
Tell Cargo to watch that directory in the package build script.
Pass the embedded directory to `scripts::run` with the interpreter and its arguments.

Use `ScriptPlatform::host()` for a host tool.
Select the target platform explicitly for a device tool.
Declare each entry in a `ScriptVariant`. Set its platform to `None` for a portable entry.
The runner removes the temporary files after success, failure, or cancellation.
