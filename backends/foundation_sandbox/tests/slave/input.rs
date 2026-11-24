//! The input scenarios: order, repeats, gaps, the end of the input and the
//! full queue.

use std::time::Duration;

use proptest::prelude::*;
use proptest::test_runner::RngSeed;

use foundation_sandbox::shared::error::{SandboxError, SlaveErrorCode};
use foundation_sandbox::shared::rpc::messages::{ExecInputRequest, ExecInputResponse};

use super::support::{
    drain, exec_request_stdin, exit_of, hex, plain_prepare, slave_code, stdout_of, Caller,
    SlaveProcess,
};

/// Input reaches the program in order, one time.
#[test]
fn input_in_order() {
    let slave = SlaveProcess::start("input-order", None, 4);
    slave.prepare(&plain_prepare("/tmp"));
    let stream = slave
        .caller
        .exec(exec_request_stdin(&hex(300), &["cat"]))
        .expect("the program starts");
    let next = |seq: u64, data: &[u8], eof: bool| {
        slave
            .caller
            .exec_input(ExecInputRequest {
                exec_id: hex(300),
                seq,
                data: data.to_vec(),
                eof,
            })
            .map(|answer| answer.next_seq)
    };
    assert_eq!(next(0, b"ab", false).expect("chunk 0"), 1);
    assert_eq!(next(1, b"cd", false).expect("chunk 1"), 2);
    assert_eq!(next(1, b"cd", false).expect("a repeat"), 2);
    assert_eq!(next(2, b"", true).expect("the end"), 3);
    let events = drain(stream);
    assert_eq!(stdout_of(&events), b"abcd");
    assert_eq!(exit_of(&events), (Some(0), None));
}

/// A repeat changes nothing.
#[test]
fn input_duplicate() {
    let slave = SlaveProcess::start("input-duplicate", None, 4);
    slave.prepare(&plain_prepare("/tmp"));
    let stream = slave
        .caller
        .exec(exec_request_stdin(&hex(301), &["cat"]))
        .expect("the program starts");
    let answer = feed(&slave.caller, &hex(301), 0, b"x", false).expect("the chunk");
    assert_eq!(answer.next_seq, 1);
    let answer = feed(&slave.caller, &hex(301), 0, b"x", false).expect("the repeat");
    assert_eq!(answer.next_seq, 1);
    feed(&slave.caller, &hex(301), 1, b"", true).expect("the end");
    let events = drain(stream);
    assert_eq!(stdout_of(&events), b"x");
}

/// A gap is refused.
#[test]
fn input_gap() {
    let slave = SlaveProcess::start("input-gap", None, 4);
    slave.prepare(&plain_prepare("/tmp"));
    let stream = slave
        .caller
        .exec(exec_request_stdin(&hex(302), &["cat"]))
        .expect("the program starts");
    let error = feed(&slave.caller, &hex(302), 1, b"x", false).expect_err("the gap is refused");
    assert_eq!(sandbox_error_code(&error), Some("sequence_gap".to_string()));
    feed(&slave.caller, &hex(302), 0, b"", true).expect("the end");
    let events = drain(stream);
    assert_eq!(stdout_of(&events), b"");
    assert_eq!(exit_of(&events), (Some(0), None));
}

/// Data after `eof` is refused.
#[test]
fn input_after_eof() {
    let slave = SlaveProcess::start("input-after-eof", None, 4);
    slave.prepare(&plain_prepare("/tmp"));
    let stream = slave
        .caller
        .exec(exec_request_stdin(
            &hex(303),
            &["sh", "-c", "cat; sleep 60"],
        ))
        .expect("the program starts");
    feed(&slave.caller, &hex(303), 0, b"", true).expect("the end");
    let error =
        feed(&slave.caller, &hex(303), 1, b"x", false).expect_err("data after eof is refused");
    assert_eq!(
        slave_code(&error).map(|(code, _)| code),
        Some(SlaveErrorCode::BadRequest.to_string())
    );
    drop(stream);
}

