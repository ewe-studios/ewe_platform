//! A local service controls the order in which two Chromium tabs finish loading.

#![cfg(feature = "browser-tests")]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_browser::{BrowserDriver, BrowserError, LaunchConfig};
use foundation_context::Ctx;

fn reply(stream: &mut TcpStream, content_type: &str, body: &str) {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    )
    .expect("write the fixture response");
}

struct ScratchService {
    address: SocketAddr,
    requests: mpsc::Receiver<(String, TcpStream)>,
    shutdown: mpsc::Sender<()>,
    service: Option<JoinHandle<()>>,
}

impl ScratchService {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the fixture service");
        let address = listener.local_addr().unwrap();
        let (requested, requests) = mpsc::channel();
        let (shutdown, stop) = mpsc::channel();
        let service = std::thread::spawn(move || {
            for stream in listener.incoming().map(Result::unwrap) {
                if stop.try_recv().is_ok() {
                    break;
                }
                let requested = requested.clone();
                std::thread::spawn(move || serve(stream, &requested));
            }
        });
        Self {
            address,
            requests,
            shutdown,
            service: Some(service),
        }
    }

    fn receive_request(&self) -> (String, TcpStream) {
        self.requests
            .recv_timeout(Duration::from_secs(15))
            .expect("the browser requests a fixture path")
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
}

impl Drop for ScratchService {
    fn drop(&mut self) {
        if self.shutdown.send(()).is_ok() && TcpStream::connect(self.address).is_ok() {
            self.service.take().unwrap().join().unwrap();
        }
    }
}

#[test]
fn a_sibling_tab_cannot_finish_a_pending_document() {
    let service = ScratchService::start();
    let driver = BrowserDriver::launch(
        LaunchConfig::chromium()
            .ctx(Ctx::host())
            .navigate_timeout(Duration::from_secs(5)),
    )
    .expect("launch Chromium");
    let peer = driver.new_page().expect("attach peer tab");
    let result = std::thread::scope(|scope| {
        let driver = &driver;
        let slow_url = service.url("/slow");
        let slow = scope.spawn(move || driver.new_page().unwrap().goto(&slow_url));
        let (path, gate) = service.receive_request();
        assert_eq!(path, "/gate");
        peer.goto(&service.url("/peer")).expect("load the peer tab");
        assert_eq!(peer.eval("document.title").unwrap(), "Peer");

        let result = slow.join().unwrap();
        drop(gate);
        result
    });
    assert!(
        matches!(result, Err(BrowserError::Timeout { .. })),
        "the service holds the slow tab's script while the peer loads: {result:?}"
    );
}

#[test]
fn a_new_document_in_the_same_tab_cannot_finish_the_requested_document() {
    let service = ScratchService::start();
    let driver = BrowserDriver::launch(
        LaunchConfig::chromium()
            .ctx(Ctx::host())
            .navigate_timeout(Duration::from_secs(5)),
    )
    .expect("launch Chromium");
    std::thread::scope(|scope| {
        let driver = &driver;
        let url = service.url("/redirect-before-load");
        let navigation = scope.spawn(move || driver.new_page().unwrap().goto(&url));
        // Release the in-page redirect only after Chromium requests the blocked script.
        let first = service.receive_request();
        let second = service.receive_request();
        let mut requests = std::collections::HashMap::from([first, second]);
        let blocked_script = requests
            .remove("/gate")
            .expect("the first document requests the script");
        reply(
            &mut requests
                .remove("/redirect")
                .expect("the first document requests permission to redirect"),
            "text/plain",
            "go",
        );
        let (path, mut beacon) = service.receive_request();
        assert_eq!(path, "/replacement-loaded");
        reply(&mut beacon, "text/plain", "");

        let result = navigation.join().unwrap();
        drop(blocked_script);
        assert!(
            matches!(result, Err(BrowserError::Timeout { .. })),
            "the replacement document completes with a different loader: {result:?}"
        );
    });
}

#[test]
fn reload_fragment_navigation_and_connection_failure() {
    let service = ScratchService::start();
    let driver = BrowserDriver::launch(
        LaunchConfig::chromium()
            .ctx(Ctx::host())
            .navigate_timeout(Duration::from_secs(5)),
    )
    .expect("launch Chromium");
    let page = driver.new_page().expect("attach the page");
    let url = service.url("/peer");
    page.goto(&url).expect("load the first document");
    page.eval("window.navigationMarker = 17").unwrap();

    page.goto(&url)
        .expect("load a new document at the same address");
    assert_eq!(
        page.eval("typeof window.navigationMarker").unwrap(),
        "undefined"
    );
    page.eval("window.navigationMarker = 23").unwrap();

    page.goto(&format!("{url}#section"))
        .expect("navigate within the document");
    assert_eq!(page.eval("location.hash").unwrap(), "#section");
    assert_eq!(page.eval("window.navigationMarker").unwrap(), 23);

    let error = page
        .goto(&service.url("/abort"))
        .expect_err("the service closes without a response");
    assert!(
        matches!(error, BrowserError::Protocol { ref message, .. } if message.contains("net::ERR_")),
        "expected Chromium's navigation error, got {error:?}"
    );

    page.goto(&url).expect("recover after a failed navigation");
    assert_eq!(page.eval("document.title").unwrap(), "Peer");
}

fn serve(mut stream: TcpStream, requested: &mpsc::Sender<(String, TcpStream)>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut line = String::new();
    let mut reader = BufReader::new(&stream);
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) if line == "\r\n" => break,
            Ok(_) => {}
        }
    }
    match path.as_str() {
        "/slow" => reply(&mut stream, "text/html", "<script src='/gate'></script>"),
        "/redirect-before-load" => reply(
            &mut stream,
            "text/html",
            "<script>fetch('/redirect').then(() => location.href = '/replacement')</script><script src='/gate'></script>",
        ),
        "/replacement" => reply(
            &mut stream,
            "text/html",
            "<script>addEventListener('load', () => fetch('/replacement-loaded'))</script>",
        ),
        "/gate" | "/redirect" | "/replacement-loaded" => requested.send((path, stream)).unwrap(),
        "/abort" => {}
        _ => reply(&mut stream, "text/html", "<title>Peer</title>"),
    }
}
