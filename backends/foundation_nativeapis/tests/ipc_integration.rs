/// Integration tests for the IPC message bus (Linux).
///
/// Tests the ipmb-adapted bus: join, send/recv, multicast routing,
/// shared memory (MemoryRegion), custom message types, and reconnection.
#[cfg(all(target_os = "linux", feature = "ipc"))]
mod tests {
    use foundation_nativeapis::ipc::{
        join, BytesMessage, Label, LabelOp, MemoryRegion, Message, Options, Selector,
    };
    use serde::{Deserialize, Serialize};
    use std::thread;
    use std::time::Duration;
    use foundation_macros::{TypeUuid, MessageBox};

    fn unique_bus(name: &str) -> String {
        format!(
            "com.ewe.test.{}.{}.{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[derive(Debug, Serialize, Deserialize, TypeUuid, PartialEq)]
    #[uuid = "f47ac10b-58cc-4372-a567-0e02b2c3d479"]
    struct TestEvent {
        path: String,
        kind: u32,
    }

    // -- Test 1: Controller sends to itself (server-mode loopback) ----------

    #[test]
    fn controller_loopback() {
        let bus = unique_bus("loopback");
        let opts = Options::new(&bus, Label::new("ctrl")).controller_affinity(true);
        let (sender, mut receiver) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("join failed");

        let payload = BytesMessage {
            format: 1,
            data: b"hello loopback".to_vec(),
        };
        let msg = Message::new(Selector::multicast(LabelOp::True), payload);
        sender.send(msg).expect("send failed");

        let received = receiver
            .recv(Some(Duration::from_secs(2)))
            .expect("recv failed");
        assert_eq!(received.payload.data, b"hello loopback");
        assert_eq!(received.payload.format, 1);
    }

    // -- Test 2: Two-thread bus — endpoint sends, controller receives ------

    #[test]
    fn two_thread_send_recv() {
        let bus = unique_bus("two-thread");

        let bus_clone = bus.clone();
        let ctrl = thread::spawn(move || {
            let opts =
                Options::new(&bus_clone, Label::new("controller")).controller_affinity(true);
            let (_sender, mut receiver) =
                join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                    .expect("controller join");

            let msg = receiver
                .recv(Some(Duration::from_secs(5)))
                .expect("controller recv");
            msg.payload.data
        });

        thread::sleep(Duration::from_millis(100));

        let opts = Options::new(&bus, Label::new("client")).controller_affinity(false);
        let (sender, _receiver) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("client join");

        let payload = BytesMessage {
            format: 0,
            data: b"from client".to_vec(),
        };
        let msg = Message::new(
            Selector::unicast(LabelOp::Leaf("controller".into())),
            payload,
        );
        sender.send(msg).expect("client send");

        let data = ctrl.join().expect("controller thread panicked");
        assert_eq!(data, b"from client");
    }

    // -- Test 3: Custom message type via TypeUuid --------------------------

    #[test]
    fn custom_message_type() {
        let bus = unique_bus("custom-type");
        let opts = Options::new(&bus, Label::new("node")).controller_affinity(true);
        let (sender, mut receiver) =
            join::<TestEvent, TestEvent>(opts, Some(Duration::from_secs(5)))
                .expect("join failed");

        let event = TestEvent {
            path: "/src/main.rs".into(),
            kind: 42,
        };
        let msg = Message::new(Selector::multicast(LabelOp::True), event);
        sender.send(msg).expect("send failed");

        let received = receiver
            .recv(Some(Duration::from_secs(2)))
            .expect("recv failed");
        assert_eq!(received.payload.path, "/src/main.rs");
        assert_eq!(received.payload.kind, 42);
    }

    // -- Test 4: Multicast routing — controller broadcasts to client ------

    #[test]
    fn multicast_routing() {
        let bus = unique_bus("multicast");

        let bus_c = bus.clone();
        let endpoint_b = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            let opts = Options::new(&bus_c, Label::new("b")).controller_affinity(false);
            let (_sender, mut receiver) =
                join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                    .expect("b join");
            receiver
                .recv(Some(Duration::from_secs(5)))
                .expect("b recv")
                .payload
                .data
        });

        let opts = Options::new(&bus, Label::new("a")).controller_affinity(true);
        let (sender, mut receiver_a) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("a join");

        thread::sleep(Duration::from_millis(300));

        let payload = BytesMessage {
            format: 0,
            data: b"broadcast".to_vec(),
        };
        let msg = Message::new(Selector::multicast(LabelOp::True), payload);
        sender.send(msg).expect("broadcast send");

        let msg_a = receiver_a
            .recv(Some(Duration::from_secs(2)))
            .expect("a recv");
        assert_eq!(msg_a.payload.data, b"broadcast");

        let data_b = endpoint_b.join().expect("b panicked");
        assert_eq!(data_b, b"broadcast");
    }

