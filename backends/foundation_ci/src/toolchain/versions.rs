use super::Report;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskError, TaskOutput, TaskResult,
};
use futures_util::future::BoxFuture;
use regex::Regex;
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, BufRead, BufReader, Seek, Write},
    sync::{Arc, Mutex},
};

struct VersionOutput(Mutex<File>);
impl TaskOutput for VersionOutput {
    fn write<'a>(&'a self, _: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.0
                .lock()
                .map_err(|_| io::Error::other("Version output unavailable"))?
                .write_all(bytes)
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub(super) async fn value(
    ctx: &TaskContext,
    program: impl AsRef<OsStr>,
    args: &[&str],
    pattern: &str,
    env: Option<(&str, &str)>,
) -> Result<Result<String, String>, TaskError> {
    let mut command = ProcessCommand::new(program.as_ref()).args(args.iter().copied());
    if let Some((key, val)) = env {
        command = command.env(key, val);
    }
    read(ctx, command, pattern).await
}

pub(super) async fn read(
    ctx: &TaskContext,
    command: ProcessCommand,
    pattern: &str,
) -> Result<Result<String, String>, TaskError> {
    let expression = Regex::new(pattern).map_err(TaskError::operation)?;
    let mut file = tempfile::tempfile().map_err(TaskError::operation)?;
    let output = Arc::new(VersionOutput(Mutex::new(
        file.try_clone().map_err(TaskError::operation)?,
    )));
    match ctx.exec_with_output(command, output).await {
        Ok(()) => {}
        Err(TaskError::Process { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(Err("not found".into()))
        }
        Err(TaskError::Exit { .. } | TaskError::Process { .. }) => {
            return Ok(Err("version command failed".into()))
        }
        Err(error) => return Err(error),
    }
    file.rewind().map_err(TaskError::operation)?;
    for line in BufReader::new(file).lines() {
        if let Some(m) = expression.captures(&line.map_err(TaskError::operation)?) {
            return Ok(Ok(m[1].to_owned()));
        }
    }
    Ok(Err("could not read version".into()))
}

pub(super) fn equal(value: Result<String, String>, expected: &str) -> Result<String, String> {
    value.and_then(|v| {
        if v == expected {
            Ok(v)
        } else {
            Err(format!("expected {expected}, got {v}"))
        }
    })
}

pub(super) fn minimum(value: Result<String, String>, expected: &str) -> Result<String, String> {
    fn parts(text: &str) -> Result<Vec<u32>, String> {
        let mut parts = text
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| format!("Invalid version {text}"))?;
        while parts.last() == Some(&0) {
            parts.pop();
        }
        Ok(parts)
    }
    let actual = value
        .map_err(|err| format!("{err}; set DEVELOPER_DIR to the Xcode developer directory"))?;
    if parts(&actual)? < parts(expected)? {
        Err(format!("expected at least {expected}, got {actual}"))
    } else {
        Ok(actual)
    }
}

pub(super) async fn exact(
    ctx: &TaskContext,
    report: &mut Report,
    name: &str,
    expected: &str,
    command: impl AsRef<OsStr>,
    args: &[&str],
    pattern: &str,
) -> TaskResult {
    report.entry(
        name,
        equal(value(ctx, command, args, pattern, None).await?, expected),
    );
    Ok(())
}
