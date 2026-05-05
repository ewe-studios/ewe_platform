//! Elevated (SYSTEM) command execution via WinRM scheduled tasks.
//!
//! Runs PowerShell scripts as SYSTEM with highest privileges by creating
//! a scheduled task, triggering it, and polling for completion.

use std::thread;
use std::time::{Duration, Instant};

use crate::config::{Result, TestbedError};

use super::WinRM;

/// Run a PowerShell script as SYSTEM via a scheduled task.
///
/// Writes the script to `C:\bootstrap-step.ps1` (with sentinel append),
/// creates a scheduled task with SYSTEM principal + highest run level,
/// triggers it, and polls for completion via sentinel file.
///
/// Returns `true` if the task completed within the timeout.
pub fn run_elevated(winrm: &WinRM, ps_code: &str, timeout_secs: u64) -> Result<()> {
    // Step 1: Write script to disk on the guest via base64 (avoids all escaping issues)
    let script_with_sentinel = format!(
        "{ps_code}\nSet-Content -Path 'C:\\bootstrap-step-done.txt' -Value 'done' -Encoding ASCII"
    );
    let script_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        script_with_sentinel.as_bytes(),
    );
    let write_script = format!(
        "[System.IO.File]::WriteAllBytes('C:\\bootstrap-step.ps1', [System.Convert]::FromBase64String('{script_b64}'))"
    );
    winrm.run_ps(&write_script)?;

    // Step 2: Create scheduled task running as SYSTEM
    let task_name = "FoundationTestbedBootstrap";
    let create_task = format!(
        r#"$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -ExecutionPolicy Bypass -File C:\bootstrap-step.ps1'
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddSeconds(1)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
Register-ScheduledTask -TaskName '{task_name}' -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null"#
    );
    winrm.run_ps(&create_task)?;

    // Step 3: Start the task
    winrm.run_ps(&format!("Start-ScheduledTask -TaskName '{task_name}'"))?;

    // Step 4: Poll for sentinel file
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(5));

        let sentinel = winrm.run_ps(
            r#"if (Test-Path 'C:\bootstrap-step-done.txt') { 'DONE' } else { 'PENDING' }"#,
        );
        if let Ok(result) = sentinel
            && result.stdout.trim() == "DONE"
        {
            cleanup_task(winrm, task_name).ok();
            cleanup_script(winrm).ok();
            return Ok(());
        }
    }

    // Timeout: try to clean up
    cleanup_task(winrm, task_name).ok();
    Err(TestbedError::BootstrapFailed {
        step: "elevated script".to_string(),
        message: format!("script did not complete within {timeout_secs}s timeout"),
    })
}

/// Delete the scheduled task.
fn cleanup_task(winrm: &WinRM, task_name: &str) -> Result<()> {
    winrm.run_ps(&format!(
        r#"Unregister-ScheduledTask -TaskName '{task_name}' -Confirm:$false"#,
    ))?;
    Ok(())
}

/// Remove the script file and sentinel.
fn cleanup_script(winrm: &WinRM) -> Result<()> {
    winrm.run_ps(
        r#"Remove-Item -Path 'C:\bootstrap-step.ps1' -Force -ErrorAction SilentlyContinue; Remove-Item -Path 'C:\bootstrap-step-done.txt' -Force -ErrorAction SilentlyContinue"#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_base64_roundtrip() {
        let original = "Write-Output 'hello world'";
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            original.as_bytes(),
        );
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &encoded,
        ).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), original);
    }
}
