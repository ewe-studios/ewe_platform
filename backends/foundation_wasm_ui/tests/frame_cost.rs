//! Compare the two encoder paths for a small signal update.

use foundation_ui_traits::{ColumnarBatch, DomOp};
use foundation_ui_wire::ops;
use foundation_wasm::{MemoryAllocations, ProtocolHandler};
use foundation_wasm_ui::{ColumnarV1, FrameV1, ProtocolMethods};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: Counting = Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

fn sample(name: &str, count: u32, mut encode: impl FnMut() -> usize) {
    let bytes = encode();
    let iterations = 1000;
    let start = Instant::now();
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..iterations {
        assert_eq!(std::hint::black_box(encode()), bytes);
    }
    let allocations = ALLOCS.load(Ordering::Relaxed) - before;
    let nanos = start.elapsed().as_nanos() / (iterations as u128);
    println!(
        "{name} | {count} | {bytes} | {} | {nanos}",
        allocations / iterations
    );
}

#[test]
#[ignore = "run with the release profile to measure small signal updates"]
fn frame_and_native_column_costs() {
    for count in [1, 4, 16] {
        let mut arena = MemoryAllocations::new();
        let codec = ColumnarV1::new();
        let mut columns = ColumnarBatch::new();
        sample("columnar", count, || {
            for node_id in 0..count {
                columns.push(&DomOp::SetText {
                    node_id,
                    text: "updated".into(),
                });
            }
            let (result, _, bytes) = codec.write_batch(&columns, &mut arena);
            codec.ack(result.memory_id, &mut arena);
            columns.clear();
            bytes
        });
        let codec = FrameV1::new(2);
        sample("frame", count, || {
            let batch = (0..count).map(|i| ops::set_text(i, "updated")).collect();
            let (result, _, bytes) = codec.encode_and_write(batch, &mut arena).unwrap();
            codec.ack(result.memory_id, &mut arena);
            bytes
        });
    }
}
