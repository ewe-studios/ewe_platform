//! Search large device reports one line at a time.
use foundation_tasks::{OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput};
use futures_util::future::BoxFuture;
use regex::bytes::Regex;
use std::{
    io,
    sync::{Arc, Mutex},
};

struct Lines {
    state: Mutex<(Vec<u8>, Option<String>)>,
    matcher: Regex,
    limit: usize,
    output: Arc<dyn TaskOutput>,
}
impl Lines {
    fn check(&self, state: &mut (Vec<u8>, Option<String>)) -> io::Result<()> {
        if let Some(hit) = self.matcher.captures(&state.0) {
            let value = hit
                .get(1)
                .or_else(|| hit.get(0))
                .ok_or_else(|| io::Error::other("Missing match"))?;
            state.1 =
                Some(String::from_utf8(value.as_bytes().to_owned()).map_err(io::Error::other)?);
        }
        state.0.clear();
        Ok(())
    }
}
impl TaskOutput for Lines {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if stream == OutputStream::Stderr {
                return self.output.write(stream, bytes).await;
            }
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("Device report unavailable"))?;
            for byte in bytes {
                if state.1.is_some() {
                    break;
                }
                if *byte == b'\n' {
                    self.check(&mut state)?;
                } else {
                    if state.0.len() >= self.limit {
                        return Err(io::Error::other("Device report line exceeds limit"));
                    }
                    state.0.push(*byte);
                }
            }
            Ok(())
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.output.flush()
    }
}
pub async fn find(
    ctx: &TaskContext,
    command: ProcessCommand,
    pattern: &str,
) -> Result<Option<String>, TaskError> {
    let lines = Arc::new(Lines {
        state: Mutex::default(),
        matcher: Regex::new(pattern).map_err(TaskError::operation)?,
        limit: ctx.require::<super::NativeBuildEnv>()?.query_bytes(),
        output: ctx.require::<TaskEnv>()?.output.clone(),
    });
    ctx.exec_with_output(command, lines.clone()).await?;
    let mut state = lines
        .state
        .lock()
        .map_err(|_| TaskError::operation("Device report unavailable"))?;
    if state.1.is_none() {
        lines.check(&mut state).map_err(TaskError::operation)?;
    }
    Ok(state.1.take())
}
