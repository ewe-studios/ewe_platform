/// Demonstrates the auto-rejoin pattern.
///
/// Creates and drops a sender/receiver pair, then joins again.
/// Proves the bus survives endpoint churn.
///
/// Run: cargo run -p foundation_nativeapis --features ipc --example ipc_rejoin
#[cfg(all(target_os = "linux", feature = "ipc"))]
fn main() {
    use foundation_nativeapis::ipc::{self, BytesMessage, Label, LabelOp, Message, Options, Selector};
    use std::time::Duration;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    println!("First join...");
    {
        let bus = format!("com.ewe.rejoin.{ts}.1");
        let opts = Options::new(&bus, Label::new("node")).controller_affinity(true);
        let (_sender, _receiver) =
            ipc::join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
                .expect("first join");
        println!("  Joined. Dropping...");
    }
    println!("  Dropped.");

    println!("Second join (fresh bus)...");
    let bus = format!("com.ewe.rejoin.{ts}.2");
    let opts = Options::new(&bus, Label::new("node")).controller_affinity(true);
    let (sender, mut receiver) =
        ipc::join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
            .expect("second join");
    println!("  Joined.");

    sender
        .send(Message::new(
            Selector::multicast(LabelOp::True),
            BytesMessage {
                format: 42,
                data: b"survived rejoin".to_vec(),
            },
        ))
        .expect("send");

    let msg = receiver
        .recv(Some(Duration::from_secs(2)))
        .expect("recv");
    println!(
        "  Received: format={} data={:?}",
        msg.payload.format,
        String::from_utf8_lossy(&msg.payload.data)
    );
    println!("Rejoin successful.");
}

#[cfg(not(all(target_os = "linux", feature = "ipc")))]
fn main() {
    eprintln!("This example requires Linux and the 'ipc' feature.");
}
