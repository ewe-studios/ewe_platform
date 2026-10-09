//! Tests for `MappedSender` + `FilterMapReceiver` (pipe_mapped.rs, F39).

use bytes::Bytes;
use foundation_core::valtron::{self as v, Pipe};

// ── MappedSender ───────────────────────────────────────────────────────────

#[test]
fn mapped_sender_roundtrips() {
    let (tx, rx) = Pipe::<String>::with_depth(4);
    let mapped = tx.map_to(|n: &u32| format!("item-{n}"));
    mapped.try_send(&1).unwrap();
    mapped.try_send(&2).unwrap();
    assert_eq!(rx.try_recv().unwrap(), "item-1");
    assert_eq!(rx.try_recv().unwrap(), "item-2");
}

#[test]
fn mapped_sender_stash_on_full() {
    // Depth=1: every fresh item also hits Full after stash drain.
    let (tx, rx) = Pipe::<String>::with_depth(1);
    let mapped = tx.map_to(|n: &u32| format!("val-{n}"));
    mapped.try_send(&1).unwrap();               // queue: [val-1]
    assert!(mapped.try_send(&2).is_err());      // stash: val-2
    assert_eq!(rx.try_recv().unwrap(), "val-1"); // drain
    assert!(mapped.try_send(&3).is_err());      // stash val-2 drains → queue [val-2]; val-3 → stash
    assert_eq!(rx.try_recv().unwrap(), "val-2"); // drain val-2
    assert!(mapped.try_send(&4).is_err());      // stash val-3 drains → queue [val-3]; val-4 → stash
    assert_eq!(rx.try_recv().unwrap(), "val-3"); // drain val-3
}

#[test]
fn mapped_sender_stash_depth2_multiple_items() {
    // Depth=2: stash drain + fresh item both fit.
    let (tx, rx) = Pipe::<String>::with_depth(2);
    let mapped = tx.map_to(|n: &u32| format!("X{n}"));

    mapped.try_send(&1).unwrap();          // queue: [X1]
    mapped.try_send(&2).unwrap();          // queue: [X1, X2]
    assert!(mapped.try_send(&3).is_err()); // stash: X3

    rx.try_recv().unwrap(); // drain X1
    rx.try_recv().unwrap(); // drain X2 — queue empty

    // Stash X3 drains + fresh X4 pushed → both fit (depth=2).
    mapped.try_send(&4).unwrap();
    assert_eq!(rx.try_recv().unwrap(), "X3");
    assert_eq!(rx.try_recv().unwrap(), "X4");
}

#[test]
fn mapped_sender_close() {
    let (tx, rx) = Pipe::<String>::with_depth(4);
    let mapped = tx.map_to(|n: &u32| format!("n{n}"));
    mapped.try_send(&1).unwrap();
    mapped.close();
    assert_eq!(rx.try_recv().unwrap(), "n1");
    assert_eq!(rx.try_recv(), Err(v::TryRecvError::Closed));
}

#[test]
fn mapped_sender_clone() {
    let (tx, rx) = Pipe::<String>::with_depth(8);
    let a = tx.map_to(|n: &u32| format!("c{n}"));
    let b = a.clone();
    a.try_send(&10).unwrap();
    b.try_send(&20).unwrap();
    assert_eq!(rx.try_recv().unwrap(), "c10");
    assert_eq!(rx.try_recv().unwrap(), "c20");
}

#[test]
fn mapped_sender_bytes_to_vec_roundtrip() {
    let (tx, rx) = Pipe::<Vec<u8>>::with_depth(4);
    let mapped = tx.map_to(|b: &Bytes| b.to_vec());
    mapped.try_send(&Bytes::from_static(b"hello")).unwrap();
    assert_eq!(rx.try_recv().unwrap(), b"hello");
}

// ── FilterMapReceiver ──────────────────────────────────────────────────────

