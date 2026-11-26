//! Lend an externally created Vulkan device, queue, and optional initialized image.
use crate::{lease::LeaseError, lease_native::ImportedDevice};
use ash::vk;
use std::ffi::CStr;
use wgpu::hal::{self, api::Vulkan};
mod features;

/// Keep the original Vulkan instance alive until all grants finish.
pub struct VulkanPlan {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    native: ash::Instance,
    physical: vk::PhysicalDevice,
    extensions: Vec<&'static CStr>,
}
impl VulkanPlan {
    /// Read the device requirements for this specific adapter.
    ///
    /// # Safety
    /// Supply a valid instance, its actual API version, enabled extensions, and physical device.
    /// The application retains the instance until this plan and every imported device are released.
    ///
    /// # Errors
    /// Rejects an unrelated adapter, missing extensions, or unavailable loader.
    pub unsafe fn from_existing(
        instance: vk::Instance,
        physical: vk::PhysicalDevice,
        version: u32,
        sdk: u32,
        enabled: &[&CStr],
    ) -> Result<Self, LeaseError> {
        if instance == vk::Instance::null() || physical == vk::PhysicalDevice::null() {
            return Err(LeaseError::Incompatible);
        }
        let entry =
            unsafe { ash::Entry::load() }.map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        let native = unsafe { ash::Instance::load(entry.static_fn(), instance) };
        if !unsafe { native.enumerate_physical_devices() }
            .map_err(vk_error)?
            .contains(&physical)
        {
            return Err(LeaseError::Incompatible);
        }
        let physical_version =
            unsafe { native.get_physical_device_properties(physical) }.api_version;
        let required_version = physical_version.min(vk::API_VERSION_1_3);
        // The Vulkan backend selects promoted device functions from the physical API version.
        // The application's instance must expose those functions through its device loader.
        if (
            vk::api_version_major(version),
            vk::api_version_minor(version),
        ) < (
            vk::api_version_major(required_version),
            vk::api_version_minor(required_version),
        ) {
            return Err(LeaseError::Incompatible);
        }
        let flags = wgpu::InstanceFlags::empty();
        let extensions = hal::vulkan::Instance::desired_extensions(&entry, version, flags)
            .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        if extensions.iter().any(|e| !enabled.contains(e)) {
            return Err(LeaseError::Incompatible);
        }
        let borrowed = unsafe {
            hal::vulkan::Instance::from_raw(
                entry,
                native.clone(),
                version,
                sdk,
                None,
                extensions,
                flags,
                wgpu::MemoryBudgetThresholds::default(),
                false,
                Some(Box::new(|| {})),
            )
        }
        .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        let exposed = borrowed
            .expose_adapter(physical)
            .ok_or(LeaseError::Incompatible)?;
        let extensions = exposed
            .adapter
            .required_device_extensions(wgpu::Features::empty());
        // SAFETY: This adapter came from the same retained HAL instance.
        let instance = unsafe { wgpu::Instance::from_hal::<Vulkan>(borrowed) };
        let adapter = unsafe { instance.create_adapter_from_hal::<Vulkan>(exposed) };
        Ok(Self {
            instance,
            adapter,
            native,
            physical,
            extensions,
        })
    }

