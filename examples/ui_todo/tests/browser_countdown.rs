//! Exercise the countdown in normal browser tabs and through the Todo server.
use super::browser_server::{open_document, with_browser_server};
use foundation_browser::Page;
use std::time::{Duration, Instant};

const SESSION: &str = "ewe.todo.countdown-session";

fn capture(page: &Page<'_>, state: &str) {
    if let Some(output) = std::env::var_os("EWE_WEB_CAPTURES") {
        let output = std::path::PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        page.screenshot(output.join(format!("web-countdown-{state}.png")))
            .unwrap();
    }
}

fn ready(page: &Page<'_>) {
    page.eval(
        "(async () => { while (!window.todoReady) await new Promise(requestAnimationFrame); })()",
    )
    .unwrap();
}

fn reload(page: &Page<'_>) {
    page.eval("document.documentElement.dataset.reloading = 'yes'; location.reload()")
        .unwrap();
    page.locator("html:not([data-reloading]) #todo-pause-button")
        .expect()
        .to_be_visible()
        .unwrap();
    ready(page);
}

fn drop_next_reply(page: &Page<'_>) {
    page.eval(
        r#"
          (() => {
            const send = window.fetch.bind(window);
            window.fetch = async (url, options) => {
              const response = await send(url, options);
              if (url === "/action") {
                window.fetch = send;
                if (window.holdCountdownReply) {
                  await new Promise(resolve => { window.releaseCountdownReply = resolve; });
                }
                await response.body?.cancel();
                throw Error("Lost countdown reply");
              }
              return response;
            };
          })()
        "#,
    )
    .unwrap();
}

/// WHY: A lost acknowledgment, cloned session, and reload must keep one tab's input pause private.
/// WHAT: Verify duplicate-session isolation, timed release, live rows, and initiating control focus.
#[test]
fn the_pause_survives_reload_without_blocking_a_second_tab() {
    with_browser_server(|driver, url, _| {
        let left = driver.new_page().unwrap();
        open_document(&left, url);
        ready(&left);
        let original = left
            .eval(&format!("sessionStorage.getItem('{SESSION}')"))
            .unwrap();

        left.eval(
            r#"(() => {
                const send = window.fetch.bind(window);
                window.fetch = (url, options) => {
                    if (String(url) !== "/action") return send(url, options);
                    window.fetch = send;
                    return Promise.reject(Error("Drop the request before it reaches the server"));
                };
                document.querySelector('#todo-pause-button').click();
            })()"#,
        )
        .unwrap();
        let retry = left.locator("#todo-pause-control button");
        retry.expect().to_be_visible().unwrap();

        left.eval("window.holdCountdownReply = true").unwrap();
        drop_next_reply(&left);
        let started = Instant::now();
        retry.click().unwrap();
        left.locator("#todo-pause-control")
            .expect()
            .to_have_text("Pausing input…")
            .unwrap();
        capture(&left, "pending");
        left.eval(
            r#"(async () => {
            while (!window.releaseCountdownReply) await new Promise(requestAnimationFrame);
            window.holdCountdownReply = false;
            window.releaseCountdownReply();
        })()"#,
        )
        .unwrap();
        retry.expect().to_be_visible().unwrap_or_else(|error| {
            panic!("{error:?} Browser state: {:?}", left.eval(
                "[document.querySelector('#notice')?.value, document.querySelector('#todo-pause-button')?.disabled, document.querySelector('#todo-pause-control')?.innerText]"
            ))
        });
        capture(&left, "retry");
        retry.click().unwrap();
        left.locator("#todo-pause-modal:modal")
            .expect()
            .to_be_visible()
            .unwrap();

        capture(&left, "blocked");

        let stale = rmp_serde::to_vec_named(&(
            "ewe.todo.v1",
            serde_json::json!({"Countdown":{"Released":{
                "scope":{"session":original}, "id":"00000000-0000-4000-8000-000000000010"
            }}}),
        ))
        .unwrap();
        left.eval(&format!(
            "document.querySelector('#page').dispatchEvent(new CustomEvent('ewe:data',{{bubbles:true, detail:{{bytes:Uint8Array.from({stale:?})}}}}))"
        )).unwrap();

        let right = driver.new_page().unwrap();
        open_document(&right, url);
        ready(&right);
        right
            .eval(&format!("sessionStorage.setItem('{SESSION}', {original})"))
            .unwrap();
        reload(&right);
        assert_ne!(
            right
                .eval(&format!("sessionStorage.getItem('{SESSION}')"))
                .unwrap(),
            original
        );
        right
            .locator("#todo-pause-modal")
            .expect()
            .to_have_count(0)
            .unwrap();

        right
            .locator(r#"button[primal\:onclick="host:open_add_dialog"]"#)
            .click()
            .unwrap();
        let draft = right.locator("dialog:modal input");
        draft.expect().to_be_visible().unwrap_or_else(|error| {
            panic!("{error:?} Peer state: {:?}", right.eval(
                "[document.querySelector('#notice')?.value, document.activeElement?.outerHTML, document.body.innerText]"
            ))
        });
        draft.fill("Delivered during the pause").unwrap();
        right
            .locator("dialog:modal .ewe-submit-button")
            .click()
            .unwrap();
        left.locator("#todo-list .ewe-checklist-row .ewe-text")
            .expect()
            .to_have_text("Delivered during the pause")
            .unwrap();
        left.locator("#todo-pause-modal:modal")
            .expect()
            .to_be_visible()
            .unwrap();

        reload(&left);
        left.locator("#todo-pause-modal:modal")
            .expect()
            .to_be_visible()
            .unwrap();
        let title = left.locator("#todo-pause-title");
        title.press("Tab").unwrap();
        left.locator("#todo-pause-title:focus")
            .expect()
            .to_have_count(1)
            .unwrap();
        title.press("Escape").unwrap();
        left.locator("#todo-pause-modal:modal")
            .expect()
            .to_be_visible()
            .unwrap();
        drop_next_reply(&left);
        left.locator("#todo-pause-note")
            .expect()
            .timeout(Duration::from_secs(65))
            .to_have_text("Waiting for the server to confirm the release.")
            .unwrap();
        left.locator("#todo-pause-modal")
            .expect()
            .timeout(Duration::from_secs(65))
            .to_have_count(0)
            .unwrap();
        assert!(started.elapsed() >= Duration::from_secs(59));
        left.locator("#todo-pause-button:focus")
            .expect()
            .to_have_count(1)
            .unwrap();
        assert_eq!(
            left.eval("document.querySelector('#notice').value")
                .unwrap(),
            ""
        );
        capture(&left, "resumed");
        assert_eq!(
            left.eval("document.querySelector('#pause-announcement').textContent")
                .unwrap(),
            "Input resumed"
        );
    });
}
