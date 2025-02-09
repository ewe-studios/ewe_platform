#![cfg(feature = "tokio_runtime")]

use std::{result, time};

use async_trait::async_trait;
use tokio::sync::broadcast;

#[async_trait]
pub trait SenderExt<T> {
    async fn send_in(
        &self,
        message: T,
        after: time::Duration,
    ) -> result::Result<usize, broadcast::error::SendError<T>>;
}

#[async_trait]
impl<T: Send> SenderExt<T> for broadcast::Sender<T> {
    async fn send_in(
        &self,
        message: T,
        after: time::Duration,
    ) -> result::Result<usize, broadcast::error::SendError<T>> {
        tokio::time::sleep(after).await;
        self.send(message)
    }
}
