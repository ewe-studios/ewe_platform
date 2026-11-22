use std::{fs, path::Path, process::Command};

const TASK: &str = r#"
use clap::Args;
use foundation_tasks::{TaskContext, TaskResult};
#[derive(Args)]
pub(super) struct Arguments {}
/// Print a greeting.
pub(super) async fn task(_: TaskContext, _: Arguments) -> TaskResult { Ok(()) }
"#;

fn cargo(manifest: &Path) -> std::process::Output {
    Command::new(env!("CARGO"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(manifest)
        .args(["--", "task", "ls"])
        .output()
        .expect("start cargo")
}

fn list(manifest: &Path) -> String {
    let result = cargo(manifest);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn rebuilds_index_after_add_rename_and_remove() {
    let root = tempfile::tempdir().unwrap();
    let manifest = root.path().join("Cargo.toml");
    let library = format!("{:?}", env!("CARGO_MANIFEST_DIR"));
    fs::write(
        &manifest,
        format!(
            r#"
[package]
name = "discover-tasks"
version = "0.0.0"
edition = "2021"
[dependencies]
clap = {{ version = "4", features = ["derive"] }}
foundation_tasks = {{ path = {library} }}
[build-dependencies]
foundation_tasks = {{ path = {library} }}
"#
        ),
    )
    .unwrap();
    fs::write(
        root.path().join("build.rs"),
        r#"
fn main() {
    foundation_tasks::build::task_dir("discover-tasks", "src/main.rs", "./tasks").unwrap();
}
"#,
    )
    .unwrap();
    let source = root.path().join("src");
    let tasks = source.join("tasks");
    fs::create_dir_all(&tasks).unwrap();
    fs::write(
        source.join("main.rs"),
        r#"
use foundation_tasks::{tasks, TaskRunner, TaskResult};
#[tasks("./tasks", namespace = "demo")]
async fn main(runner: TaskRunner) -> TaskResult {
    runner.run().await
}
"#,
    )
    .unwrap();

    fs::write(tasks.join("begin.rs"), TASK).unwrap();
    assert!(list(&manifest).contains("demo:begin"));
    fs::write(tasks.join("next_task.rs"), TASK).unwrap();
    let added = list(&manifest);
    assert!(added.contains("demo:begin") && added.contains("demo:next-task"));
    fs::rename(tasks.join("next_task.rs"), tasks.join("final_task.rs")).unwrap();
    let renamed = list(&manifest);
    assert!(renamed.contains("demo:final-task") && !renamed.contains("demo:next-task"));
    fs::remove_file(tasks.join("begin.rs")).unwrap();
    let removed = list(&manifest);
    assert!(removed.contains("demo:final-task") && !removed.contains("demo:begin"));
    fs::write(tasks.join("final-task.rs"), TASK).unwrap();
    let collision = cargo(&manifest);
    assert!(!collision.status.success());
    assert!(String::from_utf8_lossy(&collision.stderr).contains("Task final-task occurs"));
}
