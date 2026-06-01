//! Playwright browser test runner.
//!
//! WHY: Browser tests need a real browser environment with DOM and WebAssembly support.
//! WHAT: Generates a Playwright Node.js script, runs it, captures results.
//! HOW: Creates a temp dir for the Playwright script, installs playwright via npm,
//!      generates a test script tailored to the target URL/browser/headless mode.

use std::process::Command;

use tracing::{debug, info};

use crate::cli::Browser;
use crate::error::{Result, ToTrace, WasmTestbedError};

/// Output from a browser test run.
pub struct BrowserOutput {
    pub test_result: String,
    pub console_logs: Vec<String>,
    pub exit_code: i32,
}

/// Run a browser test via Playwright.
pub fn run(url: &str, browser: &Browser, headless: bool) -> Result<BrowserOutput> {
    which::which("node").map_err(|_| WasmTestbedError::NodeNotFound.trace())?;

    info!("Running browser test: {url} (headless={headless})");

    let temp_dir = tempfile::tempdir()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;
    let script_path = temp_dir.path().join("run-test.js");

    let script = generate_playwright_script(url, browser, headless);
    std::fs::write(&script_path, &script)
        .map_err(|e| WasmTestbedError::Io(e).trace())?;
    debug!("Wrote playwright script to {}", script_path.display());

    info!("Installing playwright (first run may take a moment)...");

    let status = Command::new("npm")
        .arg("init")
        .arg("-y")
        .current_dir(temp_dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;
    if !status.success() {
        return Err(WasmTestbedError::NpmInitFailed.trace());
    }

    let status = Command::new("npm")
        .arg("install")
        .arg("playwright")
        .current_dir(temp_dir.path())
        .status()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;
    if !status.success() {
        return Err(WasmTestbedError::PlaywrightInstallFailed.trace());
    }

    let browser_name = match browser {
        Browser::Chrome => "chromium",
        Browser::Firefox => "firefox",
        Browser::Safari => "webkit",
    };

    debug!("Installing playwright browser: {browser_name}");
    let status = Command::new("npx")
        .arg("playwright")
        .arg("install")
        .arg(browser_name)
        .current_dir(temp_dir.path())
        .status()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;
    if !status.success() {
        return Err(WasmTestbedError::PlaywrightBrowserInstallFailed(browser_name.to_string()).trace());
    }

    let output = Command::new("node")
        .arg(&script_path)
        .current_dir(temp_dir.path())
        .output()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    debug!("Playwright stdout:\n{stdout}");
    if !stderr.is_empty() {
        debug!("Playwright stderr:\n{stderr}");
    }

    let test_result = stdout.lines().next().unwrap_or("").trim().to_string();

    let mut console_logs = Vec::new();
    for line in stderr.lines() {
        if let Ok(logs) = serde_json::from_str::<Vec<serde_json::Value>>(line) {
            for log in logs {
                if let Some(text) = log.get("text").and_then(|v| v.as_str()) {
                    console_logs.push(text.to_string());
                }
            }
        }
    }

    if exit_code != 0 {
        return Err(WasmTestbedError::BrowserTestFailed(exit_code, test_result).trace());
    }

    Ok(BrowserOutput {
        test_result,
        console_logs,
        exit_code,
    })
}

/// Generate the Playwright test script content.
fn generate_playwright_script(url: &str, browser: &Browser, headless: bool) -> String {
    let browser_module = match browser {
        Browser::Chrome => "chromium",
        Browser::Firefox => "firefox",
        Browser::Safari => "webkit",
    };

    format!(
        r#"
const {{ {browser_module} }} = require('playwright');

(async () => {{
  const browser = await {browser_module}.launch({{ headless: {headless} }});
  const page = await browser.newPage();
  const logs = [];

  page.on('console', msg => logs.push({{ type: msg.type(), text: msg.text() }}));

  try {{
    await page.goto('{url}', {{ waitUntil: 'domcontentloaded', timeout: 30000 }});

    let output = null;
    for (let i = 0; i < 60; i++) {{
      try {{
        output = await page.$eval('#output', el => el.textContent);
      }} catch (e) {{}}
      if (output && (output.includes('test result:') || output.includes('Tests complete'))) break;
      await new Promise(r => setTimeout(r, 500));
    }}

    if (!output) {{
      console.error('TIMEOUT: test did not produce results within 30 seconds');
      process.exit(1);
    }}

    console.log(output);
  }} finally {{
    await browser.close();
  }}
}})();
"#,
    )
}
