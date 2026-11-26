#pragma once
#include <stdint.h>
#include <vulkan/vulkan.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct EweVulkanPlan EweVulkanPlan;
/* All handles belong to the host. Keep them alive through the lease completion callback.
   Pass the application's instance API version. It must support the adapter API up to Vulkan 1.3. */
EweVulkanPlan *ewe_ui_gpu_vulkan_plan(VkInstance instance, VkPhysicalDevice physical, uint32_t api_version,
                                      uint32_t sdk, uint32_t extension_count, const char *const *extensions);
/* The callback can use the feature chain and extensions to create a host-owned VkDevice.
   The creation record remains readable during the callback. */
int32_t ewe_ui_gpu_vulkan_requirements(const EweVulkanPlan *plan, void *context,
                                       int32_t (*read)(void *context,
                                                       const VkDeviceCreateInfo *requirements));
/* Supply the actual logical-device creation record and the queue family and index it created.
   Stop host submissions first. This function waits for previous use of the queue.
   The optional image must be initialized and in SHADER_READ_ONLY_OPTIMAL layout.
   The lease returns it in the same layout. Zero means the arguments omit a required record.
   An owned UniFFI grant handle carries the import result. Lift it exactly once in Kotlin. */
uint64_t ewe_ui_gpu_vulkan_grant(const EweVulkanPlan *plan, VkDevice device, uint32_t family, uint32_t index,
                                 const VkDeviceCreateInfo *creation, VkImage image,
                                 const VkImageCreateInfo *image_info, VkImageLayout layout,
                                 uint32_t image_queue_family);
void ewe_ui_gpu_vulkan_release_plan(EweVulkanPlan *plan);
#ifdef __cplusplus
}
#endif
