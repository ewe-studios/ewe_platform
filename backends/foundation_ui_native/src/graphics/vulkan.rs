//! Trusted Android native development kit entry points. See `ewe_graphics_vulkan.h`.
use super::handoff::NativeDeviceGrant;
use ash::vk;
use foundation_ui_gpu::vulkan::{VulkanImage, VulkanPlan};
use std::{
    ffi::{c_char, c_void, CStr},
    sync::Arc,
};

/// # Safety
/// The host supplies live instance and physical-device handles and its actual enabled extension list.
#[no_mangle]
pub unsafe extern "C" fn ewe_ui_gpu_vulkan_plan(
    instance: vk::Instance,
    physical: vk::PhysicalDevice,
    version: u32,
    sdk: u32,
    count: u32,
    names: *const *const c_char,
) -> *mut VulkanPlan {
    if count > 0 && names.is_null() {
        return std::ptr::null_mut();
    }
    let pointers = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(names, count as usize) }
    };
    if pointers.iter().any(|p| p.is_null()) {
        return std::ptr::null_mut();
    }
    let extensions = pointers
        .iter()
        .map(|p| unsafe { CStr::from_ptr(*p) })
        .collect::<Vec<_>>();
    unsafe { VulkanPlan::from_existing(instance, physical, version, sdk, &extensions) }
        .map_or(std::ptr::null_mut(), |plan| Box::into_raw(Box::new(plan)))
}

/// # Safety
/// The plan belongs to this library. The callback reads each pointer only before it returns.
#[no_mangle]
pub unsafe extern "C" fn ewe_ui_gpu_vulkan_requirements(
    plan: *const VulkanPlan,
    context: *mut c_void,
    read: unsafe extern "C" fn(*mut c_void, *const vk::DeviceCreateInfo<'_>) -> i32,
) -> i32 {
    unsafe { plan.as_ref() }
        .and_then(|plan| {
            plan.requirements(|info| unsafe { read(context, info) })
                .ok()
        })
        .unwrap_or(-1)
}

/// # Safety
/// The host attests to the actual logical device and image creation records and lends the queue exclusively.
/// The physical device, logical device, queue, and image must belong to the same host instance.
/// The application keeps every handle alive until the completion callback.
#[no_mangle]
pub unsafe extern "C" fn ewe_ui_gpu_vulkan_grant(
    plan: *const VulkanPlan,
    device: vk::Device,
    family: u32,
    index: u32,
    creation: *const vk::DeviceCreateInfo<'_>,
    image: vk::Image,
    image_info: *const vk::ImageCreateInfo<'_>,
    layout: vk::ImageLayout,
    image_family: u32,
) -> u64 {
    let (Some(plan), Some(creation)) = (unsafe { plan.as_ref() }, unsafe { creation.as_ref() })
    else {
        return 0;
    };
    let image = match (image == vk::Image::null(), unsafe { image_info.as_ref() }) {
        (true, None) => None,
        (false, Some(info)) => Some(VulkanImage {
            image,
            format: info.format,
            extent: info.extent,
            image_type: info.image_type,
            usage: info.usage,
            samples: info.samples,
            mips: info.mip_levels,
            layers: info.array_layers,
            layout,
            queue_family: image_family,
        }),
        _ => return 0,
    };
    let imported = unsafe { plan.borrow_device(device, family, index, creation, image) };
    let grant = NativeDeviceGrant::new(Box::new(move || imported));
    <NativeDeviceGrant as uniffi::FfiConverterArc<crate::UniFfiTag>>::lower(Arc::new(grant))
        .as_raw()
}
/// # Safety
/// Pass a plan from `ewe_ui_gpu_vulkan_plan` once. Do not call plan methods after release.
#[no_mangle]
pub unsafe extern "C" fn ewe_ui_gpu_vulkan_release_plan(plan: *mut VulkanPlan) {
    if !plan.is_null() {
        drop(unsafe { Box::from_raw(plan) });
    }
}
