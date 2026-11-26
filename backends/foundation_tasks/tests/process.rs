use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use foundation_context::Ctx;
use foundation_core::valtron::{sleep_async, valtron_test};
use foundation_tasks::{
    Cancellation, NativeProcess, OutputStream, ProcessCommand, ProcessConfig, TaskError,
    TaskOutput, TaskProcess,
};
use futures_util::future::BoxFuture;

static STDOUT_BYTES: [u8; 256 * 1024] = [b'A'; 256 * 1024];
static STDERR_BYTES: [u8; 256 * 1024] = [b'B'; 256 * 1024];

#[derive(Default)]
struct Memory {
    chunks: Mutex<Vec<(OutputStream, Vec<u8>)>>,
    fail_after: Option<std::time::Duration>,
}

impl TaskOutput for Memory {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            futures_lite::future::yield_now().await;
            if let Some(delay) = self.fail_after {
                sleep_async(delay, &Ctx::host()).await;
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
                scope.spawn(|| io::stdout().lock().write_all(&STDOUT_BYTES).unwrap());
                scope.spawn(|| io::stderr().lock().write_all(&STDERR_BYTES).unwrap());
            });
            std::process::exit(7);
        }
        #[cfg(unix)]
        "exit-with-id" => {
            let address = std::env::var("FOUNDATION_TASKS_ADDRESS").unwrap();
            let mut socket = std::net::TcpStream::connect(address).unwrap();
            socket.write_all(&std::process::id().to_ne_bytes()).unwrap();
            std::process::exit(0);
        }
        "exit" => {
            io::stdout().lock().write_all(b"finished\n").unwrap();
            std::process::exit(0);
        }
        "service-echo" => {
            let address = std::env::var("FOUNDATION_TASKS_ADDRESS").unwrap();
            let mut socket = std::net::TcpStream::connect(address).unwrap();
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            socket.write_all(&byte).unwrap();
        }
        "socket" => {
            let address = std::env::var("FOUNDATION_TASKS_ADDRESS").unwrap();
            let mut socket = std::net::TcpStream::connect(address).unwrap();
            println!("owned tree ready");
            let _ = socket.read(&mut [0]).unwrap();
        }
        "repeat" => {
            let mut bytes = [0; 6];
            io::stdin().lock().read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"y\ny\ny\n");
            std::process::exit(42);
        }
        "archive" => {
            let mut input = io::stdin().lock();
            let mut block = [0; 1024];
            for chunk in 0..=255 {
                input.read_exact(&mut block).unwrap();
                assert!(block.iter().all(|byte| *byte == chunk));
            }
            assert_eq!(input.read(&mut block).unwrap(), 0);
        }
        "private" => {
            let mut input = Vec::new();
            io::stdin().lock().read_to_end(&mut input).unwrap();
            assert_eq!(input, b"private test input");
            assert!(std::env::args_os().all(|arg| arg != "private test input"));
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

#[valtron_test(timeout = 30000)]
async fn drains_both_pipes_in_bounded_chunks_and_preserves_exit_code() {
    let process = NativeProcess::new(
        ProcessConfig::default().with_chunk_bytes(NonZeroUsize::new(113).unwrap()),
    );
    let output = Arc::new(Memory::default());
    let error = process
        .run(fixture("pipes"), output.clone(), Cancellation::new())
        .await
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
    assert!(stdout.ends_with(&STDOUT_BYTES));
    let stderr = chunks
        .iter()
        .filter(|(stream, _)| *stream == OutputStream::Stderr)
        .flat_map(|(_, chunk)| chunk)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(stderr.as_slice(), &STDERR_BYTES);
}

#[valtron_test(timeout = 30000)]
async fn a_sink_failure_stops_the_child() {
    let process = NativeProcess::new(ProcessConfig::default());
    let output = Arc::new(Memory {
        fail_after: Some(std::time::Duration::ZERO),
        ..Memory::default()
    });
    let error = process
        .run(fixture("wait"), output, Cancellation::new())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, TaskError::Process { source, .. } if source.kind() == io::ErrorKind::BrokenPipe),
        "{error:?}"
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

#[valtron_test(timeout = 30000)]
async fn cancellation_stops_the_entire_child_tree() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("path with spaces");
    let process = NativeProcess::new(ProcessConfig::default());
    let cancellation = Cancellation::new();
    let output = Arc::new(CancelOnReady {
        bytes: Mutex::default(),
        cancellation: cancellation.clone(),
    });
    let error = process
        .run(
            fixture("tree")
                .env("FOUNDATION_TASKS_MARKER", marker.as_os_str())
                .private_input(vec![0; 4 * 1024 * 1024]),
            output,
            cancellation,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, TaskError::Cancelled));
    sleep_async(std::time::Duration::from_millis(750), &Ctx::host()).await;
    assert!(
        !marker.exists(),
        "An owned descendant survived cancellation"
    );
}

