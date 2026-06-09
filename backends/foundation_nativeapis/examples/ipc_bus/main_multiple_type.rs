/// Demonstrates heterogeneous messaging on a single IPC bus.
///
/// Defines two custom message types and an enum wrapper using `#[derive(MessageBox)]`.
/// The sender transmits different variant types, and the receiver dispatches by UUID.
///
/// Run: cargo run -p foundation_nativeapis --features ipc --example ipc_multiple_type
#[cfg(all(target_os = "linux", feature = "ipc"))]
fn main() {
    use foundation_macros::{MessageBox, TypeUuid};
    use foundation_nativeapis::ipc::{self, Label, LabelOp, Message, Options, Selector};
    use serde::{Deserialize, Serialize};
    use std::{thread, time::Duration};

    #[derive(Debug, Serialize, Deserialize, TypeUuid)]
    #[uuid = "7b07473e-9659-4d47-a502-8245d71c0078"]
    struct FileEvent {
        path: String,
        kind: u32,
    }

    #[derive(Debug, Serialize, Deserialize, TypeUuid)]
    #[uuid = "e12c8a9f-3b4d-4e5f-a6b7-c8d9e0f1a2b3"]
    struct LogEntry {
        level: u8,
        message: String,
    }

    #[derive(MessageBox)]
    enum AppMessage {
        File(FileEvent),
        Log(LogEntry),
        Bytes(ipc::BytesMessage),
    }

    let opts = Options::new("com.ewe.multi-type-demo", Label::new("node"))
        .controller_affinity(true);
    let (sender, mut receiver) =
        ipc::join::<AppMessage, AppMessage>(opts, Some(Duration::from_secs(5)))
            .expect("join failed");

    let s = sender.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));

        s.send(Message::new(
            Selector::multicast(LabelOp::True),
            AppMessage::File(FileEvent {
                path: "/src/main.rs".into(),
                kind: 1,
            }),
        ))
        .expect("send FileEvent");

        s.send(Message::new(
            Selector::multicast(LabelOp::True),
            AppMessage::Log(LogEntry {
                level: 2,
                message: "build complete".into(),
            }),
        ))
        .expect("send LogEntry");

        s.send(Message::new(
            Selector::multicast(LabelOp::True),
            AppMessage::Bytes(ipc::BytesMessage {
                format: 99,
                data: b"raw payload".to_vec(),
            }),
        ))
        .expect("send BytesMessage");
    });

    for _ in 0..3 {
        let msg = receiver
            .recv(Some(Duration::from_secs(2)))
            .expect("recv failed");
        match msg.payload {
            AppMessage::File(e) => println!("FileEvent: {} (kind={})", e.path, e.kind),
            AppMessage::Log(e) => println!("LogEntry: [{}] {}", e.level, e.message),
            AppMessage::Bytes(b) => {
                println!("BytesMessage: format={} len={}", b.format, b.data.len())
            }
        }
    }
    println!("All 3 message types received successfully.");
}

#[cfg(not(all(target_os = "linux", feature = "ipc")))]
fn main() {
    eprintln!("This example requires Linux and the 'ipc' feature.");
}
