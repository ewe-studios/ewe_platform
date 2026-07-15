//! ssh2 (libssh2) backend — primary SSH transport.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::backends::Backend;
use crate::command::{Command, CommandResult};
use crate::host::Host;
use crate::pool::{connect_session, ConnectionPool};

pub struct Ssh2Backend {
    pool: Arc<ConnectionPool>,
}

impl Ssh2Backend {
    pub fn new(pool: Arc<ConnectionPool>) -> Self {
        Self { pool }
    }
}

/// A live SSH exec channel exposed as a duplex byte stream.
///
/// Reads pull the remote command's stdout; writes push to its stdin. Owns the
/// [`ssh2::Session`] it was dialed on so the channel outlives the caller's
/// session handle. Cloning yields another handle to the *same* channel (ssh2
/// `Channel`/`Session` are `Arc`-backed) — useful for split read/write halves.
#[derive(Clone)]
pub struct ChannelStream {
    channel: ssh2::Channel,
    _session: ssh2::Session,
}

impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.channel.read(buf)
    }
}

impl Write for ChannelStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.channel.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.channel.flush()
    }
}

/// A reusable SSH channel dialer that keeps one authenticated session warm.
///
/// WHY: Opening a fresh session (TCP + handshake + auth) per request is wasteful
/// when a client issues many sequential requests over the same host (e.g. a
/// Docker deploy: create → start → inspect). `Dialer` caches the session and
/// mints a **fresh channel** per [`dial`](Self::dial) — the channel is the
/// short-lived unit, the session the long-lived one.
///
/// WHAT: Holds the target [`Host`] and a lazily-established, mutex-guarded
/// session. `dial` reuses the session when it is still authenticated, otherwise
/// re-establishes it. Each returned [`ChannelStream`] owns a clone of the
/// session, so it stays alive for the channel's lifetime.
///
/// NOTE: All channels from one session share the session's internal mutex — a
/// blocking read on one blocks the others. This is correct for sequential use
/// (the common Docker case); a workload needing true concurrency should use one
/// `Dialer` per in-flight stream.
pub struct Dialer {
    host: Host,
    session: Mutex<Option<ssh2::Session>>,
}

impl Dialer {
    /// Create a dialer for `host`. The session is established lazily on first
    /// [`dial`](Self::dial).
    #[must_use]
    pub fn new(host: Host) -> Self {
        Self {
            host,
            session: Mutex::new(None),
        }
    }

    /// Open a fresh channel on the (cached) session and `exec` `command`,
    /// returning it as a duplex [`ChannelStream`].
    ///
    /// # Errors
    ///
    /// Returns a message if the session, channel, or `exec` fails.
    pub fn dial(&self, command: &str) -> Result<ChannelStream, String> {
        let session = {
            let mut guard = self.session.lock().map_err(|e| format!("lock: {e}"))?;
            match guard.as_ref() {
                Some(s) if s.authenticated() => s.clone(),
                _ => {
                    let s = connect_session(&self.host)?;
                    *guard = Some(s.clone());
                    s
                }
            }
        };
        let mut channel = session
            .channel_session()
            .map_err(|e| format!("channel: {e}"))?;
        channel.exec(command).map_err(|e| format!("exec: {e}"))?;
        Ok(ChannelStream {
            channel,
            _session: session,
        })
    }
}

impl Backend for Ssh2Backend {
    fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String> {
        let session = self.pool.get(host)?;
        let start = Instant::now();
        let shell_cmd = cmd.to_shell_command();

        let mut channel = session
            .channel_session()
            .map_err(|e| format!("channel: {e}"))?;
        channel.exec(&shell_cmd).map_err(|e| format!("exec: {e}"))?;

        let mut stdout = String::new();
        channel
            .read_to_string(&mut stdout)
            .map_err(|e| format!("read stdout: {e}"))?;

        let mut stderr = String::new();
        channel.stderr().read_to_string(&mut stderr).ok();

        channel.wait_close().ok();
        let exit_code = channel.exit_status().unwrap_or(-1);

        Ok(CommandResult {
            exit_code,
            stdout,
            stderr,
            runtime: start.elapsed(),
            host: format!("{}@{}", host.user, host.hostname),
        })
    }

    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String> {
        let session = self.pool.get(host)?;
        let local_size = std::fs::metadata(local)
            .map_err(|e| format!("stat local: {e}"))?
            .len();
        let local_data = std::fs::read(local).map_err(|e| format!("read local: {e}"))?;

        let mut remote_file = session
            .scp_send(remote, 0o644, local_size, None)
            .map_err(|e| format!("scp send: {e}"))?;
        remote_file
            .write(&local_data)
            .map_err(|e| format!("scp write: {e}"))?;
        remote_file
            .send_eof()
            .map_err(|e| format!("scp eof: {e}"))?;
        remote_file
            .wait_eof()
            .map_err(|e| format!("scp wait: {e}"))?;
        Ok(())
    }

    fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String> {
        let session = self.pool.get(host)?;
        let (mut remote_file, _stat) = session
            .scp_recv(remote)
            .map_err(|e| format!("scp recv: {e}"))?;
        let mut data = Vec::new();
        remote_file
            .read_to_end(&mut data)
            .map_err(|e| format!("scp read: {e}"))?;
        std::fs::write(local, data).map_err(|e| format!("write local: {e}"))?;
        Ok(())
    }
}
