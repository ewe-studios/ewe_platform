//! Streaming a multi-chunk body with `into_next_stream()` — the valtron way to
//! `while let Some(chunk) = body.next().await` without buffering the whole body.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_streaming
//! ```
//!
//! `into_next_stream()` turns a driven `StreamIterator` into a
//! `futures_core::Stream<Item = D>` that surfaces each `Ready` value and parks
//! over the in-band control states (`Pending`/`Delayed`/`Init`/`Wait`). This is
//! the bridge for chunk-by-chunk consumption of HTTP/RPC/upload bodies — the
//! point of "streams, not buffers".
//!
//! The `#[task]` machine emits one `Ready` per chunk; because `Ready` is **not**
//! terminal, the chunks stream out one at a time until the machine returns
//! `None` (body fully delivered).

use foundation_core::valtron;
use foundation_core::valtron::{
    execute, task, valtron, Ctx, StreamExt, StreamIteratorExt, TaskStatus,
};

/// A response body split into chunks, streamed out one at a time.
#[derive(Debug, Clone)]
struct Download {
    chunks: Vec<&'static str>,
    index: usize,
}

#[task]
fn download(ctx: Ctx, state: Download) -> Option<(Download, TaskStatus<&'static str, ()>)> {
    let _ = ctx;
    let Download { chunks, index } = state;
    if index >= chunks.len() {
        return None; // body fully delivered — the stream ends here
    }
    let chunk = chunks[index];
    Some((Download { chunks, index: index + 1 }, TaskStatus::Ready(chunk)))
}

#[valtron]
async fn main() {
    let body = ["Hello", ", ", "valtron", " ", "streams", "!"];
    let mut stream = execute(
        download::new(Download { chunks: body.to_vec(), index: 0 }),
        None,
    )
    .expect("engine live")
    .into_next_stream();

    // Await each chunk as it arrives — nothing is buffered into a Vec first.
    let mut received = Vec::new();
    while let Some(chunk) = stream.next().await {
        received.push(chunk);
    }

    let rejoined: String = received.concat();
    assert_eq!(rejoined, "Hello, valtron streams!");
    println!("streamed -> {received:?}");
}
