//! WinRM test subcommand — debug script write and elevated execution.
//!
//! Usage: testbed winrm-test <profile> [--script <path.ps1>] [--timeout <secs>]
//!
//! Writes and executes a test PowerShell script via the same elevated
//! execution path that bootstrap uses, making it easy to iterate on
//! script-writing without restarting the VM.

use std::path::Path;

use clap::ArgMatches;
use crate::vms::config::{get_profile};
use crate::vms::winrm::WinRM;
use crate::vms::winrm::elevated;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

pub fn cmd_winrm_test(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let timeout: u64 = args.get_one::<String>("timeout").unwrap().parse()?;
    let profile = get_profile(name).map_err(|e| Box::new(e) as BoxedError)?;

    let port = profile.winrm_port.ok_or_else(|| {
        format!("No WinRM port configured for '{}'", profile.name)
    })?;
    let winrm = WinRM::new("127.0.0.1", port, profile.user, profile.pass);

    // Verify WinRM connectivity
    println!("Connecting to WinRM on port {port}...");
    let result = winrm.run_ps("$env:COMPUTERNAME");
    match result {
        Ok(r) => println!("Connected! Host: {} (exit {})", r.stdout.trim(), r.exit_code),
        Err(e) => return Err(format!("WinRM not reachable: {e}").into()),
    }

    // Run a diagnostic: test the chunk write mechanism that elevated.rs uses
    println!("\n=== Diagnostic: chunk write mechanism ===");
    let test_content = b"Write-Output 'chunk test passed'";
    let test_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        test_content,
    );
    let test_cmd = format!(
        "[System.IO.File]::WriteAllBytes('C:\\winrm-test-chunk.bin', [System.Convert]::FromBase64String('{test_b64}')); if (Test-Path 'C:\\winrm-test-chunk.bin') {{ 'CHUNK_OK' }} else {{ 'CHUNK_FAIL' }}"
    );
    let test_result = winrm.run_ps(&test_cmd)?;
    println!("Chunk write result: stdout='{}' stderr='{}' exit={}", test_result.stdout.trim(), test_result.stderr.trim(), test_result.exit_code);

    // Read back the file content
    let readback = winrm.run_ps("if (Test-Path 'C:\\winrm-test-chunk.bin') { [System.Convert]::ToBase64String([System.IO.File]::ReadAllBytes('C:\\winrm-test-chunk.bin')) } else { 'MISSING' }")?;
    if readback.stdout.trim() != "MISSING" && readback.exit_code == 0 {
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            readback.stdout.trim(),
        );
        if let Ok(bytes) = decoded {
            println!("Readback content: {}", String::from_utf8_lossy(&bytes));
        }
    }
    winrm.run_ps("Remove-Item 'C:\\winrm-test-chunk.bin' -Force -ErrorAction SilentlyContinue").ok();

    // Now test with the actual elevated script content
    let test_elevated_script = r#"
