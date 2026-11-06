//! `WinRM` SOAP client for Windows guest VMs.
//!
//! Pure Rust implementation — no Python/pywinrm dependency.
//! HTTP Basic auth over plain HTTP (localhost only, port 5985).
//!
//! Shell lifecycle: Create → Execute → Receive (loop) → Delete.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, trace};

use foundation_core::valtron::Clock;

use crate::config::{Result, TestbedError, VmProfile};

pub mod elevated;

/// Result of a `WinRM` command execution.
#[derive(Debug, Clone)]
pub struct CmdResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// `WinRM` client for a specific host/port/credentials.
pub struct WinRM {
    host: String,
    port: u16,
    user: String,
    pass: String,
    /// The host's clock. Request IDs and the receive deadline read it, so a
    /// test can control both (AGENTS rule 11).
    clock: Arc<dyn Clock>,
}

/// Distinguishes two request IDs minted in the same wall millisecond.
static MESSAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl WinRM {
    /// Create a new `WinRM` client.
    #[must_use]
    pub fn new(host: &str, port: u16, user: &str, pass: &str, clock: Arc<dyn Clock>) -> Self {
        Self {
            host: host.to_string(),
            port,
            user: user.to_string(),
            pass: pass.to_string(),
            clock,
        }
    }

    /// The injected clock that this client reads.
    #[must_use]
    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }

    /// The injected clock, as a shared handle for a client built from this one.
    #[must_use]
    pub fn clock_arc(&self) -> Arc<dyn Clock> {
        Arc::clone(&self.clock)
    }

    /// Create from a VM profile.
    ///
    /// # Errors
    ///
    /// Returns an error if the profile has no `WinRM` port.
    pub fn from_profile(profile: &VmProfile, clock: Arc<dyn Clock>) -> Result<Self> {
        let winrm_port = profile
            .winrm_port
            .ok_or_else(|| TestbedError::WinrmNotReachable { port: 0 })?;
        Ok(Self::new(
            "127.0.0.1",
            winrm_port,
            profile.user,
            profile.pass,
            clock,
        ))
    }

    /// Check if `WinRM` is reachable and can create a shell (real probe).
    ///
    /// Creates and immediately deletes a `WinRM` shell to verify the full
    /// SOAP pipeline works, not just that the TCP port is open.
    #[must_use]
    pub fn ping(&self) -> bool {
        self.shell_create()
            .map(|shell_id| {
                self.shell_delete(&shell_id, None);
            })
            .is_ok()
    }

    /// Run a PowerShell script via `WinRM` shell.
    ///
    /// Encodes script as UTF-16LE + Base64 for `powershell -EncodedCommand`.
    ///
    /// # Errors
    ///
    /// Returns an error if a `WinRM` shell operation fails.
    pub fn run_ps(&self, script: &str) -> Result<CmdResult> {
        // Encode script as UTF-16LE + Base64
        let utf16le: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &utf16le);

        let ps_command = format!("powershell -EncodedCommand {encoded}");

        debug!("run_ps: creating shell for command");
        // WinRM shell lifecycle
        let shell_id = self.shell_create()?;
        let command_id = self.shell_exec(&shell_id, &ps_command)?;
        debug!("run_ps: receiving output (unlimited wait)");
        let result = self.shell_receive(&shell_id, &command_id, 0)?;
        debug!("run_ps: command complete, exit_code={}", result.exit_code);
        self.shell_delete(&shell_id, Some(&command_id));

        Ok(result)
    }

    // ── WinRM SOAP protocol ──────────────────────────────────────────────

    /// Create a `WinRM` shell.
    fn shell_create(&self) -> Result<String> {
        let msg_id = uuid_simple(self.clock());
        let body = format!(
            r#"<env:Envelope xmlns:env="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:w="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd" xmlns:p="http://schemas.microsoft.com/wbem/wsman/1/windows/shell"><env:Header><a:Action>http://schemas.xmlsoap.org/ws/2004/09/transfer/Create</a:Action><a:MessageID>uuid:{}</a:MessageID><a:To>http://{}:{}/wsman</a:To><a:ReplyTo><a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address></a:ReplyTo><w:ResourceURI s="true">http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</w:ResourceURI><w:MaxEnvelopeSize>153600</w:MaxEnvelopeSize><w:OperationTimeout>PT60S</w:OperationTimeout><w:OptionSet><w:Option Name="WINRS_NOPROFILE">FALSE</w:Option><w:Option Name="WINRS_CODEPAGE">437</w:Option></w:OptionSet></env:Header><env:Body><p:Shell><p:InputStreams>stdin</p:InputStreams><p:OutputStreams>stdout stderr</p:OutputStreams></p:Shell></env:Body></env:Envelope>"#,
            msg_id, self.host, self.port
        );

        let response = self.send_soap(
            "http://schemas.xmlsoap.org/ws/2004/09/transfer/Create",
            &body,
        )?;
        // Extract ShellId from response
        parse_shell_id(&response).ok_or_else(|| TestbedError::WinrmNotReachable { port: self.port })
    }

    /// Execute a command in an existing shell.
    fn shell_exec(&self, shell_id: &str, command: &str) -> Result<String> {
        let msg_id = uuid_simple(self.clock());
        let body = format!(
            r#"<env:Envelope xmlns:env="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:w="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd" xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell"><env:Header><a:Action>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Command</a:Action><a:MessageID>uuid:{}</a:MessageID><a:To>http://{}:{}/wsman</a:To><a:ReplyTo><a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address></a:ReplyTo><w:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</w:ResourceURI><w:SelectorSet><w:Selector Name="ShellId">{}</w:Selector></w:SelectorSet></env:Header><env:Body><rsp:CommandLine><rsp:Command>{}</rsp:Command></rsp:CommandLine></env:Body></env:Envelope>"#,
            msg_id, self.host, self.port, shell_id, command
        );

        let response = self.send_soap(
            "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Command",
            &body,
        )?;
        parse_command_id(&response)
            .ok_or_else(|| TestbedError::WinrmNotReachable { port: self.port })
    }

    /// Receive output from a running command.
    ///
    /// `max_wait_secs` caps total polling time. Use `0` for unlimited (legacy).
    fn shell_receive(
        &self,
        shell_id: &str,
        command_id: &str,
        max_wait_secs: u64,
    ) -> Result<CmdResult> {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        let deadline = if max_wait_secs > 0 {
            Some(self.clock.now() + std::time::Duration::from_secs(max_wait_secs))
        } else {
            None
        };

        // Poll for output — supports long-running commands (up to 1800s)
        for (iteration, _) in (0..3600).enumerate() {
            // 1800s total at 500ms intervals
            if let Some(dl) = deadline
                && self.clock.now() > dl
            {
                debug!(
                    "shell_receive: TIMEOUT after {} iterations, stdout={:?} stderr={:?}",
                    iteration, stdout, stderr
                );
                return Err(TestbedError::WinrmTimeout {
                    message: format!("shell_receive timed out after {max_wait_secs}s"),
                });
            }

            let msg_id = uuid_simple(self.clock());
            let body = format!(
                r#"<env:Envelope xmlns:env="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:w="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd" xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell"><env:Header><a:Action>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Receive</a:Action><a:MessageID>uuid:{}</a:MessageID><a:To>http://{}:{}/wsman</a:To><a:ReplyTo><a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address></a:ReplyTo><w:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</w:ResourceURI><w:SelectorSet><w:Selector Name="ShellId">{}</w:Selector></w:SelectorSet></env:Header><env:Body><rsp:Receive><rsp:DesiredStream CommandId="{}">stdout stderr</rsp:DesiredStream></rsp:Receive></env:Body></env:Envelope>"#,
                msg_id, self.host, self.port, shell_id, command_id
            );

            let response = self.send_soap(
                "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Receive",
                &body,
            )?;

            // Parse output streams and command state
            let (new_stdout, new_stderr, done, code) = parse_receive_response(&response);
            trace!(
                "shell_receive iter {}: done={} stdout={} bytes stderr={} bytes",
                iteration,
                done,
                new_stdout.len(),
                new_stderr.len()
            );
            stdout.push_str(&new_stdout);
            stderr.push_str(&new_stderr);

            if done {
                exit_code = code;
                trace!(
                    "shell_receive: command completed after {} iterations",
                    iteration
                );
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        Ok(CmdResult {
            stdout,
            stderr,
            exit_code,
        })
    }

    /// Run a quick `WinRM` command (sentinel/heartbeat checks).
    ///
    /// Uses a 15-second receive timeout to avoid blocking when `WinRM` is transiently
    /// unavailable during long elevated operations like OpenSSH install.
    ///
    /// # Errors
    ///
    /// Returns an error if a `WinRM` shell operation fails.
    pub fn run_ps_quiet(&self, script: &str) -> Result<CmdResult> {
        let utf16le: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &utf16le);
        let ps_command = format!("powershell -EncodedCommand {encoded}");

        trace!("run_ps_quiet: creating shell");
        let shell_id = self.shell_create()?;
        trace!("run_ps_quiet: shell created, exec command");
        let command_id = self.shell_exec(&shell_id, &ps_command)?;
        trace!("run_ps_quiet: command exec done, receiving with 15s timeout");
        let result = self.shell_receive(&shell_id, &command_id, 15);
        trace!(
            "run_ps_quiet: receive result: {:?}",
            result.as_ref().map(|r| &r.stdout)
        );
        self.shell_delete(&shell_id, Some(&command_id));
        result
    }

    /// Delete a `WinRM` shell.
    fn shell_delete(&self, shell_id: &str, command_id: Option<&str>) {
        let msg_id = uuid_simple(self.clock());
        let command_selector = command_id
            .map(|id| format!("<rsp:Signal CommandId=\"{id}\"><w:Code>0</w:Code></rsp:Signal>"))
            .unwrap_or_default();

        let body = format!(
            r#"<env:Envelope xmlns:env="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:w="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd"><env:Header><a:Action>http://schemas.xmlsoap.org/ws/2004/09/transfer/Delete</a:Action><a:MessageID>uuid:{}</a:MessageID><a:To>http://{}:{}/wsman</a:To><a:ReplyTo><a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address></a:ReplyTo><w:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</w:ResourceURI><w:SelectorSet><w:Selector Name="ShellId">{}</w:Selector></w:SelectorSet></env:Header><env:Body>{}</env:Body></env:Envelope>"#,
            msg_id, self.host, self.port, shell_id, command_selector
        );

        let _ = self.send_soap(
            "http://schemas.xmlsoap.org/ws/2004/09/transfer/Delete",
            &body,
        );
    }

    /// Send a SOAP request and return the response body.
    fn send_soap(&self, action: &str, body: &str) -> Result<String> {
        let addr: SocketAddr = format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|_| TestbedError::WinrmNotReachable { port: self.port })?;
        trace!("send_soap: connecting to {} action={}", addr, action);
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
            .map_err(|_| TestbedError::WinrmNotReachable { port: self.port })?;
        let mut stream = stream;
        stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

        let auth = self.auth_header_value();

        let header = format!(
            "POST /wsman HTTP/1.1\r\n\
             Host: {}\r\n\
             SOAPAction: {action}\r\n\
             Content-Type: application/soap+xml;charset=UTF-8\r\n\
             Authorization: Basic {}\r\n\
             User-Agent: foundation_testbed/0.1\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n",
            self.host,
            auth,
            body.len()
        );

        stream
            .write_all(header.as_bytes())
            .map_err(|_e| TestbedError::WinrmNotReachable { port: self.port })?;
        stream
            .write_all(body.as_bytes())
            .map_err(|_e| TestbedError::WinrmNotReachable { port: self.port })?;

        // Read response
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|_e| TestbedError::WinrmNotReachable { port: self.port })?;

        // Extract body after HTTP headers (after \r\n\r\n)
        if let Some(idx) = response.find("\r\n\r\n") {
            Ok(response[idx + 4..].to_string())
        } else {
            Ok(response)
        }
    }

    /// Base64-encoded Basic auth value.
    #[must_use]
    pub fn auth_header_value(&self) -> String {
        let creds = format!("{}:{}", self.user, self.pass);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, creds.as_bytes())
    }
}

