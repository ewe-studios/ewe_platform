//! Verify a real request, a durable write, and reopening the database.
use super::{Path, FIRST};

#[test]
fn server_writes_and_reads_the_same_database_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Todo ü.sqlite3");

    let (server, address) = start_server(&path, &dir.path().join("first.log"));
    add_server_record(address);
    assert_server_record(address);
    drop(server);

    let (_server, address) = start_server(&path, &dir.path().join("second.log"));
    assert_server_record(address);
}

// Stop the child on assertion failure before the temporary directory is removed.
struct ServerProcess(std::process::Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server(path: &Path, log: &Path) -> (ServerProcess, std::net::SocketAddr) {
    use std::time::{Duration, Instant};

    let stdout = std::fs::File::create(log).unwrap();
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_ui_todo"))
        .args(["127.0.0.1:0", "--store"])
        .arg(path)
        .stdout(stdout)
        .spawn()
        .unwrap();
    let mut server = ServerProcess(child);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let output = std::fs::read_to_string(log).unwrap();
        if let Some(address) = output
            .lines()
            .find_map(|line| line.strip_prefix("Open http://")?.strip_suffix('/'))
        {
            return (server, address.parse().unwrap());
        }
        assert!(
            server.0.try_wait().unwrap().is_none() && Instant::now() < deadline,
            "The Todo server did not report its bound address: {output}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn add_server_record(address: std::net::SocketAddr) {
    use buffa::Message;
    use foundation_ui_connectrpc::v1::{procedure::EXCHANGE, UiRequest};
    use foundation_ui_runtime::network::client_hello;
    use foundation_ui_wire::{
        finish_frame, writer::ClientCaps, ActionInputT, ActionRequestT, FrameKindT, FrameRequestT,
        FrameT, SCHEMA_VERSION,
    };

    let hello = client_hello(&ClientCaps::web_current());
    let mut action = ActionRequestT::default();
    action.action = Some("server:add_todo".into());
    let mut input = ActionInputT::default();
    input.json = Some(
        serde_json::to_vec(&serde_json::json!({
            "id": FIRST, "title": "Persisted across process restart"
        }))
        .unwrap(),
    );
    action.input = Some(Box::new(input));
    let mut request = FrameRequestT::default();
    request.action = Some(Box::new(action));
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = 1;
    frame.sequence_id = 1;
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    let rpc = UiRequest {
        frame: finish_frame(&frame).unwrap(),
        hello: Some(hello),
        ..Default::default()
    };
    post(address, EXCHANGE, "application/proto", &rpc.encode_to_vec());
}

fn assert_server_record(address: std::net::SocketAddr) {
    let request = serde_json::json!({ "id": FIRST }).to_string();
    let procedure = ui_todo::todo_rpc::procedure::GET_TODO;
    let body = post(address, procedure, "application/json", request.as_bytes());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({ "id": FIRST, "title": "Persisted across process restart" })
    );
}

fn post(address: std::net::SocketAddr, procedure: &str, media_type: &str, body: &[u8]) -> Vec<u8> {
    use std::io::{Read, Write};
    use std::time::Duration;

    let timeout = Duration::from_secs(5);
    let mut stream = std::net::TcpStream::connect_timeout(&address, timeout).unwrap();
    stream.set_read_timeout(Some(timeout)).unwrap();
    stream.set_write_timeout(Some(timeout)).unwrap();
    write!(
        stream,
        "POST {procedure} HTTP/1.1\r\nHost: {address}\r\nContent-Type: {media_type}\r\nConnect-Protocol-Version: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let head_len = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap();
    assert!(
        response.starts_with(b"HTTP/1.1 200 "),
        "{}",
        String::from_utf8_lossy(&response)
    );
    response.split_off(head_len + 4)
}
