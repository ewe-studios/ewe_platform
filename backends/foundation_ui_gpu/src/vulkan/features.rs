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
// Match by field name because the aggregate records use different field offsets.
unsafe fn promoted(
    provided: &BTreeMap<i32, *const vk::BaseInStructure<'static>>,
    kind: vk::StructureType,
    required: *const vk::BaseInStructure<'_>,
) -> Option<bool> {
    macro_rules! compare {
        ($($aggregate:ident => {
            $($individual:ident: $first:ident $(, $field:ident)*;)+
        })+) => {
            match kind {
                $($(
                    vk::$individual::STRUCTURE_TYPE => {
                        let aggregate = provided.get(&vk::$aggregate::STRUCTURE_TYPE.as_raw())?;
                        // The caller supplies a valid Vulkan chain with these structure types.
                        let have = unsafe { &*aggregate.cast::<vk::$aggregate>() };
                        let need = unsafe { &*required.cast::<vk::$individual>() };
                        Some(
                            (need.$first == vk::FALSE || have.$first != vk::FALSE)
                            $(&& (need.$field == vk::FALSE || have.$field != vk::FALSE))*
                        )
                    }
                )+)+
                _ => None,
            }
        };
    }
    compare! {
        PhysicalDeviceVulkan11Features => {
            PhysicalDevice16BitStorageFeatures:
                storage_buffer16_bit_access, uniform_and_storage_buffer16_bit_access,
                storage_push_constant16, storage_input_output16;
            PhysicalDeviceMultiviewFeatures:
                multiview, multiview_geometry_shader, multiview_tessellation_shader;
            PhysicalDeviceSamplerYcbcrConversionFeatures: sampler_ycbcr_conversion;
            PhysicalDeviceShaderDrawParametersFeatures: shader_draw_parameters;
        }
        PhysicalDeviceVulkan12Features => {
            PhysicalDeviceShaderAtomicInt64Features:
                shader_buffer_int64_atomics, shader_shared_int64_atomics;
            PhysicalDeviceShaderFloat16Int8Features: shader_float16, shader_int8;
            PhysicalDeviceDescriptorIndexingFeaturesEXT:
                shader_input_attachment_array_dynamic_indexing,
                shader_uniform_texel_buffer_array_dynamic_indexing,
                shader_storage_texel_buffer_array_dynamic_indexing,
                shader_uniform_buffer_array_non_uniform_indexing,
                shader_sampled_image_array_non_uniform_indexing,
                shader_storage_buffer_array_non_uniform_indexing,
                shader_storage_image_array_non_uniform_indexing,
                shader_input_attachment_array_non_uniform_indexing,
                shader_uniform_texel_buffer_array_non_uniform_indexing,
                shader_storage_texel_buffer_array_non_uniform_indexing,
                descriptor_binding_uniform_buffer_update_after_bind,
                descriptor_binding_sampled_image_update_after_bind,
                descriptor_binding_storage_image_update_after_bind,
                descriptor_binding_storage_buffer_update_after_bind,
                descriptor_binding_uniform_texel_buffer_update_after_bind,
                descriptor_binding_storage_texel_buffer_update_after_bind,
                descriptor_binding_update_unused_while_pending,
                descriptor_binding_partially_bound,
                descriptor_binding_variable_descriptor_count,
                runtime_descriptor_array;
            PhysicalDeviceTimelineSemaphoreFeaturesKHR: timeline_semaphore;
            PhysicalDeviceBufferDeviceAddressFeaturesKHR:
                buffer_device_address, buffer_device_address_capture_replay, buffer_device_address_multi_device;
            PhysicalDeviceVulkanMemoryModelFeaturesKHR:
                vulkan_memory_model, vulkan_memory_model_device_scope,
                vulkan_memory_model_availability_visibility_chains;
        }
        PhysicalDeviceVulkan13Features => {
            PhysicalDeviceImageRobustnessFeaturesEXT: robust_image_access;
            PhysicalDeviceSubgroupSizeControlFeatures: subgroup_size_control, compute_full_subgroups;
            PhysicalDeviceTextureCompressionASTCHDRFeaturesEXT: texture_compression_astc_hdr;
            PhysicalDeviceZeroInitializeWorkgroupMemoryFeatures: shader_zero_initialize_workgroup_memory;
            PhysicalDeviceShaderIntegerDotProductFeaturesKHR: shader_integer_dot_product;
            PhysicalDeviceMaintenance4FeaturesKHR: maintenance4;
        }
    }
}

/// Compare enabled features with the records from [`super::VulkanPlan::requirements`].
///
/// The enabled record can use separate feature structures or Vulkan 1.1, 1.2, and 1.3 aggregate structures.
///
/// # Safety
/// The creation records and their feature pointers must reference valid Vulkan structures for this call.
#[must_use]
pub unsafe fn includes_required_device_features(
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
    if !required.p_enabled_features.is_null()
        && !unsafe {
            includes(
                core.cast(),
                required.p_enabled_features.cast(),
                std::mem::size_of::<vk::PhysicalDeviceFeatures>() / 4,
            )
        }
    {
        return false;
    }
    let needed = unsafe { nodes(required.p_next.cast()) };
    needed.into_iter().all(|(kind, ptr)| {
        let structure = vk::StructureType::from_raw(kind);
        let Some(count) = bool_count(structure) else {
            return false;
        };
        let expected = unsafe { ptr.add(1).cast() };
        if let Some(actual) = provided.get(&kind) {
            return unsafe { includes(actual.add(1).cast(), expected, count) };
        }
        unsafe { promoted(&provided, structure, ptr) }
            .unwrap_or_else(|| unsafe { includes(std::ptr::null(), expected, count) })
    })
}
