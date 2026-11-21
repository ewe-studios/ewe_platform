//! Negotiate client capabilities and run actions for a host or server.

mod delivery;
mod dispatcher;
mod errors;
mod hello;
mod local;

use std::sync::Arc;

use crate::action::{ActionRegistry, Origin};
use foundation_context::Ctx;
use foundation_ui_wire::writer::WriterConfig;

pub use delivery::{DefaultDelivery, DeliveryHandler};
pub use hello::{Connection, HelloError, HelloOutcome};
pub use local::LocalDispatcher;

/// The component root selected by the host for this request.
/// Install it in the request context when an action renders a component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderTarget(pub u32);

type TargetLookup = Box<dyn Fn(&Ctx, &str, u32) -> Option<u32> + Send + Sync>;

/// Configures the origin, registered handlers and output encoding for one dispatcher.
pub struct DispatcherBuilder {
    accept: Origin,
    registry: Arc<ActionRegistry>,
    config: WriterConfig,
    target_lookup: TargetLookup,
    delivery: Option<Arc<dyn DeliveryHandler>>,
}

impl DispatcherBuilder {
    #[must_use]
    pub fn new(accept: Origin, registry: Arc<ActionRegistry>) -> Self {
        Self {
            accept,
            registry,
            config: WriterConfig::default(),
            target_lookup: Box::new(|ctx, _, _| ctx.get::<RenderTarget>().map(|target| target.0)),
            delivery: None,
        }
    }

    /// Selects the encoder for successful action output.
    #[must_use]
    pub fn delivery(mut self, delivery: Arc<dyn DeliveryHandler>) -> Self {
        self.delivery = Some(delivery);
        self
    }

    /// Looks up the component root in host routing data for each action and trigger.
    /// The default reads a fixed `RenderTarget` from the caller's context.
    #[must_use]
    pub fn render_target(
        mut self,
        lookup: impl Fn(&Ctx, &str, u32) -> Option<u32> + Send + Sync + 'static,
    ) -> Self {
        self.target_lookup = Box::new(lookup);
        self
    }

    /// Sets the limits for requests and the default frame writer.
    #[must_use]
    pub fn writer_config(mut self, config: WriterConfig) -> Self {
        self.config = config;
        self
    }

    #[must_use]
    pub fn build(self) -> Dispatcher {
        let delivery = self
            .delivery
            .unwrap_or_else(|| Arc::new(DefaultDelivery::new(self.config.clone())));
        Dispatcher {
            accept: self.accept,
            registry: self.registry,
            delivery,
            config: self.config,
            target_lookup: self.target_lookup,
        }
    }
}

/// Runs actions at one origin. Each invocation receives its caller's context.
pub struct Dispatcher {
    accept: Origin,
    registry: Arc<ActionRegistry>,
    config: WriterConfig,
    target_lookup: TargetLookup,
    delivery: Arc<dyn DeliveryHandler>,
}
