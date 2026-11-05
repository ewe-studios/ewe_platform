// Multi-producer single-consumer channel tests. The async send/receive paths
// run under the valtron runtime (`#[valtron_test]`) instead of tokio.

use std::time::Duration;

use ewe_channels::mspc::{create, ChannelError};
use foundation_core::valtron::{
    execute, from_future, sleep_async, valtron_test, StreamIteratorExt,
};

#[test]
fn should_be_able_to_close_a_send_channel() {
    let (mut sender, mut receiver) = create::<String>();

    sender.close().expect("should have closed");

    let err = receiver.try_receive();
    assert!(matches!(
        err,
        Err(trace) if matches!(trace.current_context(), ChannelError::Closed)
    ));
}

#[test]
fn should_be_able_to_create_and_send_string_with_channel() {
    let (mut sender, mut receiver) = create::<String>();

    let message = String::from("new text");

    sender.try_send(message.clone()).unwrap();

    let recv_message = receiver.try_receive().unwrap();
    assert_eq!(message, recv_message);
}

#[test]
fn should_be_able_to_block_send_and_receive_with_channel() {
    let (mut sender, mut receiver) = create::<String>();

    sender
        .block_send(String::from("new text"))
        .expect("should be able to block send");

    let recv_message = receiver
        .block_receive()
        .expect("should have received response");

    assert_eq!(String::from("new text"), recv_message);
}

#[valtron_test]
async fn should_be_able_to_send_and_receive_async_with_channel() {
    let (mut sender, mut receiver) = create::<String>();

    sender
        .async_send(String::from("new text"))
        .await
        .expect("should have completed");

    let recv_message = receiver
        .async_receive()
        .await
        .expect("should have received response");

    assert_eq!(String::from("new text"), recv_message);
}

#[valtron_test(threads = 2)]
async fn should_be_able_to_send_channel_into_another_thread() {
    let (mut sender, mut receiver) = create::<String>();

    execute(
        from_future(async move {
            sender.try_send(String::from("new text")).unwrap();
            sleep_async(Duration::from_millis(100)).await;
        }),
        None,
    )
    .expect("should have scheduled on the valtron pool")
    .into_collect_future()
    .await;

    let recv_message = receiver.try_receive().unwrap();
    assert_eq!(String::from("new text"), recv_message);
}
