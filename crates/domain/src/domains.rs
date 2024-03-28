// trait defintion for the Domain concept from the Principles of Architecture

use std::sync::Arc;
use std::{fmt::Display, result};

use futures::{future, Future};
use thiserror::Error;

use channels::mspc;

// Id identifies a giving (Request, Vec<Event>) pair
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Id(pub String);

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

// NamedRequest represent a target request of a specified
// type which has an Id to identify the request and
// any related events that are a response to the request.
#[derive(Clone)]
pub struct NamedRequest<T: Clone>(Id, T);

impl<T: Clone> NamedRequest<T> {
    pub fn to(&self, t: Vec<T>) -> NamedEvent<T> {
        NamedEvent(self.0.clone(), t)
    }

    pub fn id(&self) -> Id {
        self.0.clone()
    }
}

impl<T: Clone> Display for NamedRequest<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedRequest(Id={})", self.0)
    }
}

// NamedEvent are events indicative of a response to a NamedRequest
#[allow(dead_code)]
#[derive(Clone)]
pub struct NamedEvent<T: Clone>(Id, Vec<T>);

impl<'a, T: Clone> Display for NamedEvent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedEvent(forRequestId={})", self.0)
    }
}

impl<'a, T: Clone> NamedEvent<T> {
    pub fn from(&self, t: T) -> NamedRequest<T> {
        NamedRequest(self.0.clone(), t)
    }

    pub fn id(&self) -> Id {
        self.0.clone()
    }
}

#[derive(Error)]
pub enum DomainOpsErrors<E: Clone> {
    #[error("no NamedRequests was found matching the event: {0}")]
    NoMatchingRequestForEvent(NamedEvent<E>),

    #[error("Failed to deliver provided NamedEvent: {0}")]
    UnableToDeliverEvents(NamedEvent<E>),

    #[error("no NamedRequests was found matching the event: {0}")]
    UnableToSendRequest(NamedRequest<E>),

    #[error("NamedRequest {0} could not be processsed")]
    RequestFailedProcessing(NamedRequest<E>),
}

pub type DomainOpsResult<R, E> = result::Result<R, DomainOpsErrors<E>>;

#[derive(Debug, Error)]
pub enum DomainErrors {
    #[error("Failed to schedule function")]
    FailedScheduling,

    #[error("Failed to spawn function")]
    FailedSpawning,

    #[error("Domain shell is not working anymore")]
    ClosedDomainShell,
}

pub type DomainResult<R> = result::Result<R, DomainErrors>;

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
    type Events: Send + Clone + 'static;

    // Enum defining your target request types.
    type Requests: Send + Clone + 'static;

    // The platform provider context the domain will use.
    type Platform: Clone;

    // the underlying platform provided by the shell.
    fn platform(&self) -> Self::Platform
    where
        Self: Sized;

    // Means of responding to received [`NamedRequest`] from
    // the domain.
    fn respond(&mut self, event: NamedEvent<Self::Events>) -> DomainOpsResult<(), Self::Events>;

    // perform requests on behalf of the Driving clients that
    // wish to get the domain to perform operations based on it's
    // internal logic or use-cases.
    //
    // Hexagonal Architecture: Driven Side
    fn do_requests(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>;

    // schedule a task to execute when the receiver has data
    // usually the future here should really get scheduled
    // for polling if it's receiver finally received value.
    //
    // this allows us create inter-dependent work that
    // depends on the readiness of response on a channel.
    fn schedule<Fut>(
        &self,
        receiver: mspc::ReceiveChannel<NamedEvent<Self::Events>>,
        receiver_fn: impl FnOnce(mspc::Result<NamedEvent<Self::Events>>) -> Fut + 'static + Send,
    ) -> DomainResult<()>
    where
        Fut: future::Future<Output = ()> + Send,
        Self: Sized;

    // schedules a task for completion without dependence on a channel
    // get data. This is useful for work that is independent of
    // some underlying response from another work or processes.
    //
    // The focus is on the future itself and it's compeleness.
    fn spawn(&self, fut: impl Future<Output = ()> + 'static + Send) -> DomainResult<()>
    where
        Self: Sized;

    // Retuns a new unique channel which the caller can use to listen to outgoing
    // requests from the channel. Providing a broadcast semantic where the listener
    //
    fn requests(&mut self)
        -> DomainResult<mspc::ReceiveChannel<Arc<NamedRequest<Self::Requests>>>>;

    // listen to provide a receive channel that exists for the lifetime of
    // the domain and allows you listen in, into all events occuring in
    // [`Domain`].
    fn listen(&mut self) -> DomainResult<mspc::ReceiveChannel<Arc<NamedEvent<Self::Events>>>>;
}

/// MasterShell exposes core methods that allows
pub trait MasterShell: DomainShell {
    // Delivers events to the shell to be sent to all relevant
    // listens be notified of changes via events from the domain.
    //
    // This allows the domain to inform the shell and its subscribers
    // about it's changes that occur due to request or events received
    // via [`DomainShell`].respond and [`DomainShell`].send_events.
    //
    // Hexagonal Architecture: Driving Side
    fn send_events(&mut self, event: NamedEvent<Self::Events>)
        -> DomainOpsResult<(), Self::Events>;

    // Delivers request to the shell to be sent to all relevant
    // listens to perform work on behalf of the domain.
    //
    // This allows the domain to inform the shell about it's
    // need for operations not natively within it's boundaries.
    //
    // Hexagonal Architecture: Driving Side
    fn send_requests(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>;
}

pub trait DomainServicer {
    // Enum defining your target event types
    type Events: Send + Clone + 'static;

    // Enum defining your target request types.
    type Requests: Send + Clone + 'static;

    // The platform provider context the domain will use.
    type Platform: Clone;

    fn shell(
        &self,
    ) -> impl DomainShell<Platform = Self::Platform, Events = Self::Events, Requests = Self::Requests>;

    // Service delivers all incoming requests and events to the provided
    // [`Domain`]. It allows us to provide a separation between the shell and
    // the domain, but making it possible to send the shell across boundaries and
    // threads without leaking the domain into those same domains or threads.
    fn serve(
        &mut self,
        d: impl Domain<Events = Self::Events, Requests = Self::Requests, Platform = Self::Platform>,
    ) -> DomainResult<()>;
}

// Implement [`Domain`] on your type to create a business domain unit
// with specific inputs and outputs via requests and events
// via central handling function [`Domain.handle`].
pub trait Domain {
    // Enum defining your target event types
    type Events: Clone + Send + 'static;

    // Enum defining your target request types.
    type Requests: Clone + Send + 'static;

    // The platform provider context the domain
    // will use to access platform features, usually
    // a struct with a default implement.
    type Platform: Clone;

    // the domain simply must deliver response to the
    // send channel and has access to the shell if it
    // wishes to perform operations within another operaiton
    fn handle_request(
        &self,
        req: NamedRequest<Self::Requests>,
        chan: mspc::SendChannel<NamedEvent<Self::Events>>,
        shell: impl MasterShell<
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    );

    fn handle_event(
        &self,
        events: NamedEvent<Self::Events>,
        shell: impl MasterShell<
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    );
}