    /// Give the host the exact enabled feature chain and extension names before it creates its own device.
    ///
    /// # Errors
    /// Rejects an adapter without the Vulkan backend.
    pub fn requirements<T>(
        &self,
        read: impl FnOnce(&vk::DeviceCreateInfo<'_>) -> T,
    ) -> Result<T, LeaseError> {
        let adapter = unsafe { self.adapter.as_hal::<Vulkan>() }.ok_or(LeaseError::Incompatible)?;
        let mut features =
            adapter.physical_device_features(&self.extensions, wgpu::Features::empty());
        let names = self
            .extensions
            .iter()
            .map(|name| name.as_ptr())
            .collect::<Vec<_>>();
        let info = features
            .add_to_device_create(vk::DeviceCreateInfo::default().enabled_extension_names(&names));
        Ok(read(&info))
    }

    /// Import the queue represented by the host's device creation record.
    ///
    /// # Safety
    /// The logical device must belong to the plan's physical device. Supply the actual creation record.
    /// All pointers in that record must remain valid through this call. Retain the device until the release callback.
    /// The host lends its queue exclusively. A supplied image belongs to this device and queue family.
    ///
    /// # Errors
    /// Rejects unenabled features or extensions, a mismatched queue, and incompatible images.
    pub unsafe fn borrow_device(
        &self,
        device: vk::Device,
        family: u32,
        index: u32,
        creation: &vk::DeviceCreateInfo<'_>,
        image: Option<VulkanImage>,
    ) -> Result<ImportedDevice, LeaseError> {
        if device == vk::Device::null() {
            return Err(LeaseError::Incompatible);
        }
        let available = unsafe {
            self.native
                .get_physical_device_queue_family_properties(self.physical)
        };
        if !available.get(family as usize).is_some_and(|group| {
            group
                .queue_flags
                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                && index < group.queue_count
        }) {
            return Err(LeaseError::Incompatible);
        }
        let queues = unsafe {
            view(
                creation.p_queue_create_infos,
                creation.queue_create_info_count,
            )
        };
        if !queues.iter().any(|info| {
            info.queue_family_index == family && index < info.queue_count && info.flags.is_empty()
        }) {
            return Err(LeaseError::Incompatible);
        }
        let enabled = unsafe {
            view(
                creation.pp_enabled_extension_names,
                creation.enabled_extension_count,
            )
        }
        .iter()
        .map(|name| unsafe { CStr::from_ptr(*name) })
        .collect::<Vec<_>>();
        if self.extensions.iter().any(|e| !enabled.contains(e))
            || !self.requirements(|needed| unsafe { features::contains(creation, needed) })?
        {
            return Err(LeaseError::Incompatible);
        }
        let raw = unsafe { ash::Device::load(self.native.fp_v1_0(), device) };
        let adapter = unsafe { self.adapter.as_hal::<Vulkan>() }.ok_or(LeaseError::Incompatible)?;
        let desc = wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::downlevel_defaults(),
            ..wgpu::DeviceDescriptor::default()
        };
        let mut missing_limit = None;
        desc.required_limits.check_limits_with_fail_fn(
            &self.adapter.limits(),
            true,
            |name, required, available| missing_limit = Some((name, required, available)),
        );
        if let Some((name, required, available)) = missing_limit {
            return Err(LeaseError::Unavailable(format!(
                "Vulkan adapter limit {name} is {available}, but the graphics service requires {required}"
            )));
        }
        let opened = unsafe {
            adapter.device_from_raw(
                raw,
                Some(Box::new(|| {})),
                &self.extensions,
                desc.required_features,
                &desc.required_limits,
                &desc.memory_hints,
                family,
                index,
            )
        }
        .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        drop(adapter);
        let native = &opened.device;
        let selected_queue = native.raw_queue();
        if native.raw_device().handle() != device
            || native.raw_physical_device() != self.physical
            || selected_queue == vk::Queue::null()
        {
            return Err(LeaseError::Incompatible);
        }
        // Vulkan loaders can give the same queue different dispatch handles.
        // Wait on the queue selected by the verified device, family, and index.
        unsafe { native.raw_device().queue_wait_idle(selected_queue) }.map_err(vk_error)?;
        let (device, queue) =
            unsafe { self.adapter.create_device_from_hal::<Vulkan>(opened, &desc) }
                .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        let texture = image
            .map(|image| unsafe { image.import(&device, family) })
            .transpose()?;
        Ok(ImportedDevice {
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device,
            queue,
            texture,
        })
    }
}
fn vk_error(error: vk::Result) -> LeaseError {
    if error == vk::Result::ERROR_DEVICE_LOST {
        LeaseError::DeviceLost
    } else {
        LeaseError::Unavailable(format!("Vulkan error {}", error.as_raw()))
    }
}
unsafe fn view<'a, T>(ptr: *const T, count: u32) -> &'a [T] {
    if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, count as usize) }
    }
}

pub struct VulkanImage {
    pub image: vk::Image,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub image_type: vk::ImageType,
    pub usage: vk::ImageUsageFlags,
    pub samples: vk::SampleCountFlags,
    pub mips: u32,
    pub layers: u32,
    pub layout: vk::ImageLayout,
    pub queue_family: u32,
}
impl VulkanImage {
    unsafe fn import(
        self,
        device: &wgpu::Device,
        family: u32,
    ) -> Result<wgpu::Texture, LeaseError> {
        if self.image == vk::Image::null()
            || self.format != vk::Format::R8G8B8A8_UNORM
            || self.image_type != vk::ImageType::TYPE_2D
            || self.samples != vk::SampleCountFlags::TYPE_1
            || self.mips != 1
            || self.layers != 1
            || self.extent.depth != 1
            || self.extent.width == 0
            || self.extent.height == 0
            || self.queue_family != family
            || self.layout != vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
            || !self
                .usage
                .contains(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
        {
            return Err(LeaseError::Incompatible);
        }
        let size = wgpu::Extent3d {
            width: self.extent.width,
            height: self.extent.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("leased Vulkan image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let info = hal::TextureDescriptor {
            label: desc.label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: desc.dimension,
            format: desc.format,
            usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_DST,
            memory_flags: hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        let native = unsafe { device.as_hal::<Vulkan>() }.ok_or(LeaseError::Incompatible)?;
        // The host owns both the image and its allocation. Neither wrapper can destroy them.
        let texture = unsafe {
            native.texture_from_raw(
                self.image,
                &info,
                Some(Box::new(|| {})),
                hal::vulkan::TextureMemory::External,
            )
        };
        drop(native);
        Ok(unsafe {
            device.create_texture_from_hal::<Vulkan>(texture, &desc, wgpu::TextureUses::RESOURCE)
        })
    }
}