#[valtron_test(timeout = 30000)]
async fn a_path_with_shell_characters_reaches_the_child_as_one_argument() {
    let directory = tempfile::tempdir().unwrap();
    let argument = directory.path().join("a directory with spaces; $name");
    let process = NativeProcess::new(ProcessConfig::default());
    let command = fixture("argument")
        .arg(argument.as_os_str())
        .env("FOUNDATION_TASKS_ARGUMENT", argument.as_os_str());
    process
        .run(command, Arc::new(Memory::default()), Cancellation::new())
        .await
        .unwrap();
}

#[valtron_test(timeout = 30000)]
async fn repeated_input_preserves_the_child_exit_status() {
    let process = NativeProcess::new(ProcessConfig::default());
    let result = process
        .run(
            fixture("repeat").repeated_input(b"y\n".to_vec()),
            Arc::new(Memory::default()),
            Cancellation::new(),
        )
        .await;
    assert!(matches!(result, Err(TaskError::Exit { code: 42, .. })));
}

#[valtron_test(timeout = 30000)]
async fn private_input_reaches_only_the_child_pipe() {
    let process = NativeProcess::new(ProcessConfig::default());
    let output = Arc::new(Memory::default());
    let command = fixture("private").private_input(b"private test input".to_vec());
    process
        .run(command, output.clone(), Cancellation::new())
        .await
        .unwrap();
    for stream in [OutputStream::Stdout, OutputStream::Stderr] {
        let bytes: Vec<_> = output
            .chunks
            .lock()
            .unwrap()
            .iter()
            .filter(|(source, _)| *source == stream)
            .flat_map(|(_, chunk)| chunk.iter().copied())
            .collect();
        assert!(!bytes
            .windows(b"private test input".len())
            .any(|part| part == b"private test input"));
    }
}

#[valtron_test(timeout = 30000)]
async fn a_late_sink_failure_preserves_the_original_error() {
    let process = NativeProcess::new(ProcessConfig::default());
    let output = Arc::new(Memory {
        fail_after: Some(std::time::Duration::from_millis(200)),
        ..Memory::default()
    });
    let error = process
        .run(fixture("exit"), output, Cancellation::new())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, TaskError::Process { source, .. } if source.kind() == io::ErrorKind::BrokenPipe),
        "{error:?}"
    );
}

#[valtron_test(timeout = 30000)]
async fn cancellation_before_start_skips_process_creation() {
    let process = NativeProcess::new(ProcessConfig::default());
    let directory = tempfile::tempdir().unwrap();
    let command = ProcessCommand::new(directory.path().join("missing-program"));
    let cancellation = Cancellation::new();
    cancellation.cancel();
    let error = process
        .run(command, Arc::new(Memory::default()), cancellation)
        .await
        .unwrap_err();
    assert!(matches!(error, TaskError::Cancelled));
}

