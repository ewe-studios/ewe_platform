//! Native log bridge tests moved out of `src/log.rs` (spec 70 feature 59,
//! rule 7).
//!
//! The callback, `State` and `Module` were crate-private for tests. They are
//! now public so the buffering behaviour stays covered.

use std::sync::{Arc, Mutex};

use infrastructure_llama_cpp::log::{Module, State};
use infrastructure_llama_cpp::{logs_to_trace, LogOptions};
use tracing::subscriber::DefaultGuard;
use tracing_subscriber::util::SubscriberInitExt;

struct Logger {
    #[allow(unused)]
    guard: DefaultGuard,
    logs: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
struct VecWriter(Arc<Mutex<Vec<String>>>);

impl std::io::Write for VecWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let log_line = String::from_utf8(buf.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8"))?;
        self.0.lock().unwrap().push(log_line);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn create_logger(max_level: tracing::Level) -> Logger {
    let logs = Arc::new(Mutex::new(vec![]));
    let writer = VecWriter(logs.clone());

    Logger {
        guard: tracing_subscriber::fmt()
            .with_max_level(max_level)
            .with_ansi(false)
            .without_time()
            .with_file(false)
            .with_line_number(false)
            .with_level(false)
            .with_target(false)
            .with_writer(move || writer.clone())
            .finish()
            .set_default(),
        logs,
    }
}

#[test]
fn cont_disabled_log() {
    // Feeds ggml DEBUG (emitted at tracing TRACE) into an INFO subscriber,
    // so nothing should be captured.
    let logger = create_logger(tracing::Level::INFO);
    let mut log_state = Box::new(State::new(Module::LlamaCpp, LogOptions::default()));
    let log_ptr = std::ptr::from_mut::<State>(log_state.as_mut()).cast::<std::os::raw::c_void>();

    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_DEBUG,
        c"Hello ".as_ptr(),
        log_ptr,
    );
    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT,
        c"world\n".as_ptr(),
        log_ptr,
    );

    assert!(logger.logs.lock().unwrap().is_empty());

    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_DEBUG,
        c"Hello ".as_ptr(),
        log_ptr,
    );
    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT,
        c"world".as_ptr(),
        log_ptr,
    );
    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT,
        c"\n".as_ptr(),
        log_ptr,
    );
}

#[test]
fn cont_enabled_log() {
    // DEBUG, not INFO: ggml INFO is emitted at tracing DEBUG (see
    // `tracing_level_for`), so a subscriber at INFO would filter the very
    // events this test is about and assert on an empty log.
    let logger = create_logger(tracing::Level::DEBUG);
    let mut log_state = Box::new(State::new(Module::LlamaCpp, LogOptions::default()));
    let log_ptr = std::ptr::from_mut::<State>(log_state.as_mut()).cast::<std::os::raw::c_void>();

    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_INFO,
        c"Hello ".as_ptr(),
        log_ptr,
    );
    logs_to_trace(
        infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT,
        c"world\n".as_ptr(),
        log_ptr,
    );

    // Not sure where the extra \n comes from.
    assert_eq!(*logger.logs.lock().unwrap(), vec!["Hello world\n\n"]);
}
