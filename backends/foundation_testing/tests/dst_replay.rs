//! A DST replay gives the same trace in two processes (spec 84 T6).
//!
//! The test starts its own binary twice. Each child prints the trace of a
//! replay with the same seed, and the parent compares the two traces.

use std::process::Command;

/// The seed that both child processes use.
const SEED: u64 = 7;

/// The marker that a child prints before one trace line.
const MARKER: &str = "DSTTRACE ";

/// Extract the trace lines from a child's output.
fn trace_from(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| line.strip_prefix(MARKER).map(str::to_string))
        .collect()
}

#[test]
fn same_seed_replays_in_two_processes() {
    if std::env::var("DST_REPLAY_CHILD").is_ok() {
        for line in foundation_testing::dst::replay(SEED) {
            println!("{MARKER}{line}");
        }
        return;
    }

    let exe = std::env::current_exe().expect("the test binary path");
    let run = || {
        let output = Command::new(&exe)
            .env("DST_REPLAY_CHILD", "1")
            .args([
                "--exact",
                "same_seed_replays_in_two_processes",
                "--nocapture",
            ])
            .output()
            .expect("start the child test binary");
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        trace_from(&output.stdout)
    };

    let first = run();
    let second = run();

    assert!(!first.is_empty(), "the child printed no trace");
    assert_eq!(
        first.len(),
        foundation_testing::dst::REPLAY_IDS + foundation_testing::dst::REPLAY_WAITS,
        "the trace has one line per ID and per wait"
    );
    assert_eq!(first, second, "one seed must replay the run exactly");
}

#[test]
fn different_seeds_differ() {
    assert_ne!(
        foundation_testing::dst::replay(7),
        foundation_testing::dst::replay(8)
    );
}
