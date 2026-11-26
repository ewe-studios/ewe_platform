//! Route replay through the downstream connection that owns a subscription.
use crate::http;
use foundation_ui_runtime::dispatch::{FeedLease, ReplayError, ReplayFeed, SubscriptionReplay};
use foundation_ui_wire::{
    subscription::{replay_expired, tag_replay},
    writer::ClientCaps,
    ResumeSubscription, WireLimits,
};
use std::{collections::HashMap, io, num::NonZeroU64, sync::Arc};

pub(crate) type FeedLeases = HashMap<(u64, u8), FeedLease>;
pub(crate) type Replies = Box<dyn Iterator<Item = io::Result<Vec<u8>>> + Send>;

pub(crate) fn one(bytes: Vec<u8>) -> Replies {
    Box::new(std::iter::once(Ok(bytes)))
}

pub(crate) struct Subscriptions {
    feeds: HashMap<u64, Arc<dyn ReplayFeed>>,
    bindings: HashMap<u64, u64>,
    caps: ClientCaps,
}

impl Subscriptions {
    pub fn new(ctx: &http::Ctx, caps: &ClientCaps) -> Self {
        let feeds = ctx
            .get::<SubscriptionReplay>()
            .map_or_default(|service| service.feeds(ctx, caps));
        Self {
            feeds: feeds
                .into_iter()
                .map(|feed| (feed.stream_id(), feed))
                .collect(),
            bindings: HashMap::new(),
            caps: caps.clone(),
        }
    }

    pub fn attach(
        &self,
        attached: &mut FeedLeases,
        output: &async_channel::Sender<Vec<u8>>,
    ) -> io::Result<()> {
        for feed in self.feeds.values() {
            let key = (feed.stream_id(), self.caps.mount.0);
            if attached.contains_key(&key) {
                continue;
            }
            let sink = output.clone();
            let lease = feed
                .listen(
                    &self.caps,
                    Box::new(move |event| {
                        if let foundation_ui_runtime::dispatch::FeedEvent::Frame(bytes) = event {
                            if sink.try_send(bytes.to_vec()).is_ok() {
                                return true;
                            }
                        }
                        sink.close();
                        false
                    }),
                )
                .map_err(|err| io::Error::other(format!("{err:?}")))?;
            attached.insert(key, lease);
        }
        Ok(())
    }

    pub fn replay(
        &mut self,
        request: ResumeSubscription<'_>,
        limits: &WireLimits,
        max_subscriptions: usize,
    ) -> io::Result<Replies> {
        let id = NonZeroU64::new(request.subscription_id())
            .ok_or_else(|| io::Error::other("Missing subscription"))?;
        let stream = NonZeroU64::new(request.stream_id())
            .ok_or_else(|| io::Error::other("Missing stream"))?;
        let feed = self
            .feeds
            .get(&stream.get())
            .ok_or_else(|| io::Error::other("Unavailable live stream"))?;
        if let Some(bound) = self.bindings.get(&id.get()) {
            if *bound != stream.get() {
                return Err(io::Error::other("Subscription belongs to another stream"));
            }
        } else {
            if self.bindings.len() >= max_subscriptions {
                return Err(io::Error::other("Subscription limit reached"));
            }
            self.bindings.insert(id.get(), stream.get());
        }
        match feed.replay(&self.caps, request.after()) {
            Ok(history) => {
                let limits = *limits;
                Ok(Box::new(history.into_iter().map(move |bytes| {
                    tag_replay(&bytes, id, &limits).map_err(io::Error::other)
                })))
            }
            Err(ReplayError::Expired) => Ok(one(
                replay_expired(id, stream, request.after()).map_err(io::Error::other)?
            )),
            Err(ReplayError::Unavailable(cause)) => Err(io::Error::other(cause)),
        }
    }
}
