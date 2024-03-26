// Crate implementing the Engineering Principles of Channels

use std::sync::{self, Arc};

use crossbeam::{atomic, channel};
use thiserror::Error;

type Result<T> = anyhow::Result<T, ChannelError>;

#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Channel has being closed or not initialized")]
    Closed,

    #[error("Failed to send message due to: {0}")]
    SendFailed(String),

    #[error("Failed to receive message due to: {0}")]
    ReceiveFailed(channel::TryRecvError),

    #[error("Channel sent nothing, possibly closed")]
    ReceivedNoData,
}

pub fn create<T>() -> (SendChannel<T>, ReceiveChannel<T>) {
    let (tx, rx) = channel::unbounded::<T>();
    let sender = SendChannel::new(tx);
    let receiver = ReceiveChannel::new(rx);
    (sender, receiver)
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
    src: Option<channel::Sender<T>>,
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
    fn new(src: channel::Sender<T>) -> Self {
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

    pub fn try_send(&mut self, t: T) -> Result<()> {
        match &mut self.src {
            Some(src) => match src.send(t) {
                Ok(()) => Ok(()),
                Err(err) => Err(ChannelError::SendFailed(err.to_string())),
            },
            None => Err(ChannelError::Closed),
        }
    }
}

pub struct ReceiveChannel<T> {
    acached: Arc<atomic::AtomicCell<Option<T>>>,
    read_flag: Arc<atomic::AtomicCell<bool>>,
    src: Option<channel::Receiver<T>>,
}

impl<T> Clone for ReceiveChannel<T> {
    fn clone(&self) -> Self {
        Self {
            read_flag: self.read_flag.clone(),
            acached: self.acached.clone(),
            src: self.src.clone(),
        }
    }
}

impl<T> ReceiveChannel<T> {
    fn new(src: channel::Receiver<T>) -> Self {
        Self {
            src: Some(src),
            acached: Arc::new(atomic::AtomicCell::new(None)),
            read_flag: Arc::new(atomic::AtomicCell::new(false)),
        }
    }

    // if the [`RecieveChannel`] was ever read once then this
    // becomes true, its up to the user to decide how they fit
    // this into their logic.
    pub fn read_atleast_once(&self) -> Result<bool> {
        return Ok(self.read_flag.load());
    }

    pub fn is_empty(&mut self) -> Result<bool> {
        self.check_channel();
        if self.has_cached_item() {
            return Ok(false);
        }

        match &self.src {
            None => Err(ChannelError::Closed),
            Some(src) => Ok(src.is_empty()),
        }
    }

    pub fn closed(&mut self) -> Result<bool> {
        self.check_channel();
        match &self.src {
            None => Err(ChannelError::Closed),
            Some(src) => Ok(false),
        }
    }

    pub fn block_receive(&mut self) -> Result<T> {
        if let Some(item) = self.get_cached_item() {
            return Ok(item);
        }
        self.read_channel(true)
    }

    pub fn try_receive(&mut self) -> Result<T> {
        if let Some(item) = self.get_cached_item() {
            return Ok(item);
        }
        self.read_channel(false)
    }

    // checks if the channel is still active, has data and
    // is still alive, the channel might be disconnected
    // and hence we use this opportunity to properly
    // remove channel from being used indicative of
    // a closed channel. We have to do this has crossbeam
    // channels do not provide us a way to check if they
    // are disconnected/closed and requires reading
    // from them to know this based on the returned Result<>.
    fn check_channel(&mut self) {
        if self.has_cached_item() {
            return;
        }

        // attempt to receive information and cache it in buffer
        match self.read_channel(false) {
            Ok(item) => {
                self.save_cached_item(item);
                return;
            }
            // everything else just returns an ok
            _ => return,
        }
    }

    fn close_channel(&mut self) -> Result<T> {
        // remove the channel from the underlying slot
        _ = self.src.take();
        Err(ChannelError::Closed)
    }

    fn get_cached_item(&mut self) -> Option<T> {
        self.acached.take()
    }

    fn save_cached_item(&mut self, item: T) {
        self.acached.store(Some(item));
    }

    fn has_cached_item(&mut self) -> bool {
        if let Some(item) = self.acached.take() {
            self.acached.store(Some(item));
            return true;
        }
        return false;
    }

    fn read_channel(&mut self, blocking: bool) -> Result<T> {
        if blocking {
            return match &mut self.src {
                None => Err(ChannelError::Closed),
                Some(src) => match src.recv() {
                    Ok(maybe_item) => {
                        self.read_flag.store(true);
                        Ok(maybe_item)
                    }
                    Err(err) => self.close_channel(),
                },
            };
        }
        match &mut self.src {
            None => Err(ChannelError::Closed),
            Some(src) => match src.try_recv() {
                Ok(maybe_item) => {
                    self.read_flag.store(true);
                    Ok(maybe_item)
                }
                Err(err) => match err {
                    channel::TryRecvError::Disconnected => self.close_channel(),
                    channel::TryRecvError::Empty => Err(ChannelError::ReceivedNoData),
                },
            },
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
