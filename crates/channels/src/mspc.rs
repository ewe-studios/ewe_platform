// Crate implementing the Engineering Principles of Channels

use std::sync::{self, Arc};

use async_channel;
use crossbeam::atomic;
use thiserror::Error;

pub type Result<T> = anyhow::Result<T, ChannelError>;

#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Channel has being closed or not initialized")]
    Closed,

    #[error("Failed to send message due to: {0}")]
    SendFailed(String),

    #[error("Failed to receive message due to: {0}")]
    ReceiveFailed(async_channel::TryRecvError),

    #[error("Channel sent nothing, possibly closed")]
    ReceivedNoData,
}

pub fn create<T>() -> (SendChannel<T>, ReceiveChannel<T>) {
    let (tx, rx) = async_channel::unbounded::<T>();
    let sender = SendChannel::new(tx);
    let receiver = ReceiveChannel::new(rx);
    (sender, receiver)
}

pub struct ChannelGroup<E>(pub SendChannel<E>, pub ReceiveChannel<E>);

impl<E> Clone for ChannelGroup<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<E> ChannelGroup<E> {
    pub fn new() -> Self {
        let (sender, receiver) = create::<E>();
        Self(sender, receiver)
    }
}

pub trait SendOnlyChannel<T> {
    fn try_send(&mut self, t: T) -> Result<()>;
}

struct SendOnlyWrapper<T> {
    channel: SendChannel<T>,
}

impl<T> Clone for SendOnlyWrapper<T> {
    fn clone(&self) -> Self {
        Self {
            channel: self.channel.clone(),
        }
    }
}

impl<T> SendOnlyChannel<T> for SendOnlyWrapper<T> {
    fn try_send(&mut self, t: T) -> Result<()> {
        self.channel.try_send(t)
    }
}

pub struct SendChannel<T> {
    src: Option<async_channel::Sender<T>>,
}

impl<T> Clone for SendChannel<T> {
    fn clone(&self) -> Self {
        Self {
            src: self.src.clone(),
        }
    }
}

impl<T: 'static> SendChannel<T> {
    pub fn send_only(self) -> Box<dyn SendOnlyChannel<T>> {
        Box::new(SendOnlyWrapper { channel: self })
    }
}

impl<T> SendChannel<T> {
    fn new(src: async_channel::Sender<T>) -> Self {
        Self { src: Some(src) }
    }

    pub fn pending_message_count(&mut self) -> Result<usize> {
        match &mut self.src {
            Some(src) => Ok(src.len()),
            None => Err(ChannelError::Closed),
        }
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(channel) = self.src.take() {
            drop(channel);
            return Ok(());
        } else {
            return Err(ChannelError::Closed);
        }
    }

    pub async fn async_send(&mut self, t: T) -> Result<()> {
        match &mut self.src {
            Some(src) => match src.send(t).await {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err.to_string())),
            },
            None => Err(ChannelError::Closed),
        }
    }

    /// [`SendChannel`].block_send() blocks the current thread till data is sent or
    /// an error received. This generally should not be used in WASM or non-blocking
    /// environments.
    pub fn block_send(&mut self, t: T) -> Result<()> {
        match &mut self.src {
            Some(src) => match src.send_blocking(t) {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err.to_string())),
            },
            None => Err(ChannelError::Closed),
        }
    }

    pub fn try_send(&mut self, t: T) -> Result<()> {
        match &mut self.src {
            Some(src) => match src.try_send(t) {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err.to_string())),
            },
            None => Err(ChannelError::Closed),
        }
    }
}

pub struct ReceiveChannel<T> {
    read_flag: Arc<atomic::AtomicCell<bool>>,
    src: Option<async_channel::Receiver<T>>,
}

impl<T> Clone for ReceiveChannel<T> {
    fn clone(&self) -> Self {
        Self {
            read_flag: self.read_flag.clone(),
            src: self.src.clone(),
        }
    }
}

impl<T> ReceiveChannel<T> {
    fn new(src: async_channel::Receiver<T>) -> Self {
        Self {
            src: Some(src),
            read_flag: sync::Arc::new(atomic::AtomicCell::new(false)),
        }
    }

    // if the [`RecieveChannel`] was ever read once then this
    // becomes true, its up to the user to decide how they fit
    // this into their logic.
    pub fn read_atleast_once(&self) -> Result<bool> {
        return Ok(self.read_flag.load());
    }

    pub fn is_empty(&mut self) -> Result<bool> {
        match &self.src {
            None => Err(ChannelError::Closed),
            Some(src) => Ok(src.is_empty()),
        }
    }

    pub fn closed(&mut self) -> Result<bool> {
        match &self.src {
            None => Err(ChannelError::Closed),
            Some(_) => Ok(false),
        }
    }

    /// [`ReceiveChannel`].block_receive() blocks the current thread till data is received or
    /// an error is seen. This generally should not be used in WASM or non-blocking
    /// environments.
    pub fn block_receive(&mut self) -> Result<T> {
        return match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => match src.recv_blocking() {
                Ok(maybe_item) => {
                    self.read_flag.store(true);
                    Ok(maybe_item)
                }
                Err(_) => self.close_channel(),
            },
        };
    }

    pub async fn async_receive(&mut self) -> Result<T> {
        match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => match src.recv().await {
                Ok(maybe_item) => {
                    self.read_flag.store(true);
                    Ok(maybe_item)
                }
                Err(_) => {
                    if src.is_closed() {
                        return self.close_channel();
                    }
                    Err(ChannelError::ReceivedNoData)
                }
            },
        }
    }

    pub fn try_receive(&mut self) -> Result<T> {
        match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => match src.try_recv() {
                Ok(maybe_item) => {
                    self.read_flag.store(true);
                    Ok(maybe_item)
                }
                Err(err) => match err {
                    async_channel::TryRecvError::Closed => self.close_channel(),
                    async_channel::TryRecvError::Empty => Err(ChannelError::ReceivedNoData),
                },
            },
        }
    }

    fn close_channel(&mut self) -> Result<T> {
        // remove the channel from the underlying slot
        _ = self.src.take();
        Err(ChannelError::Closed)
    }
}

#[cfg(test)]
mod tests {

    use crate::mspc::{create, ChannelError};
    use std::time::Duration;

    #[test]
    fn should_be_able_to_close_a_send_channel() {
        let (mut sender, mut receiver) = create::<String>();

        sender.close().expect("should have closed");

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
    async fn should_be_able_to_block_send_and_receive_with_channel() {
        let (mut sender, mut receiver) = create::<String>();

        sender
            .block_send(String::from("new text"))
            .expect("should be able to block send");

        let recv_message = receiver
            .block_receive()
            .expect("should have received response");

        assert_eq!(String::from("new text"), recv_message);
    }

    #[tokio::test]
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

    #[tokio::test]
    async fn should_be_able_to_send_channel_into_another_thread() {
        let (mut sender, mut receiver) = create::<String>();

        tokio::spawn(async move {
            sender.try_send(String::from("new text")).unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
        .await
        .expect("should have completed");

        let recv_message = receiver.try_receive().unwrap();
        assert_eq!(String::from("new text"), recv_message);
    }
}
