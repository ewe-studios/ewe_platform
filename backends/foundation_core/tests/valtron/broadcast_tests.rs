//! N-way broadcast combinator tests (F45 N-way follow-on).
//!
//! WHY: `broadcast` / `broadcast_lossy` fan one task's matched `Ready` values out
//! to N observer branches. These tests pin the two policies: zero-loss
//! slowest-gates lockstep (`broadcast`) vs. loud drop-on-full (`broadcast_lossy`),
//! plus predicate filtering and the dropped-observer escape hatch.
//!
//! WHAT: the continuation and observers are driven **manually** (no pool), so the
//! tests are plain `#[test]` and run identically under `multi` and single/wasm.
//!
//! HOW: `drive_lockstep` interleaves "drain observers, then step continuation" so a
//! depth-bounded branch queue never permanently backs up — the only correct way to
//! consume a backpressured broadcast.

use foundation_core::valtron::{NoAction, Stream, TaskIteratorExt, TaskStatus};

/// A finite task source yielding a scripted list of `TaskStatus` values.
struct BroadcastTestTask {
    items: Vec<TaskStatus<u32, String, NoAction>>,
    index: usize,
}

impl BroadcastTestTask {
    fn new(items: Vec<TaskStatus<u32, String, NoAction>>) -> Self {
        Self { items, index: 0 }
    }
}

