// Crate implementing the Engineering Principles of Channels

use futures::{channel::mpsc, SinkExt};
use std::{borrow::BorrowMut, sync};
use thiserror::Error;

type Result<T> = anyhow::Result<T, ChannelError>;

#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Failed to send message due to: {0}")]
    SendFailed(mpsc::SendError),

    #[error("Failed to receive message due to: {0}")]
    RecevieFailed(mpsc::TryRecvError),

    #[error("Channel sent nothing, possibly closed")]
    ReceivedNoData,
}

pub fn create<T>() -> (SendChannel<T>, ReceiveChannel<T>) {
    let (tx, rx) = mpsc::unbounded::<T>();
    let sender = SendChannel::new(tx);
    let receiver = ReceiveChannel::new(rx);
    (sender, receiver)
}

enum ChannelMessage<T> {
    Next(T),
    End,
}

pub struct SendChannel<T> {
    src: mpsc::UnboundedSender<T>,
}

impl<T> SendChannel<T> {
    fn new(src: mpsc::UnboundedSender<T>) -> Self {
        Self { src }
    }

    fn try_send(&mut self, t: T) -> Result<()> {
        match self.src.start_send(t) {
            Ok(()) => Ok(()),
            Err(err) => Err(ChannelError::SendFailed(err)),
        }
    }
}

pub struct ReceiveChannel<T> {
    src: mpsc::UnboundedReceiver<T>,
}

impl<T> ReceiveChannel<T> {
    fn new(src: mpsc::UnboundedReceiver<T>) -> Self {
        Self { src }
    }

    fn try_receive(&mut self) -> Result<T> {
        match self.src.try_next() {
            Ok(maybe_item) => {
                if let Some(item) = maybe_item {
                    return Ok(item);
                }
                return Err(ChannelError::ReceivedNoData);
            }
            Err(err) => Err(ChannelError::RecevieFailed((err))),
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::create;
    use std::time::Duration;

    #[test]
    fn should_be_able_to_create_and_send_string_with_channel() {
        let (mut sender, mut receiver) = create::<String>();

        let message = String::from("new text");

        sender.try_send(message.clone()).unwrap();

        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(message, recv_message);
    }

    #[tokio::test]
    async fn should_be_able_to_send_channel_into_another_thread() {
        let (mut sender, mut receiver) = create::<String>();

        tokio::spawn(async move {
            sender.try_send(String::from("new text")).unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
        .await;

        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }
}