/// A full input queue pushes back, and the same number succeeds later.
#[test]
fn input_queue_full_then_retry() {
    let slave = SlaveProcess::start("input-queue-full", None, 4);
    let mut prepare = plain_prepare("/tmp");
    prepare.limits.input_queue_bytes = 8 * 1024;
    prepare.limits.chunk_bytes = 4 * 1024;
    slave.prepare(&prepare);
    let stream = slave
        .caller
        .exec(exec_request_stdin(
            &hex(304),
            &["sh", "-c", "sleep 1; cat >/dev/null"],
        ))
        .expect("the program starts");
    let chunk = vec![b'z'; 4096];
    let mut full_at: Option<u64> = None;
    for seq in 0..64 {
        match feed(&slave.caller, &hex(304), seq, &chunk, false) {
            Ok(_) => {}
            Err(error) => {
                assert_eq!(
                    sandbox_error_code(&error),
                    Some("input_queue_full".to_string()),
                    "the failure is the full queue"
                );
                full_at = Some(seq);
                break;
            }
        }
    }
    let full_at = full_at.expect("the queue became full");
    // The same number succeeds after the queue has room.
    let mut attempts = 0_usize;
    while feed(&slave.caller, &hex(304), full_at, &chunk, false).is_err() {
        attempts += 1;
        assert!(attempts < 500, "the same number did not succeed");
        std::thread::sleep(Duration::from_millis(20));
    }
    feed(&slave.caller, &hex(304), full_at + 1, b"", true).expect("the end");
    let _ = drain(stream);
}

proptest! {
    #![proptest_config(ProptestConfig {
        rng_seed: RngSeed::Fixed(0x5eed_0002),
        cases: 8,
        .. ProptestConfig::default()
    })]

    /// A random series of input calls matches a model that appends on `Apply`.
    #[test]
    fn input_model(
            ops in proptest::collection::vec(
                (0_u8..3, proptest::collection::vec(any::<u8>(), 0..8), any::<bool>()),
                1..25,
            )
        ) {
            let slave = SlaveProcess::start("input-model", None, 4);
            slave.prepare(&plain_prepare("/tmp"));
            let exec_id = hex(400);
            let stream = slave
                .caller
                .exec(exec_request_stdin(&exec_id, &["cat"]))
                .expect("the program starts");
            let mut model = Vec::new();
            let mut next = 0_u64;
            let mut ended = false;
            for (kind, data, eof) in ops {
                if ended {
                    break;
                }
                let seq = match kind {
                    0 => next,
                    1 => next.saturating_sub(1),
                    _ => next + 1,
                };
                let result = feed(&slave.caller, &exec_id, seq, &data, eof);
                match kind {
                    0 => {
                        let answer = result.expect("the next number is accepted");
                        model.extend_from_slice(&data);
                        next += 1;
                        prop_assert_eq!(answer.next_seq, next);
                        if eof {
                            ended = true;
                        }
                    }
                    1 => {
                        let answer = result.expect("the old number is accepted");
                        prop_assert_eq!(answer.next_seq, next);
                    }
                    _ => {
                        prop_assert!(
                            matches!(
                                result.map_err(|error| sandbox_error_code(&error)),
                                Err(Some(code)) if code == "sequence_gap"
                            ),
                            "a gap is refused"
                        );
                    }
                }
            }
            if !ended {
                feed(&slave.caller, &exec_id, next, b"", true).expect("the end");
            }
        let events = drain(stream);
        prop_assert_eq!(stdout_of(&events), model);
    }
}

fn feed(
    caller: &Caller,
    exec_id: &str,
    seq: u64,
    data: &[u8],
    eof: bool,
) -> Result<ExecInputResponse, foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError>>
{
    caller.exec_input(ExecInputRequest {
        exec_id: exec_id.to_string(),
        seq,
        data: data.to_vec(),
        eof,
    })
}

/// The `SandboxError` variant name of a Connect failure, for a shared failure.
fn sandbox_error_code(
    error: &foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError>,
) -> Option<String> {
    match super::support::sandbox_error(error) {
        SandboxError::SequenceGap => Some("sequence_gap".to_string()),
        SandboxError::InputQueueFull => Some("input_queue_full".to_string()),
        SandboxError::SlaveError { code, .. } => Some(code.to_string()),
        _ => None,
    }
}
