//! The Durable Object (DO) of one model slave (decision D16 of spec 77).
//!
//! The DO builds a `WorkerStorage`, a `WorkerModelClient` and a `ModelSlave` on
//! its first request, then gives every request to `ModelSlave::handle`. A model
//! slave has no timer, so `alarm` does nothing.

use std::sync::Arc;

use foundation_sandbox::cloudflare::WorkerStorage;
use foundation_sandbox::shared::cloudflare::ModelSlave;
use futures::lock::Mutex;

use crate::model_client::{model_config_from_env, store_model_key, WorkerModelClient};
use crate::{
    connect_error_response, ctx, limits, read_bounded_body, simple_request, worker_response,
};

/// The model slave Durable Object.
#[worker::durable_object]
pub struct ModelSlaveDo {
    state: worker::State,
    env: worker::Env,
    slave: Mutex<Option<Arc<ModelSlave>>>,
}

impl worker::DurableObject for ModelSlaveDo {
    fn new(state: worker::State, env: worker::Env) -> Self {
        Self {
            state,
            env,
            slave: Mutex::new(None),
        }
    }

    async fn fetch(&self, mut request: worker::Request) -> worker::Result<worker::Response> {
        let limit = limits().max_message_bytes;
        let body = match read_bounded_body(&mut request, limit).await {
            Ok(body) => body,
            Err(error) => {
                return connect_error_response(&request, &[], error.current_context().clone())
            }
        };
        let parts = simple_request(&request, &body)?;
        let slave = self.slave().await?;
        worker_response(slave.handle(parts).await)
    }

    /// A model slave has no timer.
    async fn alarm(&self) -> worker::Result<worker::Response> {
        worker::Response::empty()
    }
}

impl ModelSlaveDo {
    /// The one slave of this object, built on the first request.
    async fn slave(&self) -> worker::Result<Arc<ModelSlave>> {
        let mut cell = self.slave.lock().await;
        if let Some(slave) = cell.as_ref() {
            return Ok(Arc::clone(slave));
        }
        let ctx = ctx();
        store_model_key(&self.env, &ctx);
        let storage: Arc<dyn foundation_sandbox::shared::cloudflare::DoStorage> =
            Arc::new(WorkerStorage::new(self.state.storage()));
        let client = Arc::new(WorkerModelClient::new(self.env.clone()));
        let slave = Arc::new(ModelSlave::new(
            ctx,
            storage,
            client,
            model_config_from_env(&self.env),
            limits().max_message_bytes,
        ));
        *cell = Some(Arc::clone(&slave));
        Ok(slave)
    }
}