async fn drop_running_child(wait_for_output: bool) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let ready = Cancellation::new();
    let output = Arc::new(CancelOnReady {
        bytes: Mutex::default(),
        cancellation: ready.clone(),
    });
    let process = NativeProcess::new(ProcessConfig::default());
    let command = fixture("socket").env("FOUNDATION_TASKS_ADDRESS", address.to_string());
    let mut run = process.run(command, output, Cancellation::new());
    if wait_for_output {
        use futures_util::future::{select, Either};
        match select(run.as_mut(), Box::pin(ready.cancelled())).await {
            Either::Right(_) => {}
            Either::Left((result, _)) => {
                panic!("The fixture exits before its output arrives: {result:?}")
            }
        }
    } else {
        // Start the background job. Leave its result in the handoff channel.
        assert!(futures_lite::future::poll_once(run.as_mut())
            .await
            .is_none());
    }
    let (mut socket, _) = listener.accept().unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    drop(run);
    match socket.read(&mut [0]) {
        Ok(0) => {}
        Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
        result => panic!("The child keeps its socket open after drop: {result:?}"),
    }
}

#[valtron_test(timeout = 30000)]
async fn dropping_the_run_terminates_a_silent_child() {
    drop_running_child(true).await;
}

#[valtron_test(timeout = 30000)]
async fn dropping_the_startup_handoff_terminates_a_silent_child() {
    drop_running_child(false).await;
}

#[cfg(unix)]
#[valtron_test(timeout = 30000)]
async fn startup_reserves_the_exited_group_leader_until_completion() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let process = NativeProcess::new(ProcessConfig::default());
    let command = fixture("exit-with-id").env("FOUNDATION_TASKS_ADDRESS", address.to_string());
    let mut run = process.run(command, Arc::new(Memory::default()), Cancellation::new());
    assert!(futures_lite::future::poll_once(run.as_mut())
        .await
        .is_none());

    let (mut socket, _) = listener.accept().unwrap();
    let mut bytes = [0; 4];
    socket.read_exact(&mut bytes).unwrap();
    let id = u32::from_ne_bytes(bytes);
    observe_test_exit(id).unwrap();
    sleep_async(std::time::Duration::from_millis(100), &Ctx::host()).await;
    observe_test_exit(id).unwrap();
    run.await.unwrap();
    assert_eq!(
        observe_test_exit(id).unwrap_err().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[cfg(unix)]
fn observe_test_exit(id: u32) -> io::Result<()> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::uninit();
    // Observe this test child without releasing its identifier.
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            id,
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOWAIT,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[valtron_test(timeout = 30000)]
async fn detached_service_receives_work_after_ownership_transfer() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let process = NativeProcess::new(ProcessConfig::default());
    let command = fixture("service-echo").env("FOUNDATION_TASKS_ADDRESS", address.to_string());
    let mut service = process.service(command, Cancellation::new()).await.unwrap();
    let (mut socket, _) = listener.accept().unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    assert!(service.status().unwrap().is_none());
    service.detach().unwrap();
    socket.write_all(b"x").unwrap();
    let mut reply = [0];
    socket.read_exact(&mut reply).unwrap();
    assert_eq!(reply, *b"x");
}

#[valtron_test(timeout = 30000)]
async fn file_input_reopens_a_binary_file_for_each_child() {
    let root = tempfile::tempdir().unwrap();
    let name = "fixture with spaces";
    let mut file = std::fs::File::create(root.path().join(name)).unwrap();
    for byte in 0..=255 {
        file.write_all(&[byte; 1024]).unwrap();
    }
    drop(file);
    let command = fixture("archive").current_dir(root.path()).file_input(name);
    let process = NativeProcess::new(ProcessConfig::default());
    for _ in 0..2 {
        process.run(command.clone(), Arc::new(Memory::default()), Cancellation::new()).await.unwrap();
    }
    std::fs::remove_file(root.path().join(name)).unwrap();
    assert!(process.run(command, Arc::new(Memory::default()), Cancellation::new()).await.is_err());
}
