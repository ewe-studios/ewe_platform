/// Throughput benchmark — sends messages of varying sizes and reports messages/sec.
///
/// Run: cargo run -p foundation_nativeapis --features ipc --example ipc_bench --release
#[cfg(all(target_os = "linux", feature = "ipc"))]
fn main() {
    use foundation_nativeapis::ipc::{self, BytesMessage, Label, LabelOp, Message, Options, Selector};
    use std::{thread, time::{Duration, Instant}};

    let bus = format!(
        "com.ewe.bench.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let opts = Options::new(&bus, Label::new("bench")).controller_affinity(true);
    let (sender, mut receiver) =
        ipc::join::<BytesMessage, BytesMessage>(opts, Some(Duration::from_secs(5)))
            .expect("join");

    println!("{:>10} {:>12} {:>14}", "msg size", "msgs/sec", "throughput");
    println!("{}", "-".repeat(40));

    for (msg_size, count) in [
        (16, 100_000),
        (64, 50_000),
        (1024, 10_000),
        (4096, 5_000),
        (16384, 1_000),
    ] {
        let payload_data = vec![0xABu8; msg_size];
        let s = sender.clone();
        let data = payload_data.clone();

        let send_handle = thread::spawn(move || {
            for _ in 0..count {
                let msg = Message::new(
                    Selector::multicast(LabelOp::True),
                    BytesMessage { format: 0, data: data.clone() },
                );
                s.send(msg).expect("send");
            }
        });

        let start = Instant::now();
        let mut received = 0;
        while received < count {
            match receiver.recv(Some(Duration::from_secs(10))) {
                Ok(_) => received += 1,
                Err(e) => { eprintln!("recv error after {received}/{count}: {e:?}"); break; }
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        send_handle.join().expect("send thread");

        let msgs_sec = received as f64 / elapsed;
        let bytes_sec = msgs_sec * msg_size as f64;

        println!(
            "{:>10} {:>12.0} {:>12}/s",
            format_size(msg_size),
            msgs_sec,
            format_size(bytes_sec as usize),
        );
    }
}

#[cfg(all(target_os = "linux", feature = "ipc"))]
fn format_size(bytes: usize) -> String {
    if bytes >= 1 << 20 { format!("{:.1}MB", bytes as f64 / (1 << 20) as f64) }
    else if bytes >= 1 << 10 { format!("{:.1}KB", bytes as f64 / (1 << 10) as f64) }
    else { format!("{}B", bytes) }
}

#[cfg(not(all(target_os = "linux", feature = "ipc")))]
fn main() {
    eprintln!("This example requires Linux and the 'ipc' feature.");
}
