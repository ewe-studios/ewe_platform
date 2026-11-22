//! Test the graphics service through the same context handle that actions use.
mod leases;
use foundation_context::Ctx;
use foundation_ui_gpu::{GpuConfig, GpuError, GpuService};

#[test]
fn closed_service_cancels_queued_requests() {
    let ctx = Ctx::host();
    let service = GpuService::install(&ctx, GpuConfig::default()).unwrap();
    assert!(ctx.get::<GpuService>().is_some());
    service.close();
    assert!(matches!(
        futures_lite::future::block_on(service.run(|_| Box::pin(async { Ok(1) }))),
        Err(GpuError::Cancelled)
    ));
}

#[test]
#[ignore = "requires a graphics adapter with compute support"]
fn compute_without_a_view() {
    let service = GpuService::install(&Ctx::host(), GpuConfig::default()).unwrap();
    futures_lite::future::block_on(service.run(|owner| {
        Box::pin(async move {
            let info = owner.device().await?.adapter.get_info();
            println!("Adapter {:?} uses {:?}", info.name, info.backend);
            let out = owner
                .compute_words(include_str!("shaders/double.wgsl"), &[0, 2, 7], 3)
                .await?;
            assert_eq!(out, [1, 5, 15]);
            Ok(())
        })
    }))
    .unwrap();
    service.close();
}

#[test]
fn device_failures_are_shared_until_reinitialization() {
    use foundation_ui_gpu::{wgpu, DeviceProvider, GraphicsDevice};
    use futures_util::future::{join, LocalBoxFuture};
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };
    struct Unavailable(AtomicU32);
    impl DeviceProvider for Unavailable {
        fn open<'a>(
            &'a self,
            _: &'a wgpu::Instance,
            _: &'a GpuConfig,
        ) -> LocalBoxFuture<'a, Result<GraphicsDevice, GpuError>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err(GpuError::Unavailable("test adapter offline".into())) })
        }
    }
    let provider = Arc::new(Unavailable(AtomicU32::new(0)));
    let gpu =
        GpuService::install_with(&Ctx::host(), GpuConfig::default(), provider.clone()).unwrap();
    futures_lite::future::block_on(async {
        let request = || gpu.run(|owner| Box::pin(async { owner.device().await.map(|_| ()) }));
        let (first, second) = join(request(), request()).await;
        assert!(matches!(first, Err(GpuError::Unavailable(_))));
        assert!(matches!(second, Err(GpuError::Unavailable(_))));
        assert_eq!(provider.0.load(Ordering::SeqCst), 1);
        assert!(matches!(
            gpu.run(|owner| Box::pin(async { owner.reinitialize().await.map(|_| ()) }))
                .await,
            Err(GpuError::Unavailable(_))
        ));
        assert_eq!(provider.0.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn dropping_a_queued_request_skips_its_job() {
    use futures_util::{pin_mut, poll};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let gpu = GpuService::install(&Ctx::host(), GpuConfig::default()).unwrap();
    let invoked = Arc::new(AtomicBool::new(false));
    futures_lite::future::block_on(async {
        let (release, gate) = async_channel::bounded(1);
        let (started, active) = async_channel::bounded(1);
        let blocker = gpu.run(move |_| {
            Box::pin(async move {
                started.send(()).await.unwrap();
                gate.recv().await.unwrap();
                Ok(())
            })
        });
        pin_mut!(blocker);
        assert!(poll!(blocker.as_mut()).is_pending());
        active.recv().await.unwrap();
        {
            let flag = invoked.clone();
            let pending = gpu.run(move |_| {
                Box::pin(async move {
                    flag.store(true, Ordering::SeqCst);
                    Ok(())
                })
            });
            pin_mut!(pending);
            assert!(poll!(pending.as_mut()).is_pending());
        }
        release.send(()).await.unwrap();
        blocker.await.unwrap();
        gpu.run(|_| Box::pin(async { Ok(()) })).await.unwrap();
        assert!(!invoked.load(Ordering::SeqCst));
    });
}

#[test]
#[ignore = "requires a graphics adapter with compute support"]
fn invalid_limits_and_destroyed_device_report_typed_failures() {
    let mut impossible = GpuConfig::default();
    impossible.limits.max_bind_groups = u32::MAX;
    let invalid = GpuService::install(&Ctx::host(), impossible).unwrap();
    futures_lite::future::block_on(async {
        let result = invalid
            .run(|owner| Box::pin(async { owner.device().await.map(|_| ()) }))
            .await;
        assert!(matches!(result, Err(GpuError::Unsupported)));
        let service = GpuService::install(&Ctx::host(), GpuConfig::default()).unwrap();
        service
            .run(|owner| {
                Box::pin(async move {
                    let device = owner.device().await?.device.clone();
                    device.destroy();
                    device
                        .poll(foundation_ui_gpu::wgpu::PollType::Poll)
                        .unwrap();
                    assert!(matches!(owner.device().await, Err(GpuError::DeviceLost)));
                    owner.reinitialize().await?;
                    assert_eq!(
                        owner
                            .compute_words(include_str!("shaders/double.wgsl"), &[2], 1)
                            .await?,
                        [5]
                    );
                    Ok(())
                })
            })
            .await
            .unwrap();
    });
}
