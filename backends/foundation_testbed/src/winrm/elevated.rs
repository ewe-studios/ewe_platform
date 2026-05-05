//! Elevated (SYSTEM) command execution via WinRM scheduled tasks.
//!
//! Runs PowerShell scripts as SYSTEM with highest privileges by creating
//! a scheduled task, triggering it, and polling for completion via sentinel.
//!
//! Writes a heartbeat file every 3 seconds so callers can verify liveness.
//! The heartbeat is reported via the `progress` callback every ~15 seconds.

use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

use crate::config::{Result, TestbedError};

use super::WinRM;

/// Callback invoked during elevated script execution to report progress.
/// Receives a progress message string.
pub type ProgressCallback<'a> = Option<&'a dyn Fn(&str)>;

/// Run a PowerShell script as SYSTEM via a scheduled task.
///
/// Writes the script to disk via base64 encoding (avoids all escaping issues),
/// creates a scheduled task with SYSTEM principal, triggers it, and polls
/// for a unique sentinel file.
///
/// The heartbeat file (`C:\bootstrap-heartbeat.txt`) is updated every 3 seconds
/// by the script process. If a `progress` callback is provided, heartbeat status
/// is reported every ~15 seconds so callers know the script is still alive.
///
/// Returns `Err` if the script does not complete within the timeout.
pub fn run_elevated(
    winrm: &WinRM,
    ps_code: &str,
    timeout_secs: u64,
    progress: ProgressCallback<'_>,
) -> Result<()> {
    // Unique sentinel per invocation — avoids false positives from previous runs
    let sentinel_id = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let sentinel_path = format!("C:\\bootstrap-sentinel-{sentinel_id}.done");
    let heartbeat_path = "C:\\bootstrap-heartbeat.txt";
    let error_log = "C:\\bootstrap-step-error.log";
    let progress_log = "C:\\bootstrap-step-progress.log";
    let transcript_path = "C:\\bootstrap-step-transcript.log";
    let script_path = "C:\\bootstrap-step.ps1";

    debug!("run_elevated: starting (timeout={}s)", timeout_secs);

    // Clean up leftovers from previous runs
    winrm.run_ps(
        "Remove-Item -Path 'C:\\bootstrap-sentinel-*.done','C:\\bootstrap-step.ps1','C:\\bootstrap-step-error.log','C:\\bootstrap-step-transcript.log','C:\\bootstrap-step-progress.log','C:\\bootstrap-heartbeat.txt' -Force -ErrorAction SilentlyContinue",
    )?;

    // Build script: run user code, heartbeat every 3s, full transcript, capture errors, write sentinel
    let script_with_sentinel = format!(
        r#"$ErrorActionPreference = 'Continue'
$Error.Clear()

# Full transcript: capture all stdout/stderr from this script
Start-Transcript -Path '{transcript_path}' -Append -Force | Out-Null

# Heartbeat: update a file with timestamp every 3 seconds in background
$heartbeatScript = {{
    while ($true) {{
        try {{ [System.IO.File]::WriteAllText('{heartbeat_path}', (Get-Date -Format 'o'), [System.Text.Encoding]::UTF8) }} catch {{}}
        Start-Sleep -Seconds 3
    }}
}}
$heartbeatJob = Start-Job -ScriptBlock $heartbeatScript

try {{
{ps_code}
}} catch {{
    $_ | Out-File -FilePath '{error_log}' -Encoding UTF8 -Force
}}
if ($Error.Count -gt 0) {{
    $Error | Out-File -FilePath '{error_log}' -Encoding UTF8 -Append -Force
}}
Stop-Job $heartbeatJob; Remove-Job $heartbeatJob
Stop-Transcript | Out-Null
Set-Content -Path '{sentinel_path}' -Value 'done' -Encoding ASCII"#
    );

    // Write script to disk via base64
    let script_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        script_with_sentinel.as_bytes(),
    );
    let write_cmd = format!(
        "[System.IO.File]::WriteAllBytes('{script_path}', [System.Convert]::FromBase64String('{script_b64}'))"
    );
    trace!("run_elevated: writing script to VM");
    winrm.run_ps(&write_cmd)?;

    // Create scheduled task running as SYSTEM
    let task_name = format!("FoundationTestbedBootstrap_{sentinel_id}");
    trace!("run_elevated: creating scheduled task '{}'", task_name);
    let create_task = format!(
        r#"$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -ExecutionPolicy Bypass -File {script_path}'
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -DontStopOnIdleEnd
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
Register-ScheduledTask -TaskName '{task_name}' -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null"#
    );
    winrm.run_ps(&create_task)?;

    // Start the task immediately (ignores the scheduled time)
    winrm.run_ps(&format!("Start-ScheduledTask -TaskName '{task_name}'"))?;

    // Poll for unique sentinel file, checking heartbeat for liveness
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut last_heartbeat = Instant::now();
    let mut poll_count: u64 = 0;
    let mut last_error_log_size: u64 = 0;
    let mut last_transcript_size: u64 = 0;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(5));
        poll_count += 1;

        // Poll bootstrap-step-transcript.log — streams ALL stdout/stderr from the VM
        if let Ok(tlog) = winrm.run_ps_quiet(&format!(
            "if (Test-Path '{transcript_path}') {{ [System.Convert]::ToBase64String([System.IO.File]::ReadAllBytes('{transcript_path}')) }} else {{ '' }}"
        ))
            && !tlog.stdout.trim().is_empty()
        {
            if let Ok(bytes) = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                tlog.stdout.trim(),
            ) {
                let transcript_text = String::from_utf8_lossy(&bytes).to_string();
                let current_size = transcript_text.len() as u64;
                if current_size > last_transcript_size {
                    let new_content = &transcript_text[last_transcript_size as usize..];
                    for line in new_content.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            debug!("[elevated] transcript: {}", trimmed);
                            if let Some(cb) = progress {
                                cb(&format!("vm: {trimmed}"));
                            }
                        }
                    }
                    last_transcript_size = current_size;
                }
            }
        }

        // Poll bootstrap-step-error.log for new content
        if poll_count % 3 == 0 {
            if let Ok(elog) = winrm.run_ps_quiet(&format!(
                "if (Test-Path '{error_log}') {{ [System.Convert]::ToBase64String([System.IO.File]::ReadAllBytes('{error_log}')) }} else {{ '' }}"
            ))
                && !elog.stdout.trim().is_empty()
            {
                if let Ok(bytes) = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    elog.stdout.trim(),
                ) {
                    let log_text = String::from_utf8_lossy(&bytes).to_string();
                    let current_size = log_text.len() as u64;
                    if current_size > last_error_log_size {
                        // New error content appeared — report it
                        let new_content = &log_text[last_error_log_size as usize..];
                        for line in new_content.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                warn!("[elevated] error log: {}", trimmed);
                                if let Some(cb) = progress {
                                    cb(&format!("error: {trimmed}"));
                                }
                            }
                        }
                        last_error_log_size = current_size;
                    }
                }
            }
        }

        // Check sentinel — use quiet (15s timeout) to avoid blocking on transient WinRM outages
        let sentinel = winrm.run_ps_quiet(
            &format!("if (Test-Path '{sentinel_path}') {{ 'DONE' }} else {{ 'PENDING' }}"),
        );
        if let Ok(result) = sentinel
            && result.stdout.trim() == "DONE"
        {
            debug!("run_elevated: script completed after {:?}", Instant::now() - (deadline - Duration::from_secs(timeout_secs)));
            // Log any errors from the script
            let error_check = winrm.run_ps_quiet(
                &format!("if (Test-Path '{error_log}') {{ Get-Content '{error_log}' -Raw }} else {{ '' }}"),
            );
            if let Ok(ref e) = error_check
                && !e.stdout.trim().is_empty()
            {
                warn!("run_elevated: script completed with errors:\n{}", e.stdout);
            }

            // Clean up
            winrm.run_ps_quiet(&format!("Remove-Item -Path '{sentinel_path}' -Force -ErrorAction SilentlyContinue")).ok();
            winrm.run_ps_quiet(&format!("Remove-Item -Path '{heartbeat_path}' -Force -ErrorAction SilentlyContinue")).ok();
            winrm.run_ps_quiet(&format!("Remove-Item -Path '{transcript_path}' -Force -ErrorAction SilentlyContinue")).ok();
            winrm.run_ps_quiet(&format!("Remove-Item -Path '{progress_log}' -Force -ErrorAction SilentlyContinue")).ok();
            winrm.run_ps_quiet(&format!("Unregister-ScheduledTask -TaskName '{task_name}' -Confirm:$false")).ok();
            return Ok(());
        }

        // Check heartbeat — report via progress callback every 15 seconds
        let hb = winrm.run_ps_quiet(
            &format!("if (Test-Path '{heartbeat_path}') {{ Get-Content '{heartbeat_path}' }} else {{ '' }}"),
        );
        if let Ok(ref h) = hb
            && !h.stdout.trim().is_empty()
        {
            last_heartbeat = Instant::now();
        }

        // Report progress every 15 seconds (3 polls)
        if poll_count % 3 == 0 {
            let hb_age = last_heartbeat.elapsed().as_secs();
            let msg = if hb_age < 15 {
                format!("♥ alive ({hb_age}s since heartbeat)")
            } else {
                format!("⚠ heartbeat stale ({hb_age}s ago)")
            };
            if let Some(cb) = progress {
                cb(&msg);
            } else {
                debug!("[elevated] {msg}");
            }
        }
    }

    // Timeout: try to read error log and transcript and clean up
    warn!("run_elevated: script timed out after {}s", timeout_secs);
    let error_log_content = winrm.run_ps_quiet(
        &format!("if (Test-Path '{error_log}') {{ Get-Content '{error_log}' -Raw }} else {{ '' }}"),
    );
    let diag = if let Ok(ref e) = error_log_content
        && !e.stdout.trim().is_empty()
    {
        format!("\nError log: {}", e.stdout.trim().chars().take(500).collect::<String>())
    } else if let Ok(t) = winrm.run_ps_quiet(
        &format!("if (Test-Path '{transcript_path}') {{ Get-Content '{transcript_path}' -Tail 50 -Raw }} else {{ '' }}"),
    )
        && !t.stdout.trim().is_empty()
    {
        format!("\nTranscript (tail): {}", t.stdout.trim().chars().take(500).collect::<String>())
    } else {
        // Check task status for clues
        let task_info = winrm.run_ps_quiet(
            &format!("(Get-ScheduledTaskInfo -TaskName '{task_name}' -ErrorAction SilentlyContinue) | Select-Object -ExpandProperty LastRunResult"),
        );
        if let Ok(ref t) = task_info
            && !t.stdout.trim().is_empty()
        {
            format!("\nScheduled task last run result: {}", t.stdout.trim())
        } else {
            String::new()
        }
    };
    winrm.run_ps_quiet(&format!("Unregister-ScheduledTask -TaskName '{task_name}' -Confirm:$false")).ok();
    Err(TestbedError::BootstrapFailed {
        step: "elevated script".to_string(),
        message: format!("script did not complete within {timeout_secs}s timeout.{diag}"),
    })
}
