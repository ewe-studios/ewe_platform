// Executor tests: the hand-rolled cooperative executor drives channel-bound
// futures. The synchronous tests need no runtime; the async-runtime tests use
// valtron (`#[valtron_test]`) instead of tokio.

use std::time::Duration;

use ewe_channels::{executor, mspc};
use foundation_core::valtron::{
    execute, from_future, send, sleep_async, valtron_test, StreamIteratorExt,
};

#[test]
fn can_execute_a_task_without_an_async_runtime_with_scheduled_serve() {
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create::<String>();

    let (mut sr, rr) = mspc::create::<String>();

    let mut sender_clone = sender.clone();
    executor
        .schedule(rr.clone(), move |item| async move {
            sender_clone
                .async_send(item.unwrap())
                .await
                .expect("to have sent message");
        })
        .expect("should have scheduled task");

    executor
        .spawn(async move {
            sender
                .async_send(String::from("second"))
                .await
                .expect("to have sent message");
        })
        .expect("should have scheduled task");

    // send on first channel
    sr.try_send(String::from("new text")).unwrap();

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.block_receive().unwrap();
    assert_eq!(String::from("new text"), recv_message);

    let recv_message = receiver.block_receive().unwrap();
    assert_eq!(String::from("second"), recv_message);
}

#[test]
fn can_execute_a_task_without_an_async_runtime() {
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create::<String>();

    let (mut sr, rr) = mspc::create::<String>();
    executor
        .schedule(rr.clone(), move |item| async move {
            sender
                .async_send(item.unwrap())
                .await
                .expect("to have sent message");
        })
        .expect("should have scheduled task");

    // send on first channel
    sr.try_send(String::from("new text")).unwrap();

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.block_receive().unwrap();
    assert_eq!(String::from("new text"), recv_message);
}

#[test]
fn can_execute_work_without_a_receiver_and_no_async_runtime() {
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create::<String>();

    executor
        .spawn(async move {
            sender.try_send(String::from("new text")).unwrap();
        })
        .expect("should have scheduled task");

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.try_receive().unwrap();
    assert_eq!(String::from("new text"), recv_message);
}

#[valtron_test(threads = 2)]
async fn can_execute_a_task_with_an_async_runtime() {
    let (mut servicer, executor) = executor::create();
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut sr, rr) = mspc::create::<String>();

    execute(
        from_future(async move {
            sleep_async(Duration::from_millis(100), &foundation_core::valtron::Ctx::host()).await;

            executor
                .schedule(rr, move |item| async move {
                    sender
                        .async_send(item.unwrap())
                        .await
                        .expect("should have sent result");
                })
                .expect("should have scheduled task");
        }),
        None,
    )
    .expect("should have scheduled on the valtron pool")
    .into_collect_future()
    .await;

    // send on first channel
    sr.try_send(String::from("new text")).unwrap();

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.try_receive().unwrap();
    assert_eq!(String::from("new text"), recv_message);
}

#[valtron_test(threads = 2)]
async fn can_execute_work_without_a_receiver_and_a_async_runtime() {
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create::<String>();

    execute(
        from_future(async move {
            sleep_async(Duration::from_millis(100), &foundation_core::valtron::Ctx::host()).await;

            executor
                .spawn(async move {
                    sender.try_send(String::from("new text")).unwrap();
                })
                .expect("should have scheduled task");
        }),
        None,
    )
    .expect("should have scheduled on the valtron pool")
    .into_collect_future()
    .await;

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.try_receive().unwrap();
    assert_eq!(String::from("new text"), recv_message);
}

#[valtron_test(threads = 2)]
async fn can_execution_in_runtime_while_waiting_for_reciever_lead_to_panic() {
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create();

    let (mut sr, rr) = mspc::create::<String>();

    executor
        .schedule(rr.clone(), move |item| {
            send(from_future(async move {
                sleep_async(Duration::from_millis(100), &foundation_core::valtron::Ctx::host()).await;
                sender
                    .async_send(item.unwrap())
                    .await
                    .expect("should have sent result");
            }))
            .expect("should have scheduled background task");
            async {}
        })
        .expect("should have scheduled task");

    // send on first channel
    sr.try_send(String::from("new text")).unwrap();

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.block_receive().unwrap();

    assert_eq!(String::from("new text"), recv_message);
}

#[valtron_test(threads = 2)]
async fn can_execute_with_closed_sender_with_runtime_and_no_panic() {
    let (mut sender, receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create();

    let (sr, rr) = mspc::create::<String>();

    executor
        .schedule(rr.clone(), move |item| {
            send(from_future(async move {
                sleep_async(Duration::from_millis(100), &foundation_core::valtron::Ctx::host()).await;
                if let Ok(value) = item {
                    sender
                        .async_send(value)
                        .await
                        .expect("should have sent result");
                }
            }))
            .expect("should have scheduled background task");
            async {}
        })
        .expect("should have scheduled task");

    // send on first channel
    drop(sr);
    drop(receiver);

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));
}

#[valtron_test(threads = 2)]
async fn can_execute_with_async_serve_with_async_runtime() {
    let (mut sender, mut receiver) = mspc::create::<String>();

    let (mut servicer, executor) = executor::create();

    let (mut sr, rr) = mspc::create::<String>();

    executor
        .schedule(rr.clone(), move |item| {
            send(from_future(async move {
                sleep_async(Duration::from_millis(500), &foundation_core::valtron::Ctx::host()).await;
                sender
                    .async_send(item.unwrap())
                    .await
                    .expect("should have sent result");
            }))
            .expect("should have scheduled background task");
            async {}
        })
        .expect("should have scheduled task");

    // send on first channel
    sr.try_send(String::from("new text")).unwrap();

    assert!(matches!(
        servicer.schedule_serve(),
        executor::ExecutorResult::Ok(())
    ));

    // expect to receive from second channel
    let recv_message = receiver.block_receive().unwrap();

    assert_eq!(String::from("new text"), recv_message);
}
