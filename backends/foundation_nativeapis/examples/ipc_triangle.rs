/// Multi-endpoint routing — 3 endpoints in a triangle topology.
///
/// Each endpoint sends to the other two via `Or()` label expressions.
/// Demonstrates multicast + MemoryRegion passing between threads.
///
/// Run: cargo run -p foundation_nativeapis --features ipc --example ipc_triangle
#[cfg(all(target_os = "linux", feature = "ipc"))]
fn main() {
    use foundation_macros::TypeUuid;
    use foundation_nativeapis::ipc::{
        self, Label, LabelOp, MemoryRegion, Message, Options, Selector,
    };
    use serde::{Deserialize, Serialize};
    use std::{thread, time::Duration};

    #[derive(Debug, Serialize, Deserialize, TypeUuid)]
    #[uuid = "d4adfc76-f5f4-40b0-8e28-8a51a12f5e46"]
    struct Ping {
        from: String,
        seq: u32,
    }

    let bus = format!(
        "com.ewe.triangle.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let mut handles = vec![];
    for (name, targets) in [
        ("a", LabelOp::from("b").or("c")),
        ("b", LabelOp::from("a").or("c")),
        ("c", LabelOp::from("a").or("b")),
    ] {
        let bus = bus.clone();
        handles.push(thread::spawn(move || {
            let opts = Options::new(&bus, Label::new(name)).controller_affinity(true);
            let (sender, mut receiver) =
                ipc::join::<Ping, Ping>(opts, Some(Duration::from_secs(5))).expect("join");

            // Send 2 messages with a MemoryRegion
            for seq in 0..2 {
                let mut msg = Message::new(
                    Selector::multicast(targets.clone()),
                    Ping { from: name.into(), seq },
                );
                if let Some(mut region) = MemoryRegion::new(64) {
                    let buf = region.map(..).unwrap();
                    buf[0] = seq as u8;
                    msg.memory_regions.push(region);
                }
                sender.send(msg).expect("send");
            }

            // Receive messages from the other 2 endpoints (2 msgs each = 4 total)
            let mut received = vec![];
            for _ in 0..4 {
                match receiver.recv(Some(Duration::from_secs(3))) {
                    Ok(msg) => {
                        let has_region = !msg.memory_regions.is_empty();
                        received.push(format!(
                            "{} <- {} seq={} region={}",
                            name, msg.payload.from, msg.payload.seq, has_region
                        ));
                    }
                    Err(e) => {
                        received.push(format!("{} recv error: {:?}", name, e));
                        break;
                    }
                }
            }
            received
        }));
    }

    for h in handles {
        let results = h.join().expect("thread panicked");
        for r in results {
            println!("  {r}");
        }
    }
    println!("Triangle routing complete.");
}

#[cfg(not(all(target_os = "linux", feature = "ipc")))]
fn main() {
    eprintln!("This example requires Linux and the 'ipc' feature.");
}
