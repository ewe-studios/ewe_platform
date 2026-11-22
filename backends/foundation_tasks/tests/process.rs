use std::{
    io::{self, Write},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use foundation_tasks::{
    Cancellation, NativeProcess, OutputStream, ProcessCommand, ProcessConfig, TaskError,
    TaskOutput, TaskProcess,
};
use futures_util::future::BoxFuture;

#[derive(Default)]
struct Memory {
    chunks: Mutex<Vec<(OutputStream, Vec<u8>)>>,
    fail: bool,
}

impl TaskOutput for Memory {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            futures_lite::future::yield_now().await;
            if self.fail {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "test output closed",
                ));
            }
            self.chunks.lock().unwrap().push((stream, bytes.to_vec()));
            Ok(())
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn child_entry() {
    let Ok(mode) = std::env::var("FOUNDATION_TASKS_CHILD") else {
        return;
    };
    match mode.as_str() {
        "pipes" => {
            std::thread::scope(|scope| {
                scope.spawn(|| io::stdout().lock().write_all(&[b'A'; 32_768]).unwrap());
                scope.spawn(|| io::stderr().lock().write_all(&[b'B'; 32_768]).unwrap());
            });
            std::process::exit(7);
        }
        "wait" => {
            println!("fixture ready");
            std::thread::park();
        }
        "argument" => {
            let required = std::env::var_os("FOUNDATION_TASKS_ARGUMENT").unwrap();
            assert_eq!(std::env::args_os().last(), Some(required));
        }
        "tree" => {
            let mut descendant = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "process::child_entry", "--nocapture"])
                .env("FOUNDATION_TASKS_CHILD", "descendant")
                .spawn()
                .unwrap();
            println!("owned tree ready");
            descendant.wait().unwrap();
        }
        "descendant" => {
            std::thread::sleep(std::time::Duration::from_millis(600));
            std::fs::write(
                std::env::var_os("FOUNDATION_TASKS_MARKER").unwrap(),
                b"survived",
            )
            .unwrap();
        }
        _ => panic!("unexpected child mode"),
    }
}

fn fixture(mode: &str) -> ProcessCommand {
    ProcessCommand::new(std::env::current_exe().unwrap())
        .args(["--exact", "process::child_entry", "--nocapture"])
        .env("FOUNDATION_TASKS_CHILD", mode)
}

#[test]
fn drains_both_pipes_in_bounded_chunks_and_preserves_exit_code() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let process = NativeProcess::new(
        runtime.handle().clone(),
        ProcessConfig::default().with_chunk_bytes(NonZeroUsize::new(113).unwrap()),
    );
    let output = Arc::new(Memory::default());
    let error = runtime
        .block_on(process.run(fixture("pipes"), output.clone(), Cancellation::new()))
        .unwrap_err();
    assert_eq!(error.exit_code(), 7);
    let chunks = output.chunks.lock().unwrap();
    assert!(chunks.iter().all(|(_, chunk)| chunk.len() <= 113));
    let stdout = chunks
        .iter()
        .filter(|(stream, _)| *stream == OutputStream::Stdout)
        .flat_map(|(_, chunk)| chunk)
        .copied()
        .collect::<Vec<_>>();
    assert!(stdout.ends_with(&[b'A'; 32_768]));
    let stderr = chunks
        .iter()
        .filter(|(stream, _)| *stream == OutputStream::Stderr)
        .flat_map(|(_, chunk)| chunk)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(stderr, [b'B'; 32_768]);
}

#[test]
fn a_sink_failure_stops_the_child() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let process = NativeProcess::new(runtime.handle().clone(), ProcessConfig::default());
    let output = Arc::new(Memory {
        fail: true,
        ..Memory::default()
    });
    let error = runtime
        .block_on(process.run(fixture("wait"), output, Cancellation::new()))
        .unwrap_err();
    assert!(
        matches!(error, TaskError::Process { source, .. } if source.kind() == io::ErrorKind::BrokenPipe)
    );
}

struct CancelOnReady {
    bytes: Mutex<Vec<u8>>,
    cancellation: Cancellation,
}

impl TaskOutput for CancelOnReady {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if stream == OutputStream::Stdout {
                let mut output = self.bytes.lock().unwrap();
                output.extend_from_slice(bytes);
                if output
                    .windows(b"owned tree ready".len())
                    .any(|part| part == b"owned tree ready")
                {
                    self.cancellation.cancel();
                }
            }
            Ok(())
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn cancellation_stops_the_entire_child_tree() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("path with spaces");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let process = NativeProcess::new(runtime.handle().clone(), ProcessConfig::default());
    let cancellation = Cancellation::new();
    let output = Arc::new(CancelOnReady {
        bytes: Mutex::default(),
        cancellation: cancellation.clone(),
    });
    let error = runtime
        .block_on(process.run(
            fixture("tree").env("FOUNDATION_TASKS_MARKER", marker.as_os_str()),
            output,
            cancellation,
        ))
        .unwrap_err();
    assert_eq!(error.exit_code(), 130);
    std::thread::sleep(std::time::Duration::from_millis(750));
    assert!(
        !marker.exists(),
        "An owned descendant survived cancellation"
    );
}

#[test]
fn a_path_with_shell_characters_reaches_the_child_as_one_argument() {
    let directory = tempfile::tempdir().unwrap();
    let argument = directory.path().join("a directory with spaces; $name");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let process = NativeProcess::new(runtime.handle().clone(), ProcessConfig::default());
    let command = fixture("argument")
        .arg(argument.as_os_str())
        .env("FOUNDATION_TASKS_ARGUMENT", argument.as_os_str());
    runtime
        .block_on(process.run(command, Arc::new(Memory::default()), Cancellation::new()))
        .unwrap();
}
