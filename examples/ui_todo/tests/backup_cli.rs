//! Verify maintenance commands and start the server on the imported database.
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
};
use tempfile::tempdir;

const EXISTING: &str = "34aaf305-1548-4201-bd23-f3ce099cfeb4";

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_todo-maintenance"))
}

fn convert(input: &Path, output: &Path) -> Output {
    command()
        .arg("convert")
        .arg(input)
        .arg(output)
        .output()
        .unwrap()
}

fn rejects_restore(file: &Path) {
    let result = command()
        .arg("import")
        .arg(file)
        .arg(file.with_extension("sqlite3"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(result.stdout.as_slice(), b"");
}

#[test]
fn converts_in_order_and_restores_the_new_file() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("source");
    let output = dir.path().join("current");
    let original = json!([
        {"id":3,"text":"first","done":true},
        {"id":1,"text":"second","done":false,"creation_id":EXISTING}
    ])
    .to_string();
    fs::write(&input, &original).unwrap();
    rejects_restore(&input);
    let result = convert(&input, &output);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved = fs::read(&output).unwrap();
    let items: Vec<ui_todo::Todo> = serde_json::from_slice(&saved).unwrap();
    assert_eq!(items.len(), 2);
    assert_ne!(items[0].id, items[1].id);
    assert_eq!(items[1].id.to_string(), EXISTING);
    assert_eq!((&*items[0].text, items[0].done), ("first", true));
    assert_eq!((&*items[1].text, items[1].done), ("second", false));
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        format!("numeric_id\ttodo_id\n3\t{}\n1\t{EXISTING}\n", items[0].id)
    );
    assert!(!convert(&input, &output).status.success());
    assert!(!convert(&input, &input).status.success());
    assert_eq!(fs::read(&output).unwrap(), saved);
    assert_eq!(fs::read_to_string(&input).unwrap(), original);
    let database = dir.path().join("todos.sqlite3");
    assert!(command()
        .arg("import")
        .arg(&output)
        .arg(&database)
        .status()
        .unwrap()
        .success());
    let repeated = command()
        .arg("import")
        .arg(&output)
        .arg(&database)
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("already contains Todo records"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_ui_todo"))
        .args(["127.0.0.1:0", "--store"])
        .arg(database)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut address = String::new();
    let read = BufReader::new(child.stdout.take().unwrap()).read_line(&mut address);
    let initial = (|| {
        read?;
        let host = address
            .trim()
            .strip_prefix("Open http://")
            .and_then(|url| url.strip_suffix('/'))
            .ok_or_else(|| std::io::Error::other("Missing server address"))?;
        let mut socket = std::net::TcpStream::connect(host)?;
        socket.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        write!(
            socket,
            "GET /initial HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
        )?;
        let mut response = Vec::new();
        socket.read_to_end(&mut response)?;
        Ok::<_, std::io::Error>(response)
    })();
    let cleanup = child.kill();
    child.wait().unwrap();
    cleanup.unwrap();
    let initial = initial.unwrap();
    assert!(initial.starts_with(b"HTTP/1.1 200"));
    for title in ["first", "second"] {
        assert!(initial
            .windows(title.len())
            .any(|chunk| chunk == title.as_bytes()));
    }
}

#[test]
fn rejects_ambiguous_or_malformed_records() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("new");
    let row = |id: Value| json!({"id":id,"text":"title","done":true});
    let duplicate_creation = |id| json!({"id":id,"text":"","done":false,"creation_id":EXISTING});
    for data in [
        json!([row(json!(0))]),
        json!([row(json!(1)), row(json!(1))]),
        json!([row(json!(1)), row(json!(EXISTING))]),
        json!([duplicate_creation(1), duplicate_creation(2)]),
        json!([{"id":1,"text":"","done":false,"creation_id":"invalid"}]),
        json!([{"id":1,"text":"","done":false,"other":true}]),
    ] {
        fs::write(&input, data.to_string()).unwrap();
        assert!(!convert(&input, &output).status.success());
        assert!(!output.exists());
        rejects_restore(&input);
    }
    for invalid in [
        json!([row(json!(EXISTING)), row(json!(EXISTING))]).to_string(),
        json!([{"id":EXISTING,"creation_id":EXISTING,"done":true,"text":""}]).to_string(),
        String::from("[] trailing data"),
    ] {
        fs::write(&input, invalid).unwrap();
        rejects_restore(&input);
        assert!(!convert(&input, &output).status.success());
        assert!(!output.exists());
    }
    fs::write(&input, "[]").unwrap();
    assert!(convert(&input, &output).status.success());
    assert_eq!(fs::read(&output).unwrap(), b"[]");
}

#[test]
fn bounds_input_and_converted_output() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("new");
    let limit = foundation_ui_wire::WireLimits::default().frame_bytes();
    fs::File::create(&input)
        .unwrap()
        .set_len(limit as u64 + 1)
        .unwrap();
    let result = convert(&input, &output);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("byte limit"));
    rejects_restore(&input);
    let mut record = String::from(r#"[{"id":1,"done":false,"text":""#);
    record.push_str(&"x".repeat(limit - record.len() - 3));
    record.push_str(r#""}]"#);
    fs::write(&input, record).unwrap();
    let result = convert(&input, &output);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("exceeds the restore size limit"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
}