    // -- Test 5: Unicast routing — only one endpoint receives --------------

    #[test]
    fn unicast_routing() {
        let bus = unique_bus("unicast");

        let bus_c = bus.clone();
        let target = thread::spawn(move || {
            let opts = Options::new(&bus_c, Label::new("target")).controller_affinity(true);
            let (_s, mut r) = join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("target join");
            r.recv(Some(Duration::from_secs(5)))
                .expect("target recv")
                .payload
                .data
        });

        thread::sleep(Duration::from_millis(100));

        let bus_c = bus.clone();
        let other = thread::spawn(move || {
            let opts = Options::new(&bus_c, Label::new("other")).controller_affinity(false);
            let (_s, mut r) = join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("other join");
            r.recv(Some(Duration::from_millis(500)))
        });

        thread::sleep(Duration::from_millis(100));

        let opts = Options::new(&bus, Label::new("sender")).controller_affinity(false);
        let (sender, _r) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("sender join");

        let msg = Message::new(
            Selector::unicast(LabelOp::Leaf("target".into())),
            BytesMessage {
                format: 0,
                data: b"for target only".to_vec(),
            },
        );
        sender.send(msg).expect("send");

        let data = target.join().expect("target panicked");
        assert_eq!(data, b"for target only");

        let other_result = other.join().expect("other panicked");
        assert!(
            other_result.is_err(),
            "other endpoint should not receive unicast message"
        );
    }

    // -- Test 6: Shared memory (MemoryRegion) ------------------------------

    #[test]
    fn shared_memory_region() {
        let bus = unique_bus("shmem");
        let opts = Options::new(&bus, Label::new("mem")).controller_affinity(true);
        let (sender, mut receiver) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("join failed");

        let mut region = MemoryRegion::new(4096).expect("MemoryRegion::new failed");
        {
            let buf = region.map(..).expect("map failed");
            buf[..5].copy_from_slice(b"hello");
        }
        assert_eq!(region.buffer_size(), 4096);

        let payload = BytesMessage {
            format: 99,
            data: vec![],
        };
        let mut msg = Message::new(Selector::multicast(LabelOp::True), payload);
        msg.memory_regions.push(region);
        sender.send(msg).expect("send with region");

        let received = receiver
            .recv(Some(Duration::from_secs(2)))
            .expect("recv with region");
        assert_eq!(received.payload.format, 99);
        assert_eq!(received.memory_regions.len(), 1);

        let mut recv_region = received.memory_regions.into_iter().next().unwrap();
        let buf = recv_region.map(..).expect("recv map failed");
        assert_eq!(&buf[..5], b"hello");
    }

    // -- Test 7: Token mismatch rejected -----------------------------------

