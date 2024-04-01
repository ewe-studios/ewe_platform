// Module for test helpers developing and validating domains.

use channels::mspc;

use crate::domains;

pub struct ShellTester<App: domains::Domain + Clone> {
    pub app: App,
    pub platform: App::Platform,
    pub incoming_response: mspc::ChannelGroup<domains::NamedEvent<App::Events>>,
    pub incoming_requests: mspc::ChannelGroup<domains::NamedRequest<App::Requests>>,
    pub outgoing_response: mspc::ChannelGroup<domains::NamedEvent<App::Events>>,
    pub outgoing_requests: mspc::ChannelGroup<domains::NamedRequest<App::Requests>>,
}

impl<App: domains::Domain + Clone> Default for ShellTester<App> {
    fn default() -> Self {
        Self {
            app: Default::default(),
            platform: Default::default(),
            incoming_response: Default::default(),
            incoming_requests: Default::default(),
            outgoing_response: Default::default(),
            outgoing_requests: Default::default(),
        }
    }
}

impl<App: domains::Domain + Clone> Clone for ShellTester<App> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            platform: self.platform.clone(),
            incoming_response: self.incoming_response.clone(),
            incoming_requests: self.incoming_requests.clone(),
            outgoing_response: self.outgoing_response.clone(),
            outgoing_requests: self.outgoing_requests.clone(),
        }
    }
}

impl<App: domains::Domain + Clone> domains::MasterShell for ShellTester<App> {
    fn send_events(
        &mut self,
        event: domains::NamedEvent<Self::Events>,
    ) -> domains::DomainOpsResult<(), Self::Events> {
        if let Some(mut sender) = self.outgoing_response.0.clone() {
            sender.try_send(event).expect("send event");
            return Ok(());
        }
        Err(domains::DomainOpsErrors::UnableToDeliverEvents(event))
    }

    fn send_requests(
        &mut self,
        req: domains::NamedRequest<Self::Requests>,
    ) -> domains::DomainOpsResult<
        mspc::ReceiveChannel<domains::NamedEvent<Self::Events>>,
        Self::Requests,
    > {
        if let Some(mut sender) = self.outgoing_requests.0.clone() {
            sender.try_send(req).expect("send request");
            return Ok(self.incoming_response.1.clone().take().unwrap());
        }
        Err(domains::DomainOpsErrors::UnableToSendRequest(req))
    }
}

impl<App: domains::Domain + Clone> domains::DomainShell for ShellTester<App> {
    type Events = App::Events;

    type Requests = App::Requests;

    type Platform = App::Platform;

    fn platform(&self) -> Self::Platform
    where
        Self: Sized,
    {
        self.platform.clone()
    }

    fn respond(
        &mut self,
        _id: domains::Id,
    ) -> domains::DomainOpsResult<
        channels::mspc::SendChannel<domains::NamedEvent<Self::Events>>,
        domains::Id,
    > {
        Ok(self.incoming_response.0.clone().take().unwrap())
    }

    fn do_request(
        &mut self,
        req: domains::NamedRequest<Self::Requests>,
    ) -> domains::DomainOpsResult<
        channels::mspc::ReceiveChannel<domains::NamedEvent<App::Events>>,
        Self::Requests,
    > {
        if let Some(sender) = self.outgoing_response.0.clone() {
            self.app.handle_request(req, sender, self.clone())
        }
        Ok(self.incoming_response.1.clone().take().unwrap())
    }

    fn schedule<Fut>(
        &self,
        _receiver: channels::mspc::ReceiveChannel<domains::NamedEvent<Self::Events>>,
        _receiver_fn: impl FnOnce(channels::mspc::ChannelResult<domains::NamedEvent<Self::Events>>) -> Fut
            + 'static
            + Send,
    ) -> domains::DomainResult<()>
    where
        Fut: futures::prelude::future::Future<Output = ()> + Send,
        Self: Sized,
    {
        todo!()
    }

    fn spawn(
        &self,
        _fut: impl futures::prelude::Future<Output = ()> + 'static + Send,
    ) -> domains::DomainResult<()>
    where
        Self: Sized,
    {
        todo!()
    }

    fn requests(
        &mut self,
    ) -> domains::DomainResult<
        channels::mspc::ReceiveChannel<std::sync::Arc<domains::NamedRequest<Self::Requests>>>,
    > {
        todo!()
    }

    fn listen(
        &mut self,
    ) -> domains::DomainResult<
        channels::mspc::ReceiveChannel<std::sync::Arc<domains::NamedEvent<Self::Events>>>,
    > {
        todo!()
    }
}
