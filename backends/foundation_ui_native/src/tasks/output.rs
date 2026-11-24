use foundation_tasks::{OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput};
use futures_util::future::BoxFuture;
use std::{
    io,
    sync::{Arc, Mutex},
};

struct Query {
    stdout: Mutex<Vec<u8>>,
    forward: Arc<dyn TaskOutput>,
    limit: usize,
}

impl TaskOutput for Query {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if stream == OutputStream::Stderr {
                return self.forward.write(stream, bytes).await;
            }
            let mut stdout = self
                .stdout
                .lock()
                .map_err(|_| io::Error::other("Query buffer is unavailable"))?;
            if bytes.len() > self.limit.saturating_sub(stdout.len()) {
                return Err(io::Error::other("Command output exceeds the query limit"));
            }
            stdout.extend_from_slice(bytes);
            Ok(())
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.forward.flush()
    }
}

/// Read bounded standard output from a successful command.
///
/// # Errors
/// Returns process failures and output limit errors.
pub async fn query(ctx: &TaskContext, command: ProcessCommand) -> Result<Vec<u8>, TaskError> {
    let (result, bytes) = query_status(ctx, command).await?;
    result?;
    Ok(bytes)
}

pub(super) async fn query_status(
    ctx: &TaskContext,
    command: ProcessCommand,
) -> Result<(Result<(), TaskError>, Vec<u8>), TaskError> {
    let env = ctx.require::<super::NativeBuildEnv>()?;
    query_limit(ctx, command, env.query_bytes).await
}

/// Read a bounded device report.
///
/// # Errors
/// Returns process failures and device report limit errors.
pub async fn device_query(
    ctx: &TaskContext,
    command: ProcessCommand,
) -> Result<Vec<u8>, TaskError> {
    let env = ctx.require::<super::NativeBuildEnv>()?;
    let (result, data) = query_limit(ctx, command, env.device_query_bytes).await?;
    result?;
    Ok(data)
}

async fn query_limit(
    ctx: &TaskContext,
    command: ProcessCommand,
    limit: usize,
) -> Result<(Result<(), TaskError>, Vec<u8>), TaskError> {
    let output = Arc::new(Query {
        stdout: Mutex::default(),
        forward: ctx.require::<TaskEnv>()?.output.clone(),
        limit,
    });
    let result = ctx.exec_with_output(command, output.clone()).await;
    let mut bytes = output
        .stdout
        .lock()
        .map_err(|_| TaskError::operation("Query buffer is unavailable"))?;
    Ok((result, std::mem::take(&mut *bytes)))
}

struct Verification {
    forward: Arc<dyn TaskOutput>,
    state: Mutex<(usize, bool)>,
}

impl TaskOutput for Verification {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.forward.write(stream, bytes).await?;
            if stream == OutputStream::Stdout {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| io::Error::other("Signature result is unavailable"))?;
                for byte in bytes {
                    match byte {
                        b'\n' => {
                            state.1 |= state.0 == Self::MARKER.len();
                            state.0 = 0;
                        }
                        b'\r' if state.0 == Self::MARKER.len() => {}
                        byte if Self::MARKER.get(state.0) == Some(byte) => state.0 += 1,
                        _ => state.0 = Self::MARKER.len() + 1,
                    }
                }
            }
            Ok(())
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.forward.flush()
    }
}

impl Verification {
    const MARKER: &'static [u8] = b"jar verified.";
}

pub(super) async fn verifies_signature(
    ctx: &TaskContext,
    command: ProcessCommand,
) -> Result<bool, TaskError> {
    let output = Arc::new(Verification {
        forward: ctx.require::<TaskEnv>()?.output.clone(),
        state: Mutex::default(),
    });
    ctx.exec_with_output(command, output.clone()).await?;
    let state = output
        .state
        .lock()
        .map_err(|_| TaskError::operation("Signature result is unavailable"))?;
    Ok(state.1 || state.0 == Verification::MARKER.len())
}

struct Artifact {
    lines: Mutex<ArtifactLines>,
    package: &'static str,
    extensions: &'static [&'static str],
    forward: Arc<dyn TaskOutput>,
    limit: usize,
}

#[derive(Default)]
struct ArtifactLines {
    pending: Vec<u8>,
    found: Option<std::path::PathBuf>,
}

impl Artifact {
    fn parse(&self, lines: &mut ArtifactLines) -> io::Result<Option<String>> {
        let event: serde_json::Value = serde_json::from_slice(&lines.pending)?;
        let reason = event.get("reason").and_then(serde_json::Value::as_str);
        let name = event
            .pointer("/target/name")
            .and_then(serde_json::Value::as_str);
        if reason == Some("compiler-artifact") && name == Some(self.package) {
            let product = if self.extensions.is_empty() {
                event.get("executable").and_then(serde_json::Value::as_str)
            } else {
                event
                    .get("filenames")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|filenames| {
                        filenames
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .find(|path| self.extensions.iter().any(|ext| path.ends_with(ext)))
                    })
            };
            if let Some(path) = product {
                lines.found = Some(std::path::PathBuf::from(path));
            }
        }
        Ok(if reason == Some("compiler-message") {
            event
                .pointer("/message/rendered")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        } else {
            None
        })
    }
}

impl TaskOutput for Artifact {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if stream == OutputStream::Stderr {
                return self.forward.write(stream, bytes).await;
            }
            let diagnostics = {
                let mut lines = self
                    .lines
                    .lock()
                    .map_err(|_| io::Error::other("Artifact parser is unavailable"))?;
                let mut diagnostics = Vec::new();
                for part in bytes.split_inclusive(|byte| *byte == b'\n') {
                    if part.len() > self.limit.saturating_sub(lines.pending.len()) {
                        return Err(io::Error::other(
                            "A Cargo message exceeds the configured limit",
                        ));
                    }
                    lines.pending.extend_from_slice(part);
                    if part.ends_with(b"\n") {
                        diagnostics.extend(self.parse(&mut lines)?);
                        lines.pending.clear();
                    }
                }
                diagnostics
            };
            for diagnostic in diagnostics {
                self.forward
                    .write(OutputStream::Stderr, diagnostic.as_bytes())
                    .await?;
            }
            Ok(())
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.forward.flush()
    }
}

/// Find a package artifact in Cargo build messages.
///
/// # Errors
/// Returns process failures, invalid messages, or a missing artifact error.
pub async fn artifact(
    ctx: &TaskContext,
    command: ProcessCommand,
    package: &'static str,
    extensions: &'static [&'static str],
) -> Result<std::path::PathBuf, TaskError> {
    let env = ctx.require::<super::NativeBuildEnv>()?;
    let output = Arc::new(Artifact {
        lines: Mutex::default(),
        package,
        extensions,
        forward: ctx.require::<TaskEnv>()?.output.clone(),
        limit: env.cargo_message_bytes,
    });
    ctx.exec_with_output(command, output.clone()).await?;
    let mut state = output
        .lines
        .lock()
        .map_err(|_| TaskError::operation("Artifact parser is unavailable"))?;
    state
        .found
        .take()
        .ok_or_else(|| TaskError::operation(format!("Cargo did not produce {package}")))
}
