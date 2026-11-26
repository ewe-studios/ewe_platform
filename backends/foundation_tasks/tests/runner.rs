use clap::Args;
use foundation_context::Ctx;
use foundation_tasks::{TaskContext, TaskResult, TaskRunner, TaskSet};
use std::sync::{Arc, Mutex};

#[derive(Args)]
struct Values {
    #[arg(long)]
    name: String,
}

async fn record(ctx: TaskContext, values: Values) -> TaskResult {
    ctx.require::<Arc<Mutex<Vec<String>>>>()?
        .lock()
        .unwrap()
        .push(values.name);
    Ok(())
}

#[test]
fn direct_and_generic_paths_share_typed_handler() {
    let history = Arc::new(Mutex::new(Vec::<String>::new()));
    for args in [
        vec!["platform", "old_group", "original", "--name", "two words"],
        vec![
            "platform",
            "task",
            "run",
            "demo:record",
            "--name",
            "two words",
        ],
    ] {
        let ctx = Ctx::host();
        ctx.store(history.clone());
        let mut runner = TaskRunner::new(ctx, args.into_iter().map(Into::into).collect());
        let mut set = TaskSet::new("test-owner").with_namespace("demo");
        set.add("record", "Record a name", record).unwrap();
        set.expose("record", &["old_group", "original"]).unwrap();
        runner.register(set).unwrap();
        futures_lite::future::block_on(runner.run()).unwrap();
    }
    assert_eq!(*history.lock().unwrap(), ["two words", "two words"]);
}

#[test]
fn a_conflicting_set_does_not_register_other_entries() {
    let history = Arc::new(Mutex::new(Vec::<String>::new()));
    let ctx = Ctx::host();
    ctx.store(history.clone());
    let args = ["platform", "build", "--name", "first"];
    let mut runner = TaskRunner::new(ctx, args.into_iter().map(Into::into).collect());
    let mut first = TaskSet::new("first-owner").with_namespace("one");
    first.add("first", "Record first", record).unwrap();
    first.expose("first", &["build"]).unwrap();
    runner.register(first).unwrap();

    let mut second = TaskSet::new("second-owner").with_namespace("two");
    second.add("conflict", "Conflict", record).unwrap();
    second.expose("conflict", &["build", "new"]).unwrap();
    second.add("other", "Other", record).unwrap();
    let error = runner.register(second).unwrap_err().to_string();
    assert!(error.contains("first-owner") && error.contains("second-owner"));

    let mut replacement = TaskSet::new("replacement").with_namespace("two");
    replacement.add("other", "Replacement", record).unwrap();
    runner.register(replacement).unwrap();
    futures_lite::future::block_on(runner.run()).unwrap();
    assert_eq!(*history.lock().unwrap(), ["first"]);
}

#[test]
fn task_errors_preserve_the_message_and_cause() {
    use foundation_tasks::TaskError;
    use std::error::Error;

    let missing = TaskError::InvalidName("bad task".into());
    assert_eq!(missing.to_string(), "Invalid task name or path: bad task");
    assert!(missing.source().is_none());

    let source = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed output");
    let error = TaskError::Task {
        task: "example".into(),
        source: Box::new(TaskError::operation(source)),
    };
    assert_eq!(error.to_string(), "example: closed output");
    let operation = error.source().unwrap();
    assert_eq!(operation.to_string(), "closed output");
    assert_eq!(operation.source().unwrap().to_string(), "closed output");
}

#[test]
fn host_dispatch_installs_the_timer() {
    const CHILD: &str = "EWE_TASKS_HOST_TIMER_TEST";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "runner::host_dispatch_installs_the_timer"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let exit = foundation_tasks::run_entry("fixture", |host| {
        Box::pin(async move {
            let mut tasks = TaskSet::new("fixture");
            tasks.add("wait", "Wait for a timer", |ctx, _: Values| async move {
                let services = ctx
                    .context()
                    .require_env()
                    .map_err(foundation_tasks::TaskError::operation)?;
                let deadline = ctx.context().clock().now() + std::time::Duration::from_millis(1);
                services
                    .timer()
                    .sleep_until(ctx.context().clone(), deadline)
                    .await;
                Ok(())
            })?;
            let args = ["fixture", "task", "run", "wait", "--name", "timer"]
                .into_iter()
                .map(Into::into)
                .collect();
            let mut runner = TaskRunner::new(host.context().clone(), args);
            runner.register(tasks)?;
            runner.run().await
        })
    });
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}
