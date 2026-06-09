/// Measures IPC round-trip latency.
///
/// Spawns a sender thread that timestamps each message and a receiver that
/// measures the receive delay. Reports latency in microseconds.
///
/// Run: cargo run -p foundation_nativeapis --features ipc --example ipc_latency
#[cfg(all(target_os = "linux", feature = "ipc"))]
fn main() {
    use foundation_macros::TypeUuid;
    use foundation_nativeapis::ipc::{self, Label, LabelOp, Message, Options, Selector};
    use serde::{Deserialize, Serialize};
    use std::{
        thread,
        time::{Duration, SystemTime},
    };

    #[derive(Debug, Serialize, Deserialize, TypeUuid)]
    #[uuid = "d4adfc76-f5f4-40b0-8e28-8a51a12f5e46"]
    struct TimedMessage {
        create: SystemTime,
        seq: u32,
    }

    let opts = Options::new("com.ewe.latency-demo", Label::new("controller"))
        .controller_affinity(true);
    let (sender, mut receiver) =
        ipc::join::<TimedMessage, TimedMessage>(opts, Some(Duration::from_secs(5)))
            .expect("join failed");

    let count = 20;
    thread::spawn(move || {
        for i in 0..count {
            let msg = Message::new(
                Selector::multicast(LabelOp::True),
                TimedMessage {
                    create: SystemTime::now(),
                    seq: i,
                },
            );
            sender.send(msg).expect("send");
            thread::sleep(Duration::from_millis(50));
        }
    });

    let mut total_us = 0u128;
    for _ in 0..count {
        let msg = receiver
            .recv(Some(Duration::from_secs(5)))
            .expect("recv");
        let latency = SystemTime::now()
            .duration_since(msg.payload.create)
            .unwrap_or_default();
        let us = latency.as_micros();
        total_us += us;
        println!("seq={:>3}  latency={:.3}ms", msg.payload.seq, us as f64 / 1000.0);
    }
    println!(
        "\nAverage latency: {:.3}ms over {} messages",
        (total_us as f64 / count as f64) / 1000.0,
        count
    );
}

#[cfg(not(all(target_os = "linux", feature = "ipc")))]
fn main() {
    eprintln!("This example requires Linux and the 'ipc' feature.");
}
