//! Elevated (SYSTEM) command execution via WinRM scheduled tasks.
//!
//! Runs PowerShell scripts as SYSTEM with highest privileges by creating
//! a scheduled task, triggering it, and polling for completion.

use std::thread;
use std::time::{Duration, Instant};

use crate::config::Result;

use super::WinRM;

/// Run a PowerShell script as SYSTEM via a scheduled task.
///
/// Writes the script to `C:\bootstrap-step.ps1`, creates a scheduled
/// task with SYSTEM principal + highest run level, triggers it, and
/// polls for completion via sentinel file.
///
/// Returns `true` if the task completed within the timeout.
pub fn run_elevated(winrm: &WinRM, ps_code: &str, timeout_secs: u64) -> Result<bool> {
    // Step 1: Write script to disk on the guest
    let escaped = ps_code.replace("'", "''");
    let write_script = format!(
        r#"Set-Content -Path 'C:\bootstrap-step.ps1' -Value @'
{escaped}
'@ -Encoding UTF8"#
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

    // Step 4: Poll for completion
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(2));

        // Check sentinel file
        let sentinel_result = winrm.run_ps(
            r#"if (Test-Path 'C:\bootstrap-step-done.txt') { 'DONE' } else { 'PENDING' }"#,
        );
        if let Ok(result) = sentinel_result
            && result.stdout.trim() == "DONE" {
                // Clean up
                cleanup_task(winrm, task_name).ok();
                cleanup_script(winrm).ok();
                return Ok(true);
            }

        // Fallback: check task state
        let task_state = winrm.run_ps(&format!(
            r#"(Get-ScheduledTask -TaskName '{task_name}').State"#,
        ));
        if let Ok(result) = task_state {
            let state = result.stdout.trim();
            if state == "Ready" {
                // Task finished, check exit code
                let exit_result = winrm.run_ps(
                    r#"$log = Get-WinEvent -LogName 'Microsoft-Windows-TaskScheduler/Operational' -MaxEvents 100 | Where-Object {{ $_.Id -eq 201 }} | Select-Object -First 1; if ($log) {{ 'COMPLETE' }} else {{ 'UNKNOWN' }}"#,
                );
                if let Ok(exit) = exit_result
                    && exit.stdout.trim() == "COMPLETE" {
                        cleanup_task(winrm, task_name).ok();
                        cleanup_script(winrm).ok();
                        return Ok(true);
                    }
            }
        }
    }

    // Timeout: try to clean up
    cleanup_task(winrm, task_name).ok();
    Ok(false)
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
    fn test_escaped_quotes() {
        let ps = "Write-Output 'hello'";
        let escaped = ps.replace("'", "''");
        assert_eq!(escaped, "Write-Output ''hello''");
    }
}
