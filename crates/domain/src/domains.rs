#![allow(clippy::extra_unused_lifetimes)]

use std::sync::Arc;
use std::{fmt::Display, result};

use futures::{future, Future};
use thiserror::Error;

use tracing::{debug, error};

use ewe_channels::mspc::{self, ChannelError};

// Id identifies a giving (Request, Vec<Event>) pair
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(pub String);

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

/// `NamedRequest` represent a target request of a specified
/// type which has an Id to identify the request and
/// any related events that are a response to the request.
#[derive(Clone, Debug)]
pub struct NamedRequest<T: Clone>(Id, T);

impl<T: Clone> NamedRequest<T> {
    pub fn new(id: &str, t: T) -> Self {
        Self(Id(id.to_string()), t)
    }

    pub fn to_one<V: Clone>(&self, v: V) -> NamedEvent<V> {
        NamedEvent(self.0.clone(), vec![v])
    }

    pub fn to<V: Clone>(&self, v: Vec<V>) -> NamedEvent<V> {
        NamedEvent(self.0.clone(), v)
    }

    pub fn id(&self) -> Id {
        self.0.clone()
    }

    pub fn item(&self) -> T {
        self.1.clone()
    }
}

impl<T: Clone> Display for NamedRequest<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedRequest(Id={})", self.0)
    }
}

/// `NamedEvent` are events indicative of a response to a `NamedRequest`
#[derive(Clone, Debug)]
pub struct NamedEvent<T: Clone>(Id, Vec<T>);

impl<'a, T: Clone> Display for NamedEvent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedEvent(forRequestId={})", self.0)
    }
}

impl<'a, T: Clone> NamedEvent<T> {
    pub fn new(id: &str, t: Vec<T>) -> Self {
        Self(Id(id.to_string()), t)
    }

    pub fn from(&self, t: T) -> NamedRequest<T> {
        NamedRequest(self.0.clone(), t)
    }

    pub fn id(&self) -> Id {
        self.0.clone()
    }

    pub fn items(&self) -> Vec<T> {
        self.1.clone()
    }
}

#[derive(Error, Debug)]
pub enum DomainOpsErrors<E: Clone> {
    #[error("no response channel: {0}")]
    NotFound(Id),

    #[error("response was closed: {0}")]
    ClosedChannel(Id),

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

    #[error("Request receiver was closed, suspect and needs investigation")]
    ClosedRequestReceiver,

    #[error("Failed to spawn function")]
    FailedSpawning,

    #[error("Domain shell is not working anymore")]
    ClosedDomainShell,

    #[error("Request response send_channel was closed unexpectedly")]
    UnexpectedSenderClosure,

    #[error("Request response send_channel does not exists, needs investigation")]
    RequestSenderNotFound,

    #[error("System in problematic and unexpected state")]
    ProblematicState,

    #[error("System request closing of domain servicer")]
    CloseRequested,
}

pub type DomainResult<R> = result::Result<R, DomainErrors>;

/// `TaskExecutor` defines a trait that other relevant implementations
/// must implement to be able to work with the `CoreExecutor` which manages
/// relevant parties to progress in their underlying processes.
///
/// Generally you would see the [`DServicer`], [`UseCaseExecutor`] being
/// implementing this trate for registration to a `CoreExecutor`.
pub trait TaskExecutor {
    fn run_tasks(&mut self);
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
pub trait DomainShell: Clone {
    // Enum defining your target event types
    type Events: Send + Clone + 'static;

    // Enum defining your target request types.
    type Requests: Send + Clone + 'static;

    // The platform provider context the domain will use.
    type Platform: Clone + 'static;

    // the underlying platform provided by the shell.
    fn platform(&self) -> Self::Platform;

    /// Means of responding by others to received [`NamedRequest`] from
    /// the domain.
    fn respond(
        &mut self,
        id: Id,
    ) -> DomainOpsResult<mspc::SendChannel<NamedEvent<Self::Events>>, Id>;

