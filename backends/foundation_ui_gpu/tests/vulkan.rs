use ash::vk;
use foundation_ui_gpu::vulkan::includes_required_device_features;

#[test]
fn matches_enabled_fields_in_vulkan_aggregate_records() {
    let core = vk::PhysicalDeviceFeatures::default().robust_buffer_access(true);
    let mut storage = vk::PhysicalDevice16BitStorageFeatures::default()
        .uniform_and_storage_buffer16_bit_access(true);
    let mut indexing = vk::PhysicalDeviceDescriptorIndexingFeatures::default()
        .descriptor_binding_storage_image_update_after_bind(true)
        .runtime_descriptor_array(true);
    let mut timeline =
        vk::PhysicalDeviceTimelineSemaphoreFeatures::default().timeline_semaphore(true);
    let mut maintenance = vk::PhysicalDeviceMaintenance4Features::default().maintenance4(true);
    let required = vk::DeviceCreateInfo::default()
        .enabled_features(&core)
        .push_next(&mut storage)
        .push_next(&mut indexing)
        .push_next(&mut timeline)
        .push_next(&mut maintenance);

    let mut v11 =
        vk::PhysicalDeviceVulkan11Features::default().uniform_and_storage_buffer16_bit_access(true);
    let mut v12 = vk::PhysicalDeviceVulkan12Features::default()
        .descriptor_binding_storage_image_update_after_bind(true)
        .runtime_descriptor_array(true)
        .timeline_semaphore(true);
    let mut v13 = vk::PhysicalDeviceVulkan13Features::default().maintenance4(true);
    let mut actual_core = vk::PhysicalDeviceFeatures2::default().features(core);
    let actual = vk::DeviceCreateInfo::default()
        .push_next(&mut actual_core)
        .push_next(&mut v11)
        .push_next(&mut v12)
        .push_next(&mut v13);

    // Both chains own all the records they reference for this call.
    assert!(unsafe { includes_required_device_features(&actual, &required) });
}

#[test]
fn rejects_a_missing_feature_in_an_enabled_aggregate() {
    let mut required_subgroups =
        vk::PhysicalDeviceSubgroupSizeControlFeatures::default().compute_full_subgroups(true);
    let required = vk::DeviceCreateInfo::default().push_next(&mut required_subgroups);
    let mut actual_v13 = vk::PhysicalDeviceVulkan13Features::default().subgroup_size_control(true);
    let actual = vk::DeviceCreateInfo::default().push_next(&mut actual_v13);

    assert!(!unsafe { includes_required_device_features(&actual, &required) });
}
