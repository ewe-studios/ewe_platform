//! Keep the repository task command compatible with the application-owned checks.
use clap::{Arg, ArgAction, Command};
use foundation_tasks::{task, ProcessCommand, TaskResult, TaskRunner, TaskSet};
use foundation_ui_native::tasks::{self, NativeBuildEnv};

enum Argument {
    Flag(&'static str),
    Option(&'static str),
    Positional(&'static str),
}
fn application_command(
    set: &mut TaskSet,
    name: &'static str,
    local: &'static str,
    summary: &'static str,
    arguments: &'static [Argument],
) -> TaskResult {
    let mut command = Command::new(name).about(summary);
    for argument in arguments {
        command = command.arg(match argument {
            Argument::Flag(key) => Arg::new(*key).long(*key).action(ArgAction::SetTrue),
            Argument::Option(key) => Arg::new(*key).long(*key),
            Argument::Positional(key) => Arg::new(*key),
        });
    }
    set.add_command(name, summary, command, move |ctx, values| async move {
        let root = ctx.require::<NativeBuildEnv>()?.root().await?;
        let mut cmd = ProcessCommand::new("cargo")
            .current_dir(&root)
            .args(["run", "--locked", "--quiet", "--manifest-path"])
            .arg(root.join("examples/ui_todo/tools/tasks/Cargo.toml"))
            .args(["--", "task", "run", local]);
        for argument in arguments {
            match argument {
                Argument::Flag(key) if values.get_flag(key) => cmd = cmd.arg(format!("--{key}")),
                Argument::Option(key) => {
                    if let Some(value) = values.get_one::<String>(key) {
                        cmd = cmd.arg(format!("--{key}")).arg(value);
                    }
                }
                Argument::Positional(key) => {
                    if let Some(value) = values.get_one::<String>(key) {
                        cmd = cmd.arg(value);
                    }
                }
                Argument::Flag(_) => {}
            }
        }
        ctx.exec(cmd).await
    })
}
#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    use Argument::{Flag, Option, Positional};
    runner.context().store(NativeBuildEnv::from_workspace());
    let mut set = tasks::release_set()?;
    tasks::add_apple_build(&mut set)?;
    application_command(
        &mut set,
        "build-android",
        "todo:build-android",
        "Build Android libraries",
        &[Flag("with-todo")],
    )?;
    application_command(
        &mut set,
        "bindings:sync-android",
        "todo:sync-android",
        "Update or check the Android bindings",
        &[Flag("check")],
    )?;
    application_command(
        &mut set,
        "todo:prepare-ios",
        "todo:prepare-ios",
        "Prepare the Apple Todo applications",
        &[Flag("sync-bridge"), Positional("output")],
    )?;
    application_command(
        &mut set,
        "todo:test-ios",
        "todo:test-ios",
        "Test the Apple Todo applications",
        &[Positional("device")],
    )?;
    application_command(
        &mut set,
        "todo:test-android",
        "todo:test-android",
        "Test the Android Todo applications",
        &[Option("maven")],
    )?;
    application_command(
        &mut set,
        "release:test-ios-starter",
        "todo:test-ios-starter",
        "Test the Apple starter application",
        &[Option("local-package")],
    )?;
    application_command(
        &mut set,
        "release:test-android-starter",
        "todo:test-android-starter",
        "Test the Android starter application",
        &[Option("maven")],
    )?;
    runner.register(set)?;
    runner.run().await
}
