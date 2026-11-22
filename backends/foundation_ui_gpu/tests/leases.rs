use foundation_context::Ctx;
use foundation_ui_gpu::lease::{
    DeviceExchange, HostRequest, Identity, LeaseError, LeaseProvider, LeasedGraphics, Selection,
};
use futures_util::future::BoxFuture;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Default)]
struct Host(Mutex<Vec<HostRequest>>);
impl LeaseProvider for Host {
    fn request(&self, request: HostRequest) -> Result<(), LeaseError> {
        self.0.lock().unwrap().push(request);
        Ok(())
    }
}
struct Resource {
    identity: Identity,
    returns: Arc<AtomicUsize>,
}
impl LeasedGraphics for Resource {
    fn identity(&self) -> &Identity {
        &self.identity
    }
    fn compute(
        &self,
        _: String,
        words: Vec<u32>,
        _: u32,
    ) -> BoxFuture<'_, Result<Vec<u32>, LeaseError>> {
        Box::pin(async move { Ok(words) })
    }
    fn process_texture(&self, _: String, _: [u32; 3]) -> BoxFuture<'_, Result<(), LeaseError>> {
        Box::pin(async { Err(LeaseError::Incompatible) })
    }
    fn release(&self) {
        self.returns.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn out_of_order_callbacks_preserve_identity_and_return_each_grant_once() {
    let host = Arc::new(Host::default());
    let exchange = DeviceExchange::new(&Ctx::host(), host.clone()).unwrap();
    let other = DeviceExchange::new(&Ctx::host(), Arc::new(Host::default())).unwrap();
    let selection = Selection {
        device: "editor".into(),
        surface: Some("preview".into()),
    };
    let old = exchange.request(selection.clone()).unwrap();
    let new = exchange.request(selection.clone()).unwrap();
    let returns = Arc::new(AtomicUsize::new(0));
    let resource = |generation| {
        Arc::new(Resource {
            identity: Identity {
                selection: selection.clone(),
                generation,
            },
            returns: returns.clone(),
        }) as Arc<dyn LeasedGraphics>
    };
    let mut requests = std::mem::take(&mut *host.0.lock().unwrap());
    let second = requests.pop().unwrap();
    let first = requests.pop().unwrap();
    let ready = second.offer(Ok(resource(2))).unwrap();
    assert!(!other.accept(&ready));
    assert_eq!(
        second.offer(Ok(resource(2))).unwrap_err(),
        LeaseError::Stale
    );
    assert!(exchange.accept(&ready));
    assert!(!exchange.accept(&ready));
    let active = futures_lite::future::block_on(new.wait()).unwrap();
    assert_eq!(active.identity().generation, 2);
    drop(old);
    assert_eq!(
        first.offer(Ok(resource(1))).unwrap_err(),
        LeaseError::Cancelled
    );
    drop(active);
    assert_eq!(returns.load(Ordering::SeqCst), 3);
}

#[test]
#[ignore = "requires a graphics adapter with compute support"]
fn losing_the_imported_device_returns_an_idle_lease() {
    use foundation_ui_gpu::{
        lease_native::{ImportedDevice, NativeLease},
        wgpu, GpuConfig,
    };

    futures_lite::future::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance.request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();
        let host_device = device.clone();
        let (completed, result) = std::sync::mpsc::channel();
        let lease = NativeLease::import(
            Identity {
                selection: Selection {
                    device: "test".into(),
                    surface: None,
                },
                generation: 1,
            },
            ImportedDevice {
                instance,
                adapter,
                device,
                queue,
                texture: None,
            },
            GpuConfig::default(),
            move |status| completed.send(status).unwrap(),
        )
        .unwrap();

        host_device.destroy();
        host_device.poll(wgpu::PollType::Poll).unwrap();
        assert_eq!(
            result
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap(),
            Err(LeaseError::DeviceLost)
        );
        assert_eq!(
            lease.compute(String::new(), vec![1], 1).await,
            Err(LeaseError::DeviceLost)
        );
    });
}

#[test]
fn immediate_refusal_reaches_the_waiter() {
    struct ImmediateHost(Ctx);

    impl LeaseProvider for ImmediateHost {
        fn request(&self, request: HostRequest) -> Result<(), LeaseError> {
            let notice = request.offer(Err(LeaseError::Denied))?;
            assert!(self.0.get::<DeviceExchange>().unwrap().accept(&notice));
            Ok(())
        }
    }

    let ctx = Ctx::host();
    let exchange = DeviceExchange::new(&ctx, Arc::new(ImmediateHost(ctx.clone()))).unwrap();
    assert!(exchange.install(&ctx));
    let ticket = exchange
        .request(Selection {
            device: "editor".into(),
            surface: None,
        })
        .unwrap();
    assert!(matches!(
        futures_lite::future::block_on(ticket.wait()),
        Err(LeaseError::Denied)
    ));
    assert!(ctx.remove::<DeviceExchange>().is_some());
}

#[test]
fn a_replacement_rejects_old_work_and_cancellation_returns_the_new_offer() {
    let host = Arc::new(Host::default());
    let exchange = DeviceExchange::new(&Ctx::host(), host.clone()).unwrap();
    let selection = Selection {
        device: "editor".into(),
        surface: None,
    };
    let returns = Arc::new(AtomicUsize::new(0));
    let offer = |generation| {
        let ticket = exchange.request(selection.clone()).unwrap();
        let request = host.0.lock().unwrap().pop().unwrap();
        let notice = request
            .offer(Ok(Arc::new(Resource {
                identity: Identity {
                    selection: selection.clone(),
                    generation,
                },
                returns: returns.clone(),
            })))
            .unwrap();
        (notice, ticket)
    };
    let (ready, ticket) = offer(1);
    assert!(exchange.accept(&ready));
    let active = futures_lite::future::block_on(ticket.wait()).unwrap();
    let (replacement, pending) = offer(2);
    assert_eq!(
        futures_lite::future::block_on(active.compute(String::new(), vec![1], 1)),
        Err(LeaseError::Stale)
    );
    drop(pending);
    assert!(!exchange.accept(&replacement));
    let (stale, waiter) = offer(1);
    assert!(exchange.accept(&stale));
    assert!(matches!(
        futures_lite::future::block_on(waiter.wait()),
        Err(LeaseError::Stale)
    ));
    drop(active);
    assert_eq!(returns.load(Ordering::SeqCst), 3);
}
