//! Let each delivery session read the live streams that its host permits.
use foundation_context::Ctx;
use foundation_ui_wire::writer::ClientCaps;
use std::sync::Arc;

/// A live feed sends complete frames and signals when its publisher disconnects.
pub enum FeedEvent<'a> {
    Frame(&'a [u8]),
    Disconnected,
}

/// Returns false when the receiver closes.
pub type LiveSink = Box<dyn FnMut(FeedEvent<'_>) -> bool + Send>;

/// Removes a feed listener when the connection closes.
pub struct FeedLease(Option<Box<dyn FnOnce() + Send + Sync>>);
impl FeedLease {
    pub fn new(release: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self(Some(Box::new(release)))
    }
}
impl Drop for FeedLease {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            release();
        }
    }
}

/// The host owns a feed and its retained update history.
pub trait ReplayFeed: Send + Sync {
    fn stream_id(&self) -> u64;
    /// Sends future updates until the receiver closes.
    ///
    /// # Errors
    /// Returns an error if the feed rejects the client or cannot open a listener.
    fn listen(&self, caps: &ClientCaps, send: LiveSink) -> Result<FeedLease, ReplayError>;
    /// Reads retained updates after the requested sequence.
    ///
    /// # Errors
    /// Returns an error if the cursor expires or the feed cannot supply updates.
    fn replay(&self, caps: &ClientCaps, after: u64) -> Result<Vec<Arc<[u8]>>, ReplayError>;
}

/// A cursor may need a fresh view.
#[derive(Clone, Debug)]
pub enum ReplayError {
    Expired,
    Unavailable(String),
}

/// The host selects allowed streams from its authenticated request context.
#[derive(Clone)]
pub struct SubscriptionReplay(Arc<FeedSelector>);
type FeedSelector = dyn Fn(&Ctx, &ClientCaps) -> Vec<Arc<dyn ReplayFeed>> + Send + Sync;

impl SubscriptionReplay {
    pub fn new<F>(select: F) -> Self
    where
        F: Fn(&Ctx, &ClientCaps) -> Vec<Arc<dyn ReplayFeed>> + Send + Sync + 'static,
    {
        Self(Arc::new(select))
    }
    #[must_use]
    pub fn feeds(&self, ctx: &Ctx, caps: &ClientCaps) -> Vec<Arc<dyn ReplayFeed>> {
        (self.0)(ctx, caps)
    }
}