impl Iterator for BroadcastTestTask {
    type Item = TaskStatus<u32, String, NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

/// Drive a broadcast continuation + its observers in lockstep to completion.
///
/// `drain[i] == false` leaves branch `i` un-drained during the loop (used only for
/// the lossy test, which never parks). Returns the values the continuation
/// forwarded downstream, each branch's received values, and whether the source
/// ever parked (`TaskStatus::Depends`).
///
/// # Panics
/// Never panics.
fn drive_lockstep<C, O>(
    mut cont: C,
    mut observers: Vec<O>,
    drain: &[bool],
) -> (Vec<u32>, Vec<Vec<u32>>, bool)
where
    C: Iterator<Item = TaskStatus<u32, String, NoAction>>,
    O: Iterator<Item = Stream<u32, String>>,
{
    let n = observers.len();
    let mut branches: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut forwarded: Vec<u32> = Vec::new();
    let mut parked = false;

    loop {
        // Free slots first so a parked source can advance (drain designated branches).
        for (i, obs) in observers.iter_mut().enumerate() {
            if !drain.get(i).copied().unwrap_or(true) {
                continue;
            }
            while let Some(stream) = obs.next() {
                match stream {
                    Stream::Next(v) => branches[i].push(v),
                    _ => break, // Wait (empty-open) or exhausted for now
                }
            }
        }
        match cont.next() {
            Some(TaskStatus::Ready(v)) => forwarded.push(v),
            Some(TaskStatus::Depends(_)) => parked = true, // parked on a full branch
            Some(_) => {}                                  // Pending/Init/etc. — keep driving
            None => break,                                 // source exhausted, queues closed
        }
    }

    // Final drain after the continuation closed the branch queues.
    for (i, obs) in observers.iter_mut().enumerate() {
        while let Some(stream) = obs.next() {
            match stream {
                Stream::Next(v) => branches[i].push(v),
                _ => break,
            }
        }
    }

    (forwarded, branches, parked)
}

/// `broadcast` delivers every matched value to all N branches (zero loss), with
/// slack (queue_size > 1) so no parking is required.
#[test]
fn test_broadcast_fans_to_all_branches() {
    let task = BroadcastTestTask::new(vec![
        TaskStatus::Ready(1),
        TaskStatus::Ready(2),
        TaskStatus::Ready(3),
    ]);

    let (observers, cont) = task.broadcast(3, |_v| true, 10);
    let (forwarded, branches, _) = drive_lockstep(cont, observers, &[true, true, true]);

    // Continuation forwards every source value, in order.
    assert_eq!(forwarded, vec![1, 2, 3]);
    // Every branch received every value.
    for (i, b) in branches.iter().enumerate() {
        assert_eq!(b, &vec![1, 2, 3], "branch {i} must receive all values");
    }
}

/// Only predicate-matched values are copied to branches; non-matched values are
/// still forwarded downstream.
#[test]
fn test_broadcast_predicate_filters() {
    let task = BroadcastTestTask::new(vec![
        TaskStatus::Ready(1),
        TaskStatus::Ready(2),
        TaskStatus::Ready(3),
        TaskStatus::Ready(4),
    ]);

    // Only even values go to the branches.
    let (observers, cont) = task.broadcast(2, |v| *v % 2 == 0, 10);
    let (forwarded, branches, _) = drive_lockstep(cont, observers, &[true, true]);

    assert_eq!(forwarded, vec![1, 2, 3, 4], "all values forwarded downstream");
    for b in &branches {
        assert_eq!(b, &vec![2, 4], "branches only receive matched (even) values");
    }
}

/// `broadcast` with `queue_size = 1` delivers zero loss across all branches when
/// driven in lockstep (drain-then-step keeps the depth-1 queues from backing up).
#[test]
fn test_broadcast_backpressure_lockstep_zero_loss() {
    let task = BroadcastTestTask::new(vec![
        TaskStatus::Ready(10),
        TaskStatus::Ready(20),
        TaskStatus::Ready(30),
        TaskStatus::Ready(40),
    ]);

    let (observers, cont) = task.broadcast(3, |_v| true, 1);
    let (forwarded, branches, _) = drive_lockstep(cont, observers, &[true, true, true]);

    assert_eq!(forwarded, vec![10, 20, 30, 40]);
    for (i, b) in branches.iter().enumerate() {
        assert_eq!(
            b,
            &vec![10, 20, 30, 40],
            "branch {i}: zero loss under strict lockstep"
        );
    }
}

/// Pop the next `Stream::Next` value from a branch observer, or `None` on
/// empty-open (`Wait`) / closed.
fn pop_next<O: Iterator<Item = Stream<u32, String>>>(obs: &mut O) -> Option<u32> {
    match obs.next() {
        Some(Stream::Next(v)) => Some(v),
        _ => None,
    }
}

/// Deterministic proof of the slowest-gates park + resume: with two depth-1
/// branches, the second matched value must park the source (`Depends`) because a
/// branch is full, then resume and deliver once both branches drain — zero loss.
#[test]
fn test_broadcast_parks_on_full_branch_and_resumes() {
    let task = BroadcastTestTask::new(vec![TaskStatus::Ready(10), TaskStatus::Ready(20)]);
    let (mut observers, mut cont) = task.broadcast(2, |_v| true, 1);

    // Step 1: branches empty → deliver 10 to both, forward Ready(10).
    assert!(
        matches!(cont.next(), Some(TaskStatus::Ready(10))),
        "first value delivered + forwarded"
    );

    // Step 2: both branches now full → the source parks (does NOT drop 20).
    assert!(
        matches!(cont.next(), Some(TaskStatus::Depends(_))),
        "a full branch must park the source, not drop the value"
    );
    // Stepping again while still full re-parks (idempotent, no double-delivery).
    assert!(matches!(cont.next(), Some(TaskStatus::Depends(_))));

    // Drain both branches → free a slot on every branch.
    assert_eq!(pop_next(&mut observers[0]), Some(10));
    assert_eq!(pop_next(&mut observers[1]), Some(10));

    // Step 3: all branches have vacancy → deliver the stashed 20, forward Ready(20).
    assert!(
        matches!(cont.next(), Some(TaskStatus::Ready(20))),
        "resumes and delivers the stashed value once branches drain"
    );

    // Both branches received 20 exactly once — zero loss, no duplicate.
    assert_eq!(pop_next(&mut observers[0]), Some(20));
    assert_eq!(pop_next(&mut observers[1]), Some(20));

    // Source exhausted.
    assert!(cont.next().is_none());
}

/// A dropped observer closes its branch; the source stops copying to it but keeps
/// forwarding and delivering to the survivors (never a deadlock).
#[test]
fn test_broadcast_dropped_observer_keeps_forwarding() {
    let task = BroadcastTestTask::new(vec![
        TaskStatus::Ready(1),
        TaskStatus::Ready(2),
        TaskStatus::Ready(3),
    ]);

    // depth-1 so a live-but-undrained branch WOULD deadlock — proving the dropped
    // branch is genuinely skipped, not merely tolerated.
    let (mut observers, cont) = task.broadcast(3, |_v| true, 1);
    let dropped = observers.remove(1); // drop the middle branch
    drop(dropped);

    let (forwarded, branches, _) = drive_lockstep(cont, observers, &[true, true]);

    assert_eq!(forwarded, vec![1, 2, 3], "source keeps forwarding");
    for (i, b) in branches.iter().enumerate() {
        assert_eq!(b, &vec![1, 2, 3], "survivor branch {i} still receives all");
    }
}

/// `broadcast_lossy` never stalls the source: an un-drained (slow) branch loses
/// values, while a drained branch still receives everything.
#[test]
fn test_broadcast_lossy_never_stalls() {
    let task = BroadcastTestTask::new(vec![
        TaskStatus::Ready(1),
        TaskStatus::Ready(2),
        TaskStatus::Ready(3),
        TaskStatus::Ready(4),
        TaskStatus::Ready(5),
    ]);

    // Branch 0 is drained every step; branch 1 is never drained during the loop.
    let (observers, cont) = task.broadcast_lossy(2, |_v| true, 1);
    let (forwarded, branches, parked) = drive_lockstep(cont, observers, &[true, false]);

    assert_eq!(forwarded, vec![1, 2, 3, 4, 5]);
    assert!(!parked, "lossy broadcast must never park the source");
    assert_eq!(
        branches[0],
        vec![1, 2, 3, 4, 5],
        "the drained branch receives every value"
    );
    // The starved branch keeps only what its depth-1 queue could hold (drop-oldest),
    // so it has at most 1 leftover value and definitely lost some.
    assert!(
        branches[1].len() <= 1,
        "starved lossy branch drops values (got {:?})",
        branches[1]
    );
}

/// `n = 0` broadcast is a degenerate pass-through: no branches, all source values
/// still forwarded, no panic.
#[test]
fn test_broadcast_zero_branches_passthrough() {
    let task = BroadcastTestTask::new(vec![
        TaskStatus::Ready(7),
        TaskStatus::Ready(8),
    ]);

    let (observers, cont) = task.broadcast(0, |_v| true, 4);
    assert_eq!(observers.len(), 0);
    let (forwarded, branches, parked) = drive_lockstep(cont, observers, &[]);

    assert_eq!(forwarded, vec![7, 8]);
    assert!(branches.is_empty());
    assert!(!parked, "no branches means nothing can ever be full");
}

/// `queue_size = 0` is clamped to 1 (a 0-capacity queue is invalid) — must not
/// panic, and behaves as strict lockstep.
#[test]
fn test_broadcast_queue_size_zero_clamped() {
    let task = BroadcastTestTask::new(vec![TaskStatus::Ready(1), TaskStatus::Ready(2)]);

    let (observers, cont) = task.broadcast(2, |_v| true, 0);
    let (forwarded, branches, _) = drive_lockstep(cont, observers, &[true, true]);

    assert_eq!(forwarded, vec![1, 2]);
    for b in &branches {
        assert_eq!(b, &vec![1, 2]);
    }
}
