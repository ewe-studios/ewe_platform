// trait defintion for the Domain concept
use serde::Serialize;

use channels::channels::{ReceiveChannel, SendChannel, SendOnlyChannel};

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

    // Channel for listening to requests generated from the domain to
    // other interested [`DomainUseCase`] or the outside word that implement
    // different operations that need to be isolated from the domain
    // itself but are part of it's operations.
    fn requests(&self) -> ReceiveChannel<NamedRequest<Self::Requests>>;

    // listen to provide a receive channel that exists for the lifetime of
    // the domain and allows you listen in, into all events occuring in
    // [`Domain`].
    fn listen(&self) -> ReceiveChannel<Self::Events>;
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

// Defines a struct type that contains the channel for a target request
// via it's id.
// pub struct OperationsChannel<E, R>(SendChannel<E>, ReceiveChannel<R>);

// // Implements a default domain shell that can be used as the core of
// // a domain.
// pub struct DefaultShell<E, R, P> {
//     driving: OperationsChannel<E, R>,
//     driven: OperationsChannel<E, R>,
// }

// impl<E, R, P> DefaultShell<E, R, P> {
//     pub fn new() -> Self {
//         Self {}
//     }
// }

// impl<E: Send + 'static, R: Send + 'static, P: Default> DomainShell for DefaultShell<E, R, P> {
//     type Events = E;
//     type Platform = P;
//     type Requests = R;

//     fn respond(&self, event: NamedEvent<Self::Events>) {}

//     fn send_requests(
//         &self,
//         req: NamedRequest<Self::Requests>,
//     ) -> ReceiveChannel<NamedEvent<Self::Events>> {
//         todo!()
//     }

//     fn do_requests(
//         &self,
//         req: NamedRequest<Self::Requests>,
//     ) -> ReceiveChannel<NamedEvent<Self::Events>> {
//         todo!()
//     }

//     fn requests(&self) -> ReceiveChannel<NamedRequest<Self::Requests>> {
//         todo!()
//     }

//     fn listen(&self) -> ReceiveChannel<Self::Events> {
//         todo!()
//     }

//     fn serve(
//         &mut self,
//         d: &dyn Domain<
//             Events = Self::Events,
//             Requests = Self::Requests,
//             Shell = Self,
//             Platform = P,
//         >,
//     ) {
//         todo!()
//     }
// }

#[cfg(test)]
mod tests {}
