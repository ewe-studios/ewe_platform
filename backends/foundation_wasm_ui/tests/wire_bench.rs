//! WHY: "How fast is our compact columnar vs real Arrow IPC?" deserves
//! numbers, not architecture talk (spec-42 review question, 2026-06-13).
//!
//! WHAT: An IGNORED bench-style test printing encode/decode timings and
//! payload sizes for `ColumnarEncoder` (wire v1) vs
//! `foundation_arrow::ArrowIpcEncoder` (wire v2) over realistic `DomOp`
//! batches. Run it explicitly, in RELEASE (uat is unoptimized):
//!
//! ```sh
//! cargo test --release -p foundation_wasm_ui --features arrow \
//!     --test wire_bench -- --ignored --nocapture
//! ```

#![cfg(feature = "arrow")]

use std::time::Instant;

use foundation_arrow::ArrowIpcEncoder;
use foundation_ui_traits::{AttrName, ColumnarEncoder, DomOp, HtmlTag, ProtocolEncoder};

/// A realistic UI-update mix: creates, registers, attributes, text, appends.
fn batch(n: usize) -> Vec<DomOp> {
    (0..n)
        .flat_map(|i| {
            let id = 16 + u32::try_from(i).expect("id");
            [
                DomOp::CreateElement {
                    node_id: id,
                    tag: HtmlTag::from_static("div"),
                    class: "card item".into(),
                },
                DomOp::RegisterNode { node_id: id },
                DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-index"),
                    value: format!("item-{i}").into(),
                },
                DomOp::SetText {
                    node_id: id,
                    text: format!("row {i} content").into(),
                },
                DomOp::AppendChild {
                    parent_id: 1,
                    child_id: id,
                },
            ]
        })
        .collect()
}

fn bench<E: ProtocolEncoder<Vec<DomOp>>>(
    label: &str,
    encoder: &E,
    ops: &[DomOp],
    iters: u32,
) -> usize {
    // Pre-clone outside the timed loop so both encoders pay identical setup.
    let inputs: Vec<Vec<DomOp>> = (0..iters).map(|_| ops.to_vec()).collect();
    let start = Instant::now();
    let mut bytes = 0usize;
    let mut payload = Vec::new();
    for input in inputs {
        payload = encoder.encode(input);
        bytes = payload.len();
    }
    let encode_each = start.elapsed() / iters;

    let start = Instant::now();
    for _ in 0..iters {
        let decoded = encoder.decode(&payload).expect("round-trip");
        assert_eq!(decoded.len(), ops.len());
    }
    let decode_each = start.elapsed() / iters;

    println!(
        "  {label:<22} encode {encode_each:>10.2?}  decode {decode_each:>10.2?}  payload {bytes:>9} B"
    );
    bytes
}

#[test]
#[ignore = "bench — run explicitly in --release with --nocapture"]
fn columnar_vs_arrow_ipc() {
    for (n_items, iters) in [(2usize, 20_000u32), (20, 5_000), (200, 500), (2_000, 50)] {
        let ops = batch(n_items);
        println!("\n== {} DomOps ({n_items} items) ==", ops.len());
        let v1 = bench("compact columnar (v1)", &ColumnarEncoder, &ops, iters);
        let v2 = bench("Arrow IPC (v2)", &ArrowIpcEncoder, &ops, iters);
        #[allow(clippy::cast_precision_loss)]
        let ratio = v2 as f64 / v1 as f64;
        println!("  size ratio v2/v1: {ratio:.2}x");
    }
}
