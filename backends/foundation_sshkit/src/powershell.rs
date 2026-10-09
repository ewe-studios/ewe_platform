//! PowerShell command execution over SSH — UTF-16LE + Base64 encoding with
//! CLIXML envelope stripping.
//!
//! WHY: PowerShell over non-interactive SSH folds info/progress streams onto
//! stdout as a CLIXML envelope (`#< CLIXML`). Callers need clean output.
//!
//! WHAT: [`ps_exec`] encodes a script as UTF-16LE + Base64, executes it via
//! `powershell -NoProfile -EncodedCommand`, and strips the CLIXML noise.

use std::io::Read;

/// Execute a PowerShell script over an authenticated ssh2 session.
///
/// Returns `(stdout, exit_code)`. The CLIXML envelope (`#< CLIXML ...`) is
/// stripped from stdout before returning.
///
/// # Errors
///
/// Returns `Err(message)` if the channel session fails to open or the exec
/// command fails.
pub fn ps_exec(session: &ssh2::Session, script: &str) -> Result<(String, i32), String> {
    // Encode as UTF-16LE + Base64 for -EncodedCommand
    let utf16le: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &utf16le,
    );

    let cmd = format!("powershell -NoProfile -EncodedCommand {encoded}");
    let mut channel = session
        .channel_session()
        .map_err(|e| format!("channel: {e}"))?;
    channel.exec(&cmd).map_err(|e| format!("exec: {e}"))?;

    let mut stdout = String::new();
    channel
        .read_to_string(&mut stdout)
        .map_err(|e| format!("read stdout: {e}"))?;

    let exit_code = channel.exit_status().unwrap_or(-1);

    // Strip CLIXML envelope
    if let Some(idx) = stdout.find("#< CLIXML") {
        stdout.truncate(idx);
    }

    Ok((stdout, exit_code))
}