#[test]
fn filter_map_passes_even() {
    // Filter: odd → None → skipped (item consumed, Empty returned).
    //         even → Some(n) → Ok(n).
    let (tx, rx) = Pipe::<u32>::with_depth(4);
    let filtered = rx.filter_map_to(|n: &u32| if *n % 2 == 0 { Some(*n) } else { None });

    tx.try_send(1).unwrap(); // → filtered → Empty
    assert_eq!(filtered.try_recv(), Err(v::TryRecvError::Empty));

    tx.try_send(2).unwrap(); // → Some(2)
    assert_eq!(filtered.try_recv().unwrap(), 2);

    tx.try_send(3).unwrap(); // → filtered → Empty
    assert_eq!(filtered.try_recv(), Err(v::TryRecvError::Empty));

    tx.try_send(4).unwrap(); // → Some(4)
    assert_eq!(filtered.try_recv().unwrap(), 4);

    // Nothing left.
    assert_eq!(filtered.try_recv(), Err(v::TryRecvError::Empty));
}

#[test]
fn filter_map_closed() {
    let (tx, rx) = Pipe::<u32>::with_depth(2);
    let filtered = rx.filter_map_to(|n: &u32| Some(*n));
    tx.try_send(1).unwrap();
    drop(tx);
    assert_eq!(filtered.try_recv().unwrap(), 1);
    assert_eq!(filtered.try_recv(), Err(v::TryRecvError::Closed));
}

#[test]
fn filter_map_empty() {
    let (_tx, rx) = Pipe::<u32>::with_depth(4);
    let filtered = rx.filter_map_to(|n: &u32| Some(*n));
    assert_eq!(filtered.try_recv(), Err(v::TryRecvError::Empty));
}

#[test]
fn filter_map_ws_binary_extraction() {
    #[derive(Debug, PartialEq)]
    enum Msg { Binary(Vec<u8>), Text(String), Ping(Vec<u8>) }

    let (tx, rx) = Pipe::<Msg>::with_depth(4);
    let body = rx.filter_map_to(|m: &Msg| match m {
        Msg::Binary(d) => Some(Bytes::from(d.clone())),
        _ => None,
    });

    tx.try_send(Msg::Text("hi".into())).unwrap();
    assert_eq!(body.try_recv(), Err(v::TryRecvError::Empty)); // filtered

    tx.try_send(Msg::Binary(b"data".to_vec())).unwrap();
    assert_eq!(body.try_recv().unwrap(), Bytes::from_static(b"data"));

    tx.try_send(Msg::Ping(b"pong".to_vec())).unwrap();
    assert_eq!(body.try_recv(), Err(v::TryRecvError::Empty)); // filtered
}

#[test]
fn filter_map_clone() {
    let (tx, rx) = Pipe::<u32>::with_depth(4);
    let a = rx.filter_map_to(|n: &u32| if *n > 5 { Some(*n) } else { None });
    let b = a.clone();

    tx.try_send(3).unwrap();
    assert_eq!(a.try_recv(), Err(v::TryRecvError::Empty));
    assert_eq!(b.try_recv(), Err(v::TryRecvError::Empty));

    tx.try_send(7).unwrap();
    assert_eq!(b.try_recv().unwrap(), 7);
}

// ── Combined round-trip ─────────────────────────────────────────────────────

#[test]
fn mapped_send_filter_recv_roundtrip() {
    let (tx, rx) = Pipe::<String>::with_depth(4);
    let out = tx.map_to(|b: &Bytes| String::from_utf8_lossy(b).to_string());
    let inp = rx.filter_map_to(|s: &String| Some(Bytes::from(s.clone())));

    out.try_send(&Bytes::from_static(b"alpha")).unwrap();
    out.try_send(&Bytes::from_static(b"beta")).unwrap();

    assert_eq!(inp.try_recv().unwrap(), Bytes::from_static(b"alpha"));
    assert_eq!(inp.try_recv().unwrap(), Bytes::from_static(b"beta"));
}

// Async path (`send().await` / `receive().await`) is a thin wrapper
// forwarding to Pipe's SendFuture / RecvFuture, which are tested by
// `pipe_primitive.rs`. The mapped adapter's async behavior is identical.
