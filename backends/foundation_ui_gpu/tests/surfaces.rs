//! Check surface extents against the device texture limit.
use foundation_ui_gpu::{check_surface_extent, GpuConfig, GpuError};

// WHY: The shared device must accept an ordinary phone display.
// WHAT: The adapter limit admits the reported iPhone 17 physical size.
#[test]
fn phone_sized_extent_is_within_the_adapter_limit() {
    let limit = GpuConfig::default().surface_extent_limit(8192);
    assert_eq!(limit, 8192);
    assert!(check_surface_extent(limit, 1206, 2241).is_ok());
}

// WHY: A device that reports no texture limit must use a documented fallback.
// WHAT: A zero adapter report returns the configured default of 2048 pixels.
#[test]
fn a_missing_adapter_limit_uses_the_configured_fallback() {
    let config = GpuConfig::default();
    assert_eq!(config.limits.max_texture_dimension_2d, 2048);
    assert_eq!(config.surface_extent_limit(0), 2048);
}

// WHY: An oversized surface must fail as a typed error, not a panic.
// WHAT: The check carries the requested extent and the device limit.
#[test]
fn oversized_extent_reports_the_typed_error() {
    let error = check_surface_extent(2048, 1206, 2241).unwrap_err();
    assert!(matches!(
        error,
        GpuError::ExtentUnsupported {
            width: 1206,
            height: 2241,
            limit: 2048
        }
    ));
    assert!(error.to_string().contains("2241"));
}

// WHY: A rejected extent must not poison the next configuration.
// WHAT: The same limit accepts a valid extent after it rejects an oversized one.
#[test]
fn a_valid_extent_is_accepted_after_a_rejected_one() {
    let limit = GpuConfig::default().surface_extent_limit(2048);
    assert!(check_surface_extent(limit, 1206, 2241).is_err());
    assert!(check_surface_extent(limit, 1206, 2000).is_ok());
}

// WHY: A rejected extent must not stop the graphics worker or its surface.
// WHAT: Attach, present, reject an oversized resize, then resize and present.
#[cfg(all(target_vendor = "apple", not(target_family = "wasm")))]
#[test]
fn a_phone_surface_presents_after_an_oversized_resize_is_rejected() {
    use foundation_context::Ctx;
    use foundation_ui_gpu::{wgpu, GpuService, RenderHost};
    use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
    use std::{
        ffi::c_void,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    struct MetalLayer {
        layer: Retained<AnyObject>,
        width: u32,
        height: u32,
        live: Arc<AtomicBool>,
    }
    impl RenderHost for MetalLayer {
        fn create_surface(
            &self,
            instance: &wgpu::Instance,
        ) -> Result<wgpu::Surface<'static>, GpuError> {
            // SAFETY: This host keeps the layer alive until its surface drops.
            unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    Retained::as_ptr(&self.layer).cast_mut().cast::<c_void>(),
                ))
            }
            .map_err(|e| GpuError::Operation(e.to_string()))
        }
        fn size(&self) -> (u32, u32) {
            (self.width, self.height)
        }
        fn is_live(&self) -> bool {
            self.live.load(Ordering::Acquire)
        }
    }
    fn layer(width: u32, height: u32) -> MetalLayer {
        // SAFETY: CAMetalLayer is a system class. The host releases one reference.
        let layer: Retained<AnyObject> =
            unsafe { Retained::from_raw(msg_send![class!(CAMetalLayer), new]) }
                .expect("CAMetalLayer");
        MetalLayer {
            layer,
            width,
            height,
            live: Arc::new(AtomicBool::new(true)),
        }
    }

    let service = GpuService::install(&Ctx::host(), GpuConfig::default()).unwrap();
    futures_lite::future::block_on(async {
        let presented = service
            .run(|owner| {
                Box::pin(async move {
                    let first = layer(1206, 2241);
                    let live = first.live.clone();
                    let phone = match owner.attach("phone".into(), Box::new(first)).await {
                        Ok(id) => id,
                        // A host without an adapter cannot exercise this path.
                        Err(GpuError::Unavailable(reason)) => {
                            eprintln!("Skipped the hardware surface check: {reason}");
                            return Ok(false);
                        }
                        Err(error) => return Err(error),
                    };
                    owner.clear(phone.clone(), wgpu::Color::BLACK).await?;
                    let limit = owner
                        .device()
                        .await?
                        .device
                        .limits()
                        .max_texture_dimension_2d;
                    let beyond = limit.checked_add(1).expect("adapter texture limit");
                    let oversized = owner.resize(phone.clone(), beyond, 1).await;
                    assert!(matches!(oversized, Err(GpuError::ExtentUnsupported { .. })));
                    owner.resize(phone.clone(), 800, 600).await?;
                    owner.clear(phone.clone(), wgpu::Color::BLACK).await?;

                    let duplicate = owner
                        .attach("phone".into(), Box::new(layer(800, 600)))
                        .await;
                    assert!(matches!(duplicate, Err(GpuError::Operation(_))));
                    let retired = owner
                        .present(phone.clone(), |_, _, _| {
                            live.store(false, Ordering::Release);
                            Ok(())
                        })
                        .await;
                    assert!(matches!(retired, Err(GpuError::StaleSurface)));
                    let resize = owner.resize(phone.clone(), 800, 600).await;
                    assert!(matches!(resize, Err(GpuError::StaleSurface)));
                    let replacement = owner
                        .attach("phone".into(), Box::new(layer(800, 600)))
                        .await?;
                    assert!(matches!(owner.detach(&phone), Err(GpuError::StaleSurface)));
                    owner.clear(replacement, wgpu::Color::BLACK).await?;
                    Ok(true)
                })
            })
            .await
            .unwrap();
        if !presented {
            eprintln!("Skipped the hardware surface check: no adapter");
        }
        service.shutdown().await;
    });
}