// ── Response parsing ─────────────────────────────────────────────────────────

/// Extract `ShellId` from a SOAP response.
#[must_use]
pub fn parse_shell_id(response: &str) -> Option<String> {
    // Look for <Selector Name="ShellId">VALUE</Selector>
    let start = response.find(r#"Name="ShellId">"#)?;
    let rest = &response[start + r#"Name="ShellId">"#.len()..];
    let end = rest.find("</")?;
    Some(rest[..end].to_string())
}

/// Extract `CommandId` from a SOAP response.
#[must_use]
pub fn parse_command_id(response: &str) -> Option<String> {
    // Look for <rsp:CommandId>VALUE</rsp:CommandId>
    let start = response.find("<rsp:CommandId>")?;
    let rest = &response[start + "<rsp:CommandId>".len()..];
    let end = rest.find("</rsp:CommandId>")?;
    Some(rest[..end].to_string())
}

/// Parse a `WinRM` Receive response for stdout, stderr, completion, and exit code.
#[must_use]
pub fn parse_receive_response(response: &str) -> (String, String, bool, i32) {
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut done = false;
    let mut exit_code = 0;

    // Extract Base64-encoded stdout streams
    for (data, is_stderr_flag) in extract_streams(response) {
        if let Ok(bytes) =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data.trim())
        {
            let text = String::from_utf8_lossy(&bytes).to_string();
            if is_stderr_flag {
                stderr.push_str(&text);
            } else {
                stdout.push_str(&text);
            }
        }
    }

    // Check if command is done
    if response
        .contains("http://schemas.microsoft.com/wbem/wsman/1/windows/shell/CommandState/Done")
        || response
            .contains("http://schemas.dmtf.org/wbem/wsman/1/wsman/secprofile/CommandState/Done")
    {
        done = true;
        // Try to extract exit code
        if let Some(start) = response.find("<rsp:ExitCode>") {
            let rest = &response[start + "<rsp:ExitCode>".len()..];
            if let Some(end) = rest.find("</rsp:ExitCode>") {
                exit_code = rest[..end].trim().parse().unwrap_or(0);
            }
        }
    }

    (stdout, stderr, done, exit_code)
}

/// Find all Base64 data chunks in Stream elements, marking stderr ones.
fn extract_streams(response: &str) -> Vec<(String, bool)> {
    let mut results = Vec::new();
    let mut search_from = 0;

    while let Some(start) = response[search_from..].find("<rsp:Stream") {
        let block_start = search_from + start;
        // Find the Name attribute to determine if this is stderr
        let block = &response[block_start..];
        let is_stderr =
            block.starts_with(r#"Name="stderr""#) || block.starts_with(r"Name='stderr'");

        if let Some(data_start) = block.find('>') {
            let content = &block[data_start + 1..];
            if let Some(data_end) = content.find("</rsp:Stream>") {
                results.push((content[..data_end].to_string(), is_stderr));
            }
        }

        search_from = block_start + 1;
    }

    results
}

/// Generate a unique message ID for `WinRM` request correlation.
#[allow(dead_code)]
#[must_use]
pub fn generate_message_id(clock: &dyn Clock) -> String {
    let uuid = uuid_simple(clock);
    format!("uuid:{uuid}")
}

/// Simple message ID generation from the injected wall clock and a counter.
///
/// Two IDs minted in the same wall millisecond stay distinct because the
/// counter never repeats within the process.
#[allow(dead_code)]
fn uuid_simple(clock: &dyn Clock) -> String {
    let millis = crate::clock::wall_now_millis(clock).unsigned_abs();
    let sequence = MESSAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{millis:016x}{sequence:016x}")
}