    /// perform requests on behalf of the Driving clients that
    /// wish to get the domain to perform operations based on it's
    /// internal logic or use-cases.
    ///
    /// Hexagonal Architecture: Driven Side
    fn do_request(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>;

    /// schedule a task to execute when the receiver has data
    /// usually the future here should really get scheduled
    /// for polling if it's receiver finally received value.
    ///
    /// This allows us create inter-dependent work that
    /// depends on the readiness of response on a channel.
    fn schedule<Fut>(
        &self,
        receiver: mspc::ReceiveChannel<NamedEvent<Self::Events>>,
        receiver_fn: impl FnOnce(mspc::ChannelResult<NamedEvent<Self::Events>>) -> Fut + 'static + Send,
    ) -> DomainResult<()>
    where
        Fut: future::Future<Output = ()> + Send;

    /// schedules a task for completion without dependence on a channel
    /// get data. This is useful for work that is independent of
    /// some underlying response from another work or processes.
    ///
    /// The focus is on the future itself and it's compeleness.
    fn spawn(&self, fut: impl Future<Output = ()> + 'static + Send) -> DomainResult<()>;

    /// Retuns a new unique channel which the caller can use to listen to outgoing
    /// requests from the channel. Providing a broadcast semantic where the listener
    fn requests(&mut self)
        -> DomainResult<mspc::ReceiveChannel<Arc<NamedRequest<Self::Requests>>>>;

    /// listen to provide a receive channel that exists for the lifetime of
    /// the domain and allows you listen in, into all events occuring in
    /// [`Domain`].
    fn listen(&mut self) -> DomainResult<mspc::ReceiveChannel<Arc<NamedEvent<Self::Events>>>>;
}

/// `MasterShell` exposes core methods that allows
pub trait MasterShell: DomainShell {
    /// Delivers events to only outside listeners and not to the domain
    /// and to all relevant listeners listento be notified on
    /// important changes in this domain instance.
    ///
    /// This allows the domain to inform the shell and its subscribers
    /// about it's changes that occur due to request or events received
    /// via [`DomainShell`].respond and [`DomainShell`].`send_events`.
    ///
    /// Hexagonal Architecture: Driving Side
    fn send_others(&mut self, event: NamedEvent<Self::Events>)
        -> DomainOpsResult<(), Self::Events>;

    /// Delivers events to the shell to be sent to both the domain
    /// and to all relevant listeners listening to be notified on
    /// important changes in this domain instance.
    ///
    /// This allows the domain to inform the shell and its subscribers
    /// about it's changes that occur due to request or events received
    /// via [`DomainShell`].respond and [`DomainShell`].`send_events`.
    ///
    /// Hexagonal Architecture: Driving Side
    fn send_all(&mut self, event: NamedEvent<Self::Events>) -> DomainOpsResult<(), Self::Events>;

    /// Delivers request to the shell to be sent to all relevant
    /// listens to perform work on behalf of the domain.
    ///
    /// This allows the domain to inform the shell about it's
    /// need for operations not natively within it's boundaries.
    ///
    /// Hexagonal Architecture: Driving Side
    fn send_request(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>;
}

// Implement [`Domain`] on your type to create a business domain unit
// with specific inputs and outputs via requests and events
// via central handling function [`Domain.handle`].
pub trait Domain: Clone + Default {
    // Enum defining your target event types
    type Events: Clone + Send + 'static;

    // Enum defining your target request types.
    type Requests: Clone + Send + 'static;

    // The platform provider context the domain
    // will use to access platform features, usually
    // a struct with a default implement.
    type Platform: Default + Clone + 'static;

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

    /// handles events from both the domain's handling of requests
    /// and other incoming events sent
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

/// `UseCases` are logic that either fit a specific workflow steps
/// that do not need to belong to a specific domain or are consiered
/// external business logic that might work along side a domain but
/// do not inherently live within that domain.
///
/// This can be from underlying lopic accessing local filesystem or
/// abstraction of some application logic that simple needs to handle
/// specific requests type from a requesting domain.
///
/// They usually hook into a [`DomainShell::requests`] broadcasts
/// handling the specific request type that are focused on.
pub trait UseCase: Clone {
    // Enum defining your target event types
    type Event: Clone + Send + 'static;

    // Enum defining your target request types.
    type Request: Clone + Send + 'static;

    // The platform provider context the domain
    // will use to access platform features, usually
    // a struct with a default implement.
    type Platform: Clone + 'static;

    /// allows the `UseCaseManager` decide which specific requests matches
    /// a given use-case.
    fn is_request(&self, req: Arc<NamedRequest<Self::Request>>) -> bool;

    /// `handle_request` handles the processing of requests by this use-case
    /// containing the underlying logic necessary to perform
    /// it's specific workflow.
    fn handle_request(
        &mut self,
        req: Arc<NamedRequest<Self::Request>>,
        chan: mspc::SendChannel<NamedEvent<Self::Event>>,
        shell: impl DomainShell<
            Events = Self::Event,
            Requests = Self::Request,
            Platform = Self::Platform,
        >,
    );
}

pub struct UseCaseExecutor<
    Shell,
    U,
    E: Clone + Send + 'static,
    R: Clone + Send + 'static,
    P: Clone + 'static,
> where
    Shell: DomainShell<Events = E, Requests = R, Platform = P>,
    U: UseCase<Event = E, Request = R, Platform = P>,
{
    use_case: U,
    shell: Shell,
    receiver: mspc::ReceiveChannel<Arc<NamedRequest<R>>>,
}

impl<S, U, E: Clone + Send + 'static, R: Clone + Send + 'static, P: Clone + 'static>
    UseCaseExecutor<S, U, E, R, P>
where
    S: DomainShell<Events = E, Requests = R, Platform = P>,
    U: UseCase<Event = E, Request = R, Platform = P>,
{
    pub fn new(mut shell_provider: S, use_case: U) -> Self {
        Self {
            receiver: shell_provider.requests().expect("expected request channel"),
            shell: shell_provider,
            use_case,
        }
    }
}

impl<S, U, E: Clone + Send + 'static, R: Clone + Send + 'static, P: Clone + 'static> TaskExecutor
    for UseCaseExecutor<S, U, E, R, P>
where
    S: DomainShell<Events = E, Requests = R, Platform = P>,
    U: UseCase<Event = E, Request = R, Platform = P>,
{
    fn run_tasks(&mut self) {
        match self.receiver.try_receive() {
            Ok(req) => {
                if !self.use_case.is_request(req.clone()) {
                    return;
                }

                debug!("UseCase executor received a new task");

                let sender = self.shell.respond(req.id()).expect("get response sender");
                self.use_case
                    .handle_request(req, sender, self.shell.clone());
            }
            Err(ChannelError::ReceiveFailed(err)) => {
                error!("UseCase executor failed with a receive error: {}", err);
            }
            Err(ChannelError::Closed) => {
                error!("UseCase executor receiver was closed");
            }
            _ => {}
        }
    }
}
