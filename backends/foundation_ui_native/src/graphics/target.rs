//! The platform bridge owns one reference to its layer or native window.
use foundation_ui_gpu::{wgpu, GpuError};

pub(super) struct Target {
    #[cfg(target_vendor = "apple")]
    layer: std::ptr::NonNull<std::ffi::c_void>,
    #[cfg(target_os = "android")]
    window: ndk::native_window::NativeWindow,
}
impl Target {
    pub(super) fn surface(
        &self,
        instance: &wgpu::Instance,
    ) -> Result<wgpu::Surface<'static>, GpuError> {
        #[cfg(target_vendor = "apple")]
        let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(self.layer.as_ptr());
        #[cfg(target_os = "android")]
        let target = {
            use wgpu::rwh::HasWindowHandle;
            wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(wgpu::rwh::AndroidDisplayHandle::new().into()),
                raw_window_handle: self
                    .window
                    .window_handle()
                    .map_err(|e| GpuError::Operation(e.to_string()))?
                    .as_raw(),
            }
        };
        #[cfg(any(target_vendor = "apple", target_os = "android"))]
        // SAFETY: DrawingHost retains this target until after its registered wgpu surface drops.
        return unsafe { instance.create_surface_unsafe(target) }
            .map_err(|e| GpuError::Operation(e.to_string()));
        #[cfg(not(any(target_vendor = "apple", target_os = "android")))]
        {
            let _ = (self, instance);
            Err(GpuError::IncompatibleSurface)
        }
    }
}

#[cfg(any(target_vendor = "apple", target_os = "android"))]
fn lower(target: Target) -> u64 {
    <super::NativeDrawingTarget as uniffi::FfiConverterArc<crate::UniFfiTag>>::lower(
        std::sync::Arc::new(super::NativeDrawingTarget {
            target,
            #[cfg(target_os = "android")]
            retired: false.into(),
        }),
    )
    .as_raw()
}

#[cfg(target_vendor = "apple")]
mod apple {
    use super::{lower, Target};
    use std::{ffi::c_void, ptr::NonNull};
    #[link(name = "objc")]
    extern "C" {
        fn objc_retain(object: *mut c_void) -> *mut c_void;
        fn objc_release(object: *mut c_void);
    }
    // SAFETY: The retained layer stays alive across threads. Only the graphics owner presents to it.
    unsafe impl Send for Target {}
    // SAFETY: Sharing this reference does not grant access to mutable layer properties.
    unsafe impl Sync for Target {}
    impl Drop for Target {
        fn drop(&mut self) {
            // SAFETY: The constructor retained this layer exactly once.
            unsafe { objc_release(self.layer.as_ptr()) };
        }
    }
    /// Transfer one retained platform layer into a native binding handle.
    ///
    /// # Safety
    /// The caller passes a live `CAMetalLayer` or null.
    #[no_mangle]
    pub unsafe extern "C" fn ewe_ui_gpu_retain_metal_layer(layer: *mut c_void) -> u64 {
        match NonNull::new(unsafe { objc_retain(layer) }) {
            Some(layer) => lower(Target { layer }),
            None => 0,
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::{lower, Target};
    use crate::graphics::NativeDrawingTarget;
    use std::{ffi::c_void, sync::atomic::Ordering};
    use uniffi::FfiConverterArc;

    /// Invalidate a retained Java Surface without waiting for the graphics owner.
    ///
    /// # Safety
    /// The handle is an owned clone of the live target binding.
    #[no_mangle]
    pub unsafe extern "system" fn Java_ewe_ui_android_GpuTargetBridge_retire(
        _: *mut c_void,
        _: *mut c_void,
        handle: i64,
    ) {
        let Some(handle) = (unsafe { uniffi::Handle::from_raw(handle.cast_unsigned()) }) else {
            return;
        };
        if let Ok(target) =
            <NativeDrawingTarget as FfiConverterArc<crate::UniFfiTag>>::try_lift(handle)
        {
            target.retired.store(true, Ordering::Release);
        }
    }

    /// Transfer a Java Surface through its native window reference.
    ///
    /// # Safety
    /// The Java native interface supplies a valid environment and a live android.view.Surface.
    #[no_mangle]
    pub unsafe extern "system" fn Java_ewe_ui_android_GpuTargetBridge_retain(
        env: *mut c_void,
        _: *mut c_void,
        surface: *mut c_void,
    ) -> i64 {
        unsafe { ndk::native_window::NativeWindow::from_surface(env.cast(), surface.cast()) }
            .map_or(0, |window| lower(Target { window }).cast_signed())
    }
}