Write-Output "Testing elevated script write"
Set-Content -Path 'C:\winrm-test-elevated.txt' -Value 'elevated worked' -Encoding UTF8
Write-Output "File created"
"#;
    println!("\n=== Diagnostic: elevated script write ({} bytes) ===", test_elevated_script.len());

    // Manually do what run_elevated does: base64 encode, write chunk, verify
    let script_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        test_elevated_script.as_bytes(),
    );
    println!("Base64 length: {} bytes", script_b64.len());

    let write_cmd = format!(
        "[System.IO.File]::WriteAllBytes('C:\\winrm-test-script.bin', [System.Convert]::FromBase64String('{script_b64}')); if (Test-Path 'C:\\winrm-test-script.bin') {{ 'WRITE_OK' }} else {{ 'WRITE_FAIL' }}"
    );
    println!("Write command length: {} bytes", write_cmd.len());

    let write_result = winrm.run_ps(&write_cmd)?;
    println!("Write result: stdout='{}' stderr='{}' exit={}", write_result.stdout.trim(), write_result.stderr.trim(), write_result.exit_code);

    // Decode on VM and check
    if write_result.stdout.contains("WRITE_OK") {
        let file_check = winrm.run_ps("$f = Get-Item 'C:\\winrm-test-script.bin'; '{0} bytes' -f $f.Length")?;
        println!("File size on VM: {}", file_check.stdout.trim());

        // Read back and compare
        let readback = winrm.run_ps("[System.Convert]::ToBase64String([System.IO.File]::ReadAllBytes('C:\\winrm-test-script.bin'))")?;
        if readback.exit_code == 0 {
            let rb_b64 = readback.stdout.trim();
            if rb_b64 == script_b64 {
                println!("Round-trip verification: MATCH");
            } else {
                println!("Round-trip MISMATCH! sent {} b64, got {} b64", script_b64.len(), rb_b64.len());
                if rb_b64.len() < 200 {
                    println!("Readback b64: {rb_b64}");
                }
            }
        }
    }

    winrm.run_ps("Remove-Item 'C:\\winrm-test-script.bin','C:\\winrm-test-elevated.txt' -Force -ErrorAction SilentlyContinue").ok();
    // Now test the actual run_elevated path with a tiny script
    println!("\n=== Testing actual run_elevated with tiny script ===");
    let tiny_script = "Write-Output 'tiny test'";
    println!("Script: '{tiny_script}' ({} bytes)", tiny_script.len());

    // Manually replicate what run_elevated does (simplified version)
    use std::time::{SystemTime, UNIX_EPOCH};
    let sentinel_id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().to_string();
    let sentinel_path = format!("C:\\winrm-test-sentinel-{sentinel_id}.done");
    let script_path = "C:\\winrm-test-elevated.ps1";

    // Step 1: Write the script directly (no sentinel wrapper, just the raw script)
    let raw_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, tiny_script.as_bytes());
    let write_cmd = format!(
        "[System.IO.File]::WriteAllBytes('{script_path}', [System.Convert]::FromBase64String('{raw_b64}'))"
    );
    println!("Write command: {} bytes", write_cmd.len());
    let wr = winrm.run_ps(&write_cmd)?;
    println!("Write result: exit={} stdout='{}'", wr.exit_code, wr.stdout.lines().last().unwrap_or("").trim());

    // Step 2: Verify
    let vr = winrm.run_ps(&format!("if (Test-Path '{script_path}') {{ 'OK' }} else {{ 'MISSING' }}"))?;
    println!("Verify: {}", vr.stdout.lines().find(|l| l.contains("OK") || l.contains("MISSING")).unwrap_or("unknown").trim());

    // Step 3: Try running it
    if vr.stdout.contains("OK") {
        let run_cmd = format!("powershell -NoProfile -ExecutionPolicy Bypass -File {script_path}");
        println!("Running via: {run_cmd}");
        let rr = winrm.run_ps(&run_cmd)?;
        println!("Run result: exit={} stdout='{}' stderr='{}'", rr.exit_code, rr.stdout.trim(), rr.stderr.trim());
    }

    // Cleanup
    winrm.run_ps(&format!("Remove-Item '{script_path}','{sentinel_path}' -Force -ErrorAction SilentlyContinue")).ok();

    println!("\n=== Diagnostic complete ===\n");

    if let Some(script_path) = args.get_one::<String>("script") {
        // Load and execute a local .ps1 file via elevated execution
        let ps_code = std::fs::read_to_string(script_path)
            .map_err(|e| format!("Reading {script_path}: {e}"))?;
        println!("\nExecuting {} via elevated run_elevated (timeout={timeout}s)...", Path::new(script_path).display());
        println!("Script size: {} bytes\n", ps_code.len());

        let progress = |msg: &str| println!("  [progress] {msg}");
        elevated::run_elevated(&winrm, &ps_code, timeout, Some(&progress))?;
        println!("\nScript completed successfully.");
    } else {
        // Run a built-in test script that exercises the write + execute path
        let test_script = r#"
Write-Output "Hello from elevated WinRM!"
Write-Output "Testing file write..."
Set-Content -Path 'C:\winrm-test-output.txt' -Value 'test successful' -Encoding UTF8
Write-Output "File written to C:\winrm-test-output.txt"
if (Test-Path 'C:\winrm-test-output.txt') {
    Write-Output "File verified: $(Get-Content C:\winrm-test-output.txt)"
} else {
    throw "File was not created!"
}
Remove-Item 'C:\winrm-test-output.txt' -Force
Write-Output "Cleanup done. All good."
"#;
        println!("\nRunning built-in test script via elevated run_elevated...");
        let progress = |msg: &str| println!("  [progress] {msg}");
        elevated::run_elevated(&winrm, test_script, timeout, Some(&progress))?;
        println!("\nTest script completed successfully.");
    }

    Ok(())
}
