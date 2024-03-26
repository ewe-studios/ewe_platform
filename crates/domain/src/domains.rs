// trait defintion for the Domain concept from the Principles of Architecture

use std::sync::Arc;

use futures::Future;
use serde::Serialize;

use channels::{
    channels::{ReceiveChannel, SendChannel, SendOnlyChannel},
    executor,
};

// Id identifies a giving (Request, Vec<Event>) pair
pub type Id = String;

// NamedRequest represent a target request of a specified
// type which has an Id to identify the request and
// any related events that are a response to the request.
pub struct NamedRequest<T>(Id, T);

impl<T> NamedRequest<T> {
    pub fn to(&self, t: Vec<T>) -> NamedEvent<T> {
        NamedEvent(self.0.clone(), t)
    }
}

// NamedEvent are events indicative of a response to a NamedRequest
pub struct NamedEvent<T>(Id, Vec<T>);

impl<T> NamedEvent<T> {
    pub fn from(&self, t: T) -> NamedRequest<T> {
        NamedRequest(self.0.clone(), t)
    }
}

// DomainShell provides the underlying boundary that wraps a domain and
// handles internal plumbing that the Domain uses to both process
// comunicate with the outside world.
//
// The DomainShell must always be Send-able and therefore threadsafe.
// So that it can go across threads and be the focal point for interaction.
//
// We envision that a Domain would accept a domain shell and use it accordingly.
//
// e.g DomainShell().serve(Domain)
pub trait DomainShell {
    // Enum defining your target event types
    type Events: Send + 'static;

    // Enum defining your target request types.
    type Requests: Send + 'static;

    // The platform provider context the domain will use.
    type Platform: Default;

    // Means of responding to received [`NamedRequest`] from
    // the domain.
    fn respond(&self, event: NamedEvent<Self::Events>);

    // perform requests on behalf of the Driving clients that
    // wish to get the domain to perform operations based on it's
    // internal logic or use-cases.
    //
    // Hexagonal Architecture: Driven Side
    fn do_requests(
        &self,
        req: NamedRequest<Self::Requests>,
    ) -> ReceiveChannel<NamedEvent<Self::Events>>;

    // Delivers request to the shell to be sent to all relevant
    // listens to perform work on behalf of the domain.
    //
    // This allows the domain really to inform the shell about it's
    // need for operations not natively within it's boundaries.
    //
    // Hexagonal Architecture: Driving Side
    fn send_requests(
        &self,
        req: NamedRequest<Self::Requests>,
    ) -> ReceiveChannel<NamedEvent<Self::Events>>;

    // schedule a task to execute when the receiver has data
    // usually the future here should really get scheduled
    // for polling if it's receiver finally received value.
    //
    // this allows us create inter-dependent work that
    // depends on the readiness of response on a channel.
    fn schedule(
        &self,
        receiver: ReceiveChannel<Self::Events>,
        fut: impl Future<Output = ()> + 'static + Send,
    ) where
        Self: Sized;

    // schedules a task for completion without dependence on a channel
    // get data. This is useful for work that is independent of
    // some underlying response from another work or processes.
    //
    // The focus is on the future itself and it's compeleness.
    //
    fn spawn(&self, fut: impl Future<Output = ()> + 'static + Send)
    where
        Self: Sized;

    // Retuns a new unique channel which the caller can use to listen to outgoing
    // requests from the channel. Providing a broadcast semantic where the listener
    //
    fn requests(&self) -> ReceiveChannel<NamedRequest<Arc<Self::Requests>>>;

    // listen to provide a receive channel that exists for the lifetime of
    // the domain and allows you listen in, into all events occuring in
    // [`Domain`].
    fn listen(&self) -> ReceiveChannel<Arc<Self::Events>>;
}

pub trait DomainServicer {
    // Enum defining your target event types
    type Events: Send + 'static;

    // Enum defining your target request types.
    type Requests: Send + 'static;

    // The platform provider context the domain will use.
    type Platform: Default;

    fn shell(
        &self,
    ) -> &dyn DomainShell<Platform = Self::Platform, Events = Self::Events, Requests = Self::Requests>;

    // Service delivers all incoming requests and events to the provided
    // [`Domain`]. It allows us to provide a separation between the shell and
    // the domain, but making it possible to send the shell across boundaries and
    // threads without leaking the domain into those same domains or threads.
    fn serve(
        &mut self,
        d: &dyn Domain<
            Shell = Self,
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    );
}

// Implement [`Domain`] on your type to create a business domain unit
// with specific inputs and outputs via requests and events
// via central handling function [`Domain.handle`].
pub trait Domain {
    // Enum defining your target event types
    type Events: Send + 'static;

    // Enum defining your target request types.
    type Requests: Send + 'static;

    // The domain shell the Domain will recieve
    type Shell: DomainShell;

    // The platform provider context the domain
    // will use to access platform features, usually
    // a struct with a default implement.
    type Platform: Default;

    // the domain simply must deliver response to the
    // send channel and has access to the shell if it
    // wishes to perform operations within another operaiton
    fn handle_request(
        &self,
        req: NamedRequest<Self::Requests>,
        chan: &dyn SendOnlyChannel<Self::Events>,
        shell: &Self::Shell,
        platform: &Self::Platform,
    );

    fn handle_event(
        &self,
        events: NamedEvent<Self::Events>,
        shell: &Self::Shell,
        platform: &Self::Platform,
    );
}
