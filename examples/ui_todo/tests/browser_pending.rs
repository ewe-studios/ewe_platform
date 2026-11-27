//! Verify concurrent browser persistence and legacy import with two real tabs.
#[path = "support/browser_server.rs"]
mod browser_server;
use browser_server::{open_document, with_browser_server};
use foundation_browser::BrowserDriver;
#[path = "browser_countdown.rs"]
mod browser_countdown;

#[test]
fn pending_changes_survive_concurrent_tabs_and_confirm_independently() {
    with_browser_server(|driver, url, _| verify_pending_tabs(driver, url));
}

#[test]
fn legacy_import_keeps_receipts_and_recovers_late_writes() {
    with_browser_server(|driver, url, _| {
        let page = driver.new_page().unwrap();
        open_document(&page, &format!("{url}theme.css"));
        assert_eq!(
            page.eval(include_str!("browser_pending_migration.js"))
                .unwrap(),
            "ok"
        );
    });
}

/// Two live applications retain both operations when their first requests fail.
fn verify_pending_tabs(driver: &BrowserDriver<'_>, url: &str) {
    let left = driver.new_page().unwrap();
    let right = driver.new_page().unwrap();
    let tabs = [&left, &right];
    let open = r#"button[primal\:onclick="host:open_add_dialog"]:not([hidden] *)"#;
    let retry = r#"button[primal\:onclick="host:retry_todo"]:not([hidden] *)"#;
    for (tab, title) in tabs.iter().zip(["Left tab", "Right tab"]) {
        open_document(tab, url);
        tab.locator(open).expect().to_be_visible().unwrap();
        let selector = serde_json::json!(open);
        tab.eval(&format!("document.querySelector({selector}).click()"))
            .unwrap();
        let input = tab.locator("dialog:modal input");
        input.expect().to_be_visible().unwrap();
        input.fill(title).unwrap();
        tab.eval(r#"{
            const fetch = window.fetch.bind(window);
            window.fetch = (url, opts) => new URL(url, location.href).pathname === "/action"
                ? Promise.reject(Error("Offline")) : fetch(url, opts);
            window.submitTogether = new BroadcastChannel("todo-storage-test");
            submitTogether.onmessage = () => document.querySelector("dialog:modal .ewe-submit-button").click();
        }"#).unwrap();
    }
    left.eval("new BroadcastChannel('todo-storage-test').postMessage('submit')")
        .unwrap();
    let mut ids = Vec::new();
    for tab in tabs {
        tab.locator(retry).expect().to_have_count(1).unwrap();
        ids.push(
            tab.eval(r#"document.querySelector('button[value^="add:"]').value"#)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    assert_ne!(ids[0], ids[1]);
    let first = format!(r#"button[value="{}"]"#, ids[0]);
    let second = format!(r#"button[value="{}"]"#, ids[1]);
    for tab in tabs {
        reload_todos(tab);
        tab.locator(&first).expect().to_be_visible().unwrap();
        tab.locator(&second).expect().to_be_visible().unwrap();
    }
    left.locator(&first).click().unwrap();
    left.locator(&first).expect().to_have_count(0).unwrap();
    right.locator(&first).expect().to_have_count(0).unwrap();
    right.locator(&second).expect().to_be_visible().unwrap();
    right.locator(retry).expect().to_have_count(1).unwrap();
    right.locator(&second).click().unwrap();
    right.locator(retry).expect().to_have_count(0).unwrap();
    for tab in tabs {
        for id in &ids {
            let id = id.strip_prefix("add:").unwrap();
            tab.locator(&format!(
                r#"button[value="{id}"][primal\:onclick="server:toggle_todo"]"#
            ))
            .expect()
            .to_be_visible()
            .unwrap();
        }
        tab.locator(retry).expect().to_have_count(0).unwrap();
        open_document(tab, "about:blank");
    }
}

/// Wait for the new application view after the browser reloads the current document.
fn reload_todos(page: &foundation_browser::Page<'_>) {
    page.eval("document.documentElement.dataset.reloading = 'yes'; location.reload()")
        .unwrap();
    page.locator("html:not([data-reloading]) #open-count")
        .expect()
        .timeout(foundation_browser::LaunchConfig::default().navigate_timeout)
        .to_have_count(1)
        .unwrap_or_else(|error| panic!("Reload: {error:?}, {:?}",page.eval("[location.href, document.readyState, document.querySelector('#notice')?.value, window.todoReady, performance.getEntriesByType('resource').map(r => r.name)]")));
}
