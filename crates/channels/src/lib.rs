// Crate implementing the Engineering Principles of Channels

use futures::{channel::mpsc, stream::FusedStream, SinkExt};
use std::{
    borrow::{Borrow, BorrowMut},
    sync,
};
use thiserror::Error;

type Result<T> = anyhow::Result<T, ChannelError>;

#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Channel has being closed or not initialized")]
    Closed,

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
    src: Option<mpsc::UnboundedSender<T>>,
}

impl<T> SendChannel<T> {
    fn new(src: mpsc::UnboundedSender<T>) -> Self {
        Self { src: Some(src) }
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(channel) = self.src.take() {
            channel.close_channel();
            return Ok(());
        } else {
            return Err(ChannelError::Closed);
        }
    }

    pub fn try_send(&mut self, t: T) -> Result<()> {
        match &mut self.src {
            Some(src) => match src.start_send(t) {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err)),
            },
            None => Err(ChannelError::Closed),
        }
    }
}

pub struct ReceiveChannel<T> {
    src: Option<mpsc::UnboundedReceiver<T>>,
}

impl<T> ReceiveChannel<T> {
    fn new(src: mpsc::UnboundedReceiver<T>) -> Self {
        Self { src: Some(src) }
    }

    pub fn try_receive(&mut self) -> Result<T> {
        match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => match src.try_next() {
                Ok(None) => {
                    // take the contents
                    _ = self.src.take();

                    // return as closed.
                    Err(ChannelError::Closed)
                }
                Ok(maybe_item) => {
                    if let Some(item) = maybe_item {
                        return Ok(item);
                    }
                    return Err(ChannelError::ReceivedNoData);
                }
                Err(err) => Err(ChannelError::RecevieFailed((err))),
            },
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{create, ChannelError};
    use std::time::Duration;

    #[test]
    fn should_be_able_to_close_a_send_channel() {
        let (mut sender, mut receiver) = create::<String>();

        sender.close();

        let err = receiver.try_receive();
        assert!(matches!(err, Err(ChannelError::Closed)));
    }

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
