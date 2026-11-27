//! The local catalog: one load per model, shared by every resolve.

#![cfg(unix)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_agent_rpc::core::model::{
    Device, LoadState, ModelCatalog, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::host::native::{LoadFuture, LocalCatalog, ModelLoader};
use foundation_agent_rpc::model::ModelSource;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;

struct CountingLoader {
    count: Arc<AtomicUsize>,
}

impl ModelLoader for CountingLoader {
    fn load<'a>(&'a self, _source: &'a ModelSource, _ctx: &'a Ctx) -> LoadFuture<'a> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let mut mock = MockModelProvider::new();
            mock.on_any(vec![mock_text("hi")]);
            Ok(ModelHandle::from_router(
                "mock",
                ModelKind::Api,
                Device::Remote,
                "mock",
                mock.into_router(),
            ))
        })
    }
}

fn ctx() -> Ctx {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

#[valtron_test]
async fn native_catalog_loads_each_model_once() {
    let count = Arc::new(AtomicUsize::new(0));
    let loader = Arc::new(CountingLoader {
        count: Arc::clone(&count),
    });
    let sources = vec![ModelSource::candle("mock", "/nonexistent")];
    let catalog = LocalCatalog::new(&sources, loader, ctx()).expect("catalog");
    let request = ModelRequest {
        name: None,
        kind: None,
        device: None,
    };

    let first = catalog.resolve(&request).await.expect("the first resolve");
    let second = catalog.resolve(&request).await.expect("the second resolve");
    assert!(first.same_model(&second), "every resolve shares one model");
    assert_eq!(count.load(Ordering::SeqCst), 1, "the model loads once");
    assert_eq!(catalog.capabilities().models[0].state, LoadState::Loaded);
}

#[valtron_test]
async fn native_catalog_preload_loads_before_the_first_resolve() {
    let count = Arc::new(AtomicUsize::new(0));
    let loader = Arc::new(CountingLoader {
        count: Arc::clone(&count),
    });
    let sources = vec![ModelSource::candle("mock", "/nonexistent")];
    let catalog = LocalCatalog::new(&sources, loader, ctx()).expect("catalog");
    catalog.preload_all().await.expect("preload");
    assert_eq!(count.load(Ordering::SeqCst), 1);

    let request = ModelRequest {
        name: None,
        kind: None,
        device: None,
    };
    catalog
        .resolve(&request)
        .await
        .expect("resolve after preload");
    assert_eq!(count.load(Ordering::SeqCst), 1, "preload is not repeated");
}
