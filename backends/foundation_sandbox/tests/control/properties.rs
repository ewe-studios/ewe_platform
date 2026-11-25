//! Property tests: the input identity and the same seed.

use std::sync::Arc;

use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::testing::{spec, ScriptedSlave, SimDriver};
use foundation_sandbox::shared::control::SandboxProvider;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;
use proptest::prelude::*;

use super::drain_create;
use super::support;

proptest! {
    #![proptest_config(support::property_config(0x5eed_0002))]

    /// The bytes that the slave applied equal the bytes that the caller wrote,
    /// for any chunk sizes.
    #[test]
    fn stdin_model(chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..128), 0..8)) {
        let _serial = support::serial();
        let (ctx, _clock) = support::ctx(80);
        let slave = Arc::new(ScriptedSlave::new());
        slave.with_exec_events(vec![
            ExecEventKind::Started { pid: 1 },
            ExecEventKind::Exit { code: Some(0), signal: None },
        ]);
        let driver = Arc::new(SimDriver::new(support::driver_id()));
        let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
        let handle = support::block_on(manager.create(spec())).expect("create");
        let mut command = Command::new(["cat"]);
        command.stdin = foundation_sandbox::shared::command::StdinMode::Piped;
        let mut exec = support::block_on(handle.exec(command)).expect("exec");
        let mut stdin = exec.stdin().expect("stdin");
        let expected = chunks.concat();
        let mut input = chunks.clone();
        support::block_on(async move {
            for chunk in input.drain(..) {
                stdin.write(&chunk).await.expect("write");
            }
            stdin.close().await.expect("close");
        });
        let applied = slave.applied_input();
        let mut bytes = Vec::new();
        let mut eof = false;
        let mut expected_seq = 0_u64;
        for record in applied {
            prop_assert_eq!(record.seq, expected_seq);
            expected_seq += 1;
            bytes.extend_from_slice(&record.data);
            eof |= record.eof;
        }
        prop_assert_eq!(bytes, expected);
        prop_assert!(eof);
    }
}

#[test]
fn same_seed_same_trace() {
    let _serial = support::serial();
    assert_eq!(trace(7), trace(7));
}

fn trace(seed: u64) -> String {
    let (ctx, _clock) = support::ctx(seed);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![
        ExecEventKind::Started { pid: 3 },
        ExecEventKind::Stdout {
            data: b"a\n".to_vec(),
        },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let mut stream = manager.create_with_progress(spec());
    let (progress, result) = drain_create(&mut stream);
    let handle = result.expect("create");
    let output = support::block_on(handle.run(Command::new(["echo"]))).expect("run");
    format!(
        "{progress:?}|{:?}|{:?}|{:?}",
        slave.calls(),
        output,
        handle.info()
    )
}
