//! Verify the feature values saved at Vulkan device creation.
use ash::vk::{self, TaggedStructure};
use std::collections::BTreeMap;

// Every listed Vulkan feature structure contains a base header followed by these VkBool32 fields.
// Exclude trailing alignment padding. Reject new or unknown required feature structures.
fn bool_count(kind: vk::StructureType) -> Option<usize> {
    Some(match kind {
        vk::PhysicalDeviceFragmentShaderBarycentricFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceImageRobustnessFeaturesEXT::STRUCTURE_TYPE
        | vk::PhysicalDeviceMaintenance4FeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceRayQueryFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceRayTracingPositionFetchFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceSamplerYcbcrConversionFeatures::STRUCTURE_TYPE
        | vk::PhysicalDeviceShaderDrawParametersFeatures::STRUCTURE_TYPE
        | vk::PhysicalDeviceShaderIntegerDotProductFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceTextureCompressionASTCHDRFeaturesEXT::STRUCTURE_TYPE
        | vk::PhysicalDeviceTimelineSemaphoreFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceZeroInitializeWorkgroupMemoryFeatures::STRUCTURE_TYPE => 1,
        vk::PhysicalDeviceCooperativeMatrixFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceShaderAtomicInt64Features::STRUCTURE_TYPE
        | vk::PhysicalDeviceShaderFloat16Int8Features::STRUCTURE_TYPE
        | vk::PhysicalDeviceShaderImageAtomicInt64FeaturesEXT::STRUCTURE_TYPE
        | vk::PhysicalDeviceSubgroupSizeControlFeatures::STRUCTURE_TYPE => 2,
        vk::PhysicalDeviceBufferDeviceAddressFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceMultiviewFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceRobustness2FeaturesEXT::STRUCTURE_TYPE
        | vk::PhysicalDeviceVulkanMemoryModelFeaturesKHR::STRUCTURE_TYPE => 3,
        vk::PhysicalDevice16BitStorageFeatures::STRUCTURE_TYPE => 4,
        vk::PhysicalDeviceAccelerationStructureFeaturesKHR::STRUCTURE_TYPE
        | vk::PhysicalDeviceMeshShaderFeaturesEXT::STRUCTURE_TYPE
        | vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::STRUCTURE_TYPE => 5,
        vk::PhysicalDeviceShaderAtomicFloatFeaturesEXT::STRUCTURE_TYPE => 12,
        vk::PhysicalDevicePortabilitySubsetFeaturesKHR::STRUCTURE_TYPE => 15,
        vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::STRUCTURE_TYPE => 20,
        _ => return None,
    })
}
unsafe fn nodes(
    mut head: *const vk::BaseInStructure<'_>,
) -> BTreeMap<i32, *const vk::BaseInStructure<'static>> {
    let mut nodes = BTreeMap::new();
    while let Some(node) = unsafe { head.as_ref() } {
        if nodes.insert(node.s_type.as_raw(), head.cast()).is_some() {
            break;
        }
        head = node.p_next;
    }
    nodes
}
unsafe fn includes(have: *const u32, need: *const u32, count: usize) -> bool {
    let need = unsafe { std::slice::from_raw_parts(need, count) };
    if have.is_null() {
        return need.iter().all(|bit| *bit == 0);
    }
    let have = unsafe { std::slice::from_raw_parts(have, count) };
    need.iter()
        .zip(have)
        .all(|(required, actual)| *required == 0 || *actual != 0)
}
pub(super) unsafe fn contains(
    actual: &vk::DeviceCreateInfo<'_>,
    required: &vk::DeviceCreateInfo<'_>,
) -> bool {
    let provided = unsafe { nodes(actual.p_next.cast()) };
    let core = provided
        .get(&vk::StructureType::PHYSICAL_DEVICE_FEATURES_2.as_raw())
        .map_or(actual.p_enabled_features, |&ptr| {
            let info = unsafe { &*ptr.cast::<vk::PhysicalDeviceFeatures2>() };
            &raw const info.features
        });
    // VkPhysicalDeviceFeatures contains only consecutive VkBool32 values.
    if !unsafe {
        includes(
            core.cast(),
            required.p_enabled_features.cast(),
            std::mem::size_of::<vk::PhysicalDeviceFeatures>() / 4,
        )
    } {
        return false;
    }
    let needed = unsafe { nodes(required.p_next.cast()) };
    needed.into_iter().all(|(kind, ptr)| {
        let Some(n) = bool_count(vk::StructureType::from_raw(kind)) else {
            return false;
        };
        let expected = unsafe { ptr.add(1).cast() };
        let actual = provided
            .get(&kind)
            .map_or(std::ptr::null(), |ptr| unsafe { ptr.add(1).cast() });
        unsafe { includes(actual, expected, n) }
    })
}
