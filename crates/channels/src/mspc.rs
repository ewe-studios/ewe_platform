// Crate implementing the Engineering Principles of Channels

use std::sync::{self, Arc};

use async_channel;
use crossbeam::atomic;
use thiserror::Error;

pub type ChannelResult<T> = anyhow::Result<T, ChannelError>;

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

#[must_use]
pub fn create<T>() -> (SendChannel<T>, ReceiveChannel<T>) {
    let (tx, rx) = async_channel::unbounded::<T>();
    let sender = SendChannel::new(tx);
    let receiver = ReceiveChannel::new(rx);
    (sender, receiver)
}

pub struct ChannelGroup<E>(pub Option<SendChannel<E>>, pub Option<ReceiveChannel<E>>);

impl<E> Default for ChannelGroup<E> {
    fn default() -> Self {
        let (sender, receiver) = create::<E>();
        Self(Some(sender), Some(receiver))
    }
}

impl<E> Clone for ChannelGroup<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<E> ChannelGroup<E> {
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = create::<E>();
        Self(Some(sender), Some(receiver))
    }
}

pub trait SendOnlyChannel<T> {
    /// # Errors
    ///
    /// Returns an error if the channel is closed or the send operation fails.
    fn try_send(&mut self, t: T) -> ChannelResult<()>;
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
    fn try_send(&mut self, t: T) -> ChannelResult<()> {
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
    #[must_use]
    pub fn send_only(self) -> Box<dyn SendOnlyChannel<T>> {
        Box::new(SendOnlyWrapper { channel: self })
    }
}

impl<T> SendChannel<T> {
    fn new(src: async_channel::Sender<T>) -> Self {
        Self { src: Some(src) }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed.
    pub fn pending_message_count(&mut self) -> ChannelResult<usize> {
        match &mut self.src {
            Some(src) => Ok(src.len()),
            None => Err(ChannelError::Closed),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is already closed.
    pub fn close(&mut self) -> ChannelResult<()> {
        if let Some(channel) = self.src.take() {
            drop(channel);
            Ok(())
        } else {
            Err(ChannelError::Closed)
        }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed or the send operation fails.
    pub async fn async_send(&mut self, t: T) -> ChannelResult<()> {
        match &mut self.src {
            Some(src) => match src.send(t).await {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err.to_string())),
            },
            None => Err(ChannelError::Closed),
        }
    }

    /// [`SendChannel`].`block_send()` blocks the current thread till data is sent or
    /// an error received. This generally should not be used in WASM or non-blocking
    /// environments.
    ///
    /// # Errors
    ///
    /// Returns an error if the channel is closed or the send operation fails.
    pub fn block_send(&mut self, t: T) -> ChannelResult<()> {
        match &mut self.src {
            Some(src) => match src.send_blocking(t) {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err.to_string())),
            },
            None => Err(ChannelError::Closed),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed or the send operation fails.
    pub fn try_send(&mut self, t: T) -> ChannelResult<()> {
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

    pub fn drain(&mut self) -> Drain<'_, T> {
        Drain { receiver: self }
    }

    // if the [`RecieveChannel`] was ever read once then this
    // becomes true, its up to the user to decide how they fit
    // this into their logic.
    /// # Errors
    ///
    /// Returns an error if the read flag cannot be accessed (should not occur in practice).
    pub fn read_atleast_once(&self) -> ChannelResult<bool> {
        Ok(self.read_flag.load())
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed.
    pub fn is_empty(&mut self) -> ChannelResult<bool> {
        match &self.src {
            None => Err(ChannelError::Closed),
            Some(src) => Ok(src.is_empty()),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed.
    pub fn is_closed(&mut self) -> ChannelResult<bool> {
        match &self.src {
            None => Err(ChannelError::Closed),
            Some(_) => Ok(false),
        }
    }

    /// [`ReceiveChannel`].`block_receive()` blocks the current thread till data is received or
    /// an error is seen. This generally should not be used in WASM or non-blocking
    /// environments.
    ///
    /// # Errors
    ///
    /// Returns an error if the channel is closed or the receive operation fails.
    pub fn block_receive(&mut self) -> ChannelResult<T> {
        match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => match src.recv_blocking() {
                Ok(maybe_item) => {
                    self.read_flag.store(true);
                    Ok(maybe_item)
                }
                Err(_) => self.close_channel(),
            },
        }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed or no data is available.
    pub async fn async_receive(&mut self) -> ChannelResult<T> {
        match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => {
                if let Ok(maybe_item) = src.recv().await {
                    self.read_flag.store(true);
                    Ok(maybe_item)
                } else {
                    if src.is_closed() {
                        return self.close_channel();
                    }
                    Err(ChannelError::ReceivedNoData)
                }
            }
        }
    }

    /// # Errors
    ///
    /// Returns an error if the channel is closed or no data is available.
    pub fn try_receive(&mut self) -> ChannelResult<T> {
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

    fn close_channel(&mut self) -> ChannelResult<T> {
        // remove the channel from the underlying slot
        _ = self.src.take();
        Err(ChannelError::Closed)
    }

    #[cfg(test)]
    pub fn close(&mut self) {
        _ = self.src.take();
    }
}

pub struct Drain<'a, T> {
    receiver: &'a mut ReceiveChannel<T>,
}

impl<T> Iterator for Drain<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.try_receive() {
            Ok(item) => Some(item),
            Err(ChannelError::Closed) => panic!("should not happen, channel was closed"),
            Err(_) => None,
        }
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