    #[test]
    fn token_mismatch_rejected() {
        let bus = unique_bus("token-mismatch");

        let bus_c = bus.clone();
        let _ctrl = thread::spawn(move || {
            let opts = Options::new(&bus_c, Label::new("ctrl"))
                .controller_affinity(true)
                .token("secret123");
            let (_s, mut r) = join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("ctrl join");
            let _ = r.recv(Some(Duration::from_secs(3)));
        });

        thread::sleep(Duration::from_millis(200));

        let opts = Options::new(&bus, Label::new("intruder"))
            .controller_affinity(false)
            .token("wrong-token");
        let result = join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(2)));

        assert!(result.is_err(), "join with wrong token should fail");
    }

    // -- Test 8: Multiple messages in sequence -----------------------------

    #[test]
    fn multiple_messages_sequence() {
        let bus = unique_bus("sequence");
        let opts = Options::new(&bus, Label::new("seq")).controller_affinity(true);
        let (sender, mut receiver) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("join");

        for i in 0..10u16 {
            let msg = Message::new(
                Selector::multicast(LabelOp::True),
                BytesMessage {
                    format: i,
                    data: format!("msg-{i}").into_bytes(),
                },
            );
            sender.send(msg).expect("send");
        }

        for i in 0..10u16 {
            let received = receiver
                .recv(Some(Duration::from_secs(2)))
                .expect("recv");
            assert_eq!(received.payload.format, i);
            assert_eq!(received.payload.data, format!("msg-{i}").into_bytes());
        }
    }

    // -- Test 9: Sender is Clone, multiple threads can send ----------------

    #[test]
    fn sender_clone_multithreaded() {
        let bus = unique_bus("clone-send");
        let opts = Options::new(&bus, Label::new("hub")).controller_affinity(true);
        let (sender, mut receiver) =
            join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("join");

        let mut handles = vec![];
        for i in 0..4u16 {
            let s = sender.clone();
            handles.push(thread::spawn(move || {
                let msg = Message::new(
                    Selector::multicast(LabelOp::True),
                    BytesMessage {
                        format: i,
                        data: vec![i as u8; 64],
                    },
                );
                s.send(msg).expect("clone send");
            }));
        }

        for h in handles {
            h.join().expect("send thread panicked");
        }

        let mut received_formats = vec![];
        for _ in 0..4 {
            let msg = receiver
                .recv(Some(Duration::from_secs(2)))
                .expect("recv");
            received_formats.push(msg.payload.format);
        }
        received_formats.sort();
        assert_eq!(received_formats, vec![0, 1, 2, 3]);
    }

    // -- Test 10: Heterogeneous messaging via #[derive(MessageBox)] --------

    #[derive(Debug, Serialize, Deserialize, TypeUuid, PartialEq)]
    #[uuid = "a1a1a1a1-b2b2-c3c3-d4d4-e5e5e5e5e5e5"]
    struct FileEvent {
        path: String,
        kind: u32,
    }

    #[derive(Debug, Serialize, Deserialize, TypeUuid, PartialEq)]
    #[uuid = "f6f6f6f6-a7a7-b8b8-c9c9-d0d0d0d0d0d0"]
    struct LogEntry {
        level: u8,
        message: String,
    }

    #[derive(MessageBox)]
    enum AppMessage {
        File(FileEvent),
        Log(LogEntry),
    }

    #[test]
    fn heterogeneous_message_types() {
        let bus = unique_bus("hetero");
        let opts = Options::new(&bus, Label::new("app")).controller_affinity(true);
        let (sender, mut receiver) =
            join::<AppMessage, AppMessage>(opts, Some(Duration::from_secs(5)))
                .expect("join");

        // Send a FileEvent
        sender
            .send(Message::new(
                Selector::multicast(LabelOp::True),
                AppMessage::File(FileEvent {
                    path: "/src/main.rs".into(),
                    kind: 1,
                }),
            ))
            .expect("send FileEvent");

        // Send a LogEntry
        sender
            .send(Message::new(
                Selector::multicast(LabelOp::True),
                AppMessage::Log(LogEntry {
                    level: 3,
                    message: "build complete".into(),
                }),
            ))
            .expect("send LogEntry");

        // Receive and match
        let msg1 = receiver.recv(Some(Duration::from_secs(2))).expect("recv 1");
        match msg1.payload {
            AppMessage::File(ref e) => {
                assert_eq!(e.path, "/src/main.rs");
                assert_eq!(e.kind, 1);
            }
            _ => panic!("expected FileEvent, got {:?}", std::mem::discriminant(&msg1.payload)),
        }

        let msg2 = receiver.recv(Some(Duration::from_secs(2))).expect("recv 2");
        match msg2.payload {
            AppMessage::Log(ref e) => {
                assert_eq!(e.level, 3);
                assert_eq!(e.message, "build complete");
            }
            _ => panic!("expected LogEntry"),
        }
    }

    // -- Test 11: LabelOp builder ergonomics -------------------------------

    #[test]
    fn label_op_builder_api() {
        let op = LabelOp::from("target");
        assert!(op.validate(&Label::new("target")));

        let op = LabelOp::from("a").or("b");
        let sel = Selector::multicast(op);
        assert!(sel.validate(&Label::new("a")));
        assert!(sel.validate(&Label::new("b")));
        assert!(!sel.validate(&Label::new("c")));

        let op = !LabelOp::from("excluded");
        assert!(!op.validate(&Label::new("excluded")));
        assert!(op.validate(&Label::new("anything-else")));
    }

    // -- Test 12: Selector::unicast with string directly -------------------

    #[test]
    fn selector_string_shorthand() {
        let sel = Selector::unicast("target");
        assert!(sel.validate(&Label::new("target")));
        assert!(!sel.validate(&Label::new("other")));

        let sel = Selector::multicast(LabelOp::from("a").or("b"));
        assert!(sel.validate(&Label::new("a")));
        assert!(sel.validate(&Label::new("b")));
    }
}
