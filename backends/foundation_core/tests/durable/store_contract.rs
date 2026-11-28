//! Compile-time tests for the store trait shape.

use crate::durable::support::{then, Scenario};
use foundation_core::durable::{
    AppendOutcome, AppendRequest, CloseRequest, CreateOutcome, CreateSpec, DeleteOutcome,
    DurableDriver, DurableStreamStore, ForkSpec, NotifierRef, ReadPage, ReadRequest, StoreError,
    StreamHead, StreamName,
};

struct MockStore;

impl DurableStreamStore for MockStore {
    async fn create(
        &self,
        _name: StreamName,
        _spec: CreateSpec,
    ) -> Result<CreateOutcome, StoreError> {
        unimplemented!("mock")
    }

    async fn head(&self, _name: &StreamName) -> Result<StreamHead, StoreError> {
        unimplemented!("mock")
    }

    async fn append(
        &self,
        _name: &StreamName,
        _req: AppendRequest,
    ) -> Result<AppendOutcome, StoreError> {
        unimplemented!("mock")
    }

    async fn read(&self, _name: &StreamName, _req: ReadRequest) -> Result<ReadPage, StoreError> {
        unimplemented!("mock")
    }

    async fn close(
        &self,
        _name: &StreamName,
        _req: CloseRequest,
    ) -> Result<AppendOutcome, StoreError> {
        unimplemented!("mock")
    }

    async fn delete(&self, _name: &StreamName) -> Result<DeleteOutcome, StoreError> {
        unimplemented!("mock")
    }

    async fn fork(&self, _name: StreamName, _spec: ForkSpec) -> Result<CreateOutcome, StoreError> {
        unimplemented!("mock")
    }

    fn notifier(&self, _name: &StreamName) -> Option<NotifierRef> {
        None
    }
}

struct MockDriver {
    store: MockStore,
}

impl DurableDriver for MockDriver {
    type Store = MockStore;

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        self.store.notifier(name)
    }
}

#[test]
fn the_store_trait_is_implementable() {
    let store = MockStore;
    let name = StreamName::parse("events/abc").expect("valid name");

    assert!(store.notifier(&name).is_none());
}

#[test]
fn given_a_driver_when_a_scenario_runs_then_the_driver_is_used() {
    let driver = MockDriver { store: MockStore };
    let name = StreamName::parse("events/abc").expect("valid name");

    let notifier = Scenario::given(driver).when(|driver| driver.notifier(&name));

    then(notifier.is_none(), "the mock driver has no notifier");
}
