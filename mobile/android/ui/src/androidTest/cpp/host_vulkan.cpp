// Test host. It creates and destroys its Vulkan device independently of the Rust core.
#include "ewe_graphics_vulkan.h"
#include <algorithm>
#include <android/native_window_jni.h>
#include <array>
#include <cstring>
#include <dlfcn.h>
#include <jni.h>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
#include <vulkan/vulkan.h>

static void check(VkResult status, const char *where) {
    if (status != VK_SUCCESS && status != VK_SUBOPTIMAL_KHR)
        throw std::runtime_error(std::string(where) + " returned " + std::to_string(status));
}
template <typename T> static T symbol(void *lib, const char *name) {
    auto value = reinterpret_cast<T>(dlsym(lib, name));
    if (!value)
        throw std::runtime_error(name);
    return value;
}
struct Host {
    void *lib = nullptr;
    decltype(&ewe_ui_gpu_vulkan_requirements) requirements = nullptr;
    decltype(&ewe_ui_gpu_vulkan_release_plan) releasePlan = nullptr;
    decltype(&ewe_ui_gpu_vulkan_grant) makeGrant = nullptr;
    EweVulkanPlan *plan = nullptr;
    VkInstance instance = {};
    VkPhysicalDevice physical = {};
    VkDevice device = {};
    VkQueue queue = {};
    uint32_t family = 0;
    VkDeviceQueueCreateInfo queueInfo{VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO};
    float priority = 1.f;
    VkImage image = {};
    VkImageCreateInfo imageInfo{VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO};
    VkDeviceMemory imageMemory = {}, bufferMemory = {};
    VkBuffer buffer = {};
    VkCommandPool pool = {};
    ANativeWindow *window = nullptr;
    VkSurfaceKHR surface = {};
    VkSwapchainKHR swapchain = {};
    VkFence acquireFence = {};
    uint64_t grant = 0;
    uint32_t grantIndex = 0;
    bool create = false, withImage = false;
    VkDeviceSize memory(VkMemoryRequirements req, VkMemoryPropertyFlags wanted, VkDeviceMemory &out) {
        VkPhysicalDeviceMemoryProperties props;
        vkGetPhysicalDeviceMemoryProperties(physical, &props);
        for (uint32_t i = 0; i < props.memoryTypeCount; i++) {
            if ((req.memoryTypeBits & (1u << i)) && (props.memoryTypes[i].propertyFlags & wanted) == wanted) {
                VkMemoryAllocateInfo alloc{VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
                alloc.allocationSize = req.size;
                alloc.memoryTypeIndex = i;
                check(vkAllocateMemory(device, &alloc, nullptr, &out), "allocate memory");
                return req.size;
            }
        }
        throw std::runtime_error("No required memory type");
    }
    template <typename F> void submit(F encode) {
        VkCommandBufferAllocateInfo alloc{VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO};
        alloc.commandPool = pool;
        alloc.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
        alloc.commandBufferCount = 1;
        VkCommandBuffer cmd{};
        check(vkAllocateCommandBuffers(device, &alloc, &cmd), "allocate commands");
        VkCommandBufferBeginInfo start{VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
        check(vkBeginCommandBuffer(cmd, &start), "begin commands");
        encode(cmd);
        check(vkEndCommandBuffer(cmd), "end commands");
        VkSubmitInfo submission{VK_STRUCTURE_TYPE_SUBMIT_INFO};
        submission.commandBufferCount = 1;
        submission.pCommandBuffers = &cmd;
        check(vkQueueSubmit(queue, 1, &submission, VK_NULL_HANDLE), "submit");
        check(vkQueueWaitIdle(queue), "finish");
        vkFreeCommandBuffers(device, pool, 1, &cmd);
    }
    static VkImageSubresourceRange range() { return {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1}; }
    static void barrier(VkCommandBuffer cmd, VkImage img, VkImageLayout old, VkImageLayout next,
                        VkAccessFlags src, VkAccessFlags dst) {
        VkImageMemoryBarrier change{VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER};
        change.oldLayout = old;
        change.newLayout = next;
        change.srcAccessMask = src;
        change.dstAccessMask = dst;
        change.srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
        change.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
        change.image = img;
        change.subresourceRange = range();
        vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, 0,
                             0, nullptr, 0, nullptr, 1, &change);
    }
    static int32_t applyRequirements(void *context, const VkDeviceCreateInfo *required) {
        auto &host = *static_cast<Host *>(context);
        auto info = *required;
        info.queueCreateInfoCount = 1;
        info.pQueueCreateInfos = &host.queueInfo;
        if (host.create)
            return vkCreateDevice(host.physical, &info, nullptr, &host.device);
        host.grant = host.makeGrant(host.plan, host.device, host.family, host.grantIndex, &info,
                                    host.withImage ? host.image : VK_NULL_HANDLE,
                                    host.withImage ? &host.imageInfo : nullptr,
                                    VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, host.family);
        return host.grant ? 0 : -1;
    }
    void open(uint32_t sdk) {
        lib = dlopen("libfoundation_ui_native.so", RTLD_NOW);
        if (!lib)
            throw std::runtime_error(dlerror());
        auto newPlan = symbol<decltype(&ewe_ui_gpu_vulkan_plan)>(lib, "ewe_ui_gpu_vulkan_plan");
        requirements = symbol<decltype(requirements)>(lib, "ewe_ui_gpu_vulkan_requirements");
        releasePlan = symbol<decltype(releasePlan)>(lib, "ewe_ui_gpu_vulkan_release_plan");
        makeGrant = symbol<decltype(makeGrant)>(lib, "ewe_ui_gpu_vulkan_grant");
        uint32_t count = 0;
        check(vkEnumerateInstanceExtensionProperties(nullptr, &count, nullptr), "count instance extensions");
        std::vector<VkExtensionProperties> available(count);
        check(vkEnumerateInstanceExtensionProperties(nullptr, &count, available.data()),
              "instance extensions");
        std::vector<const char *> enabled;
        for (auto &ext : available)
            enabled.push_back(ext.extensionName);
        uint32_t loaderVersion = VK_API_VERSION_1_0;
        const auto enumerateVersion = reinterpret_cast<PFN_vkEnumerateInstanceVersion>(
            vkGetInstanceProcAddr(VK_NULL_HANDLE, "vkEnumerateInstanceVersion"));
        if (enumerateVersion)
            check(enumerateVersion(&loaderVersion), "Vulkan loader version");
        VkApplicationInfo app{VK_STRUCTURE_TYPE_APPLICATION_INFO};
        app.apiVersion = std::min(loaderVersion, VK_API_VERSION_1_3);
        VkInstanceCreateInfo info{VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO};
        info.pApplicationInfo = &app;
        info.enabledExtensionCount = enabled.size();
        info.ppEnabledExtensionNames = enabled.data();
        check(vkCreateInstance(&info, nullptr, &instance), "create instance");
        check(vkEnumeratePhysicalDevices(instance, &count, nullptr), "count adapters");
        std::vector<VkPhysicalDevice> adapters(count);
        check(vkEnumeratePhysicalDevices(instance, &count, adapters.data()), "enumerate adapters");
        if (adapters.empty())
            throw std::runtime_error("No Vulkan adapter");
        physical = adapters.front();
        vkGetPhysicalDeviceQueueFamilyProperties(physical, &count, nullptr);
        std::vector<VkQueueFamilyProperties> groups(count);
        vkGetPhysicalDeviceQueueFamilyProperties(physical, &count, groups.data());
        while (family < count &&
               (groups[family].queueFlags & (VK_QUEUE_GRAPHICS_BIT | VK_QUEUE_COMPUTE_BIT)) !=
                   (VK_QUEUE_GRAPHICS_BIT | VK_QUEUE_COMPUTE_BIT))
            family++;
        if (family == count)
            throw std::runtime_error("No graphics and compute queue");
        queueInfo.queueFamilyIndex = family;
        queueInfo.queueCount = 1;
        queueInfo.pQueuePriorities = &priority;
        plan = newPlan(instance, physical, app.apiVersion, sdk, enabled.size(), enabled.data());
        if (!plan)
            throw std::runtime_error("Incompatible Vulkan instance");
        create = true;
        int status = requirements(plan, this, applyRequirements);
        create = false;
        if (status != 0)
            throw std::runtime_error("Host vkCreateDevice failed: " + std::to_string(status));
        vkGetDeviceQueue(device, family, 0, &queue);
        VkCommandPoolCreateInfo poolInfo{VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO};
        poolInfo.queueFamilyIndex = family;
        check(vkCreateCommandPool(device, &poolInfo, nullptr, &pool), "create command pool");
        imageInfo.imageType = VK_IMAGE_TYPE_2D;
        imageInfo.format = VK_FORMAT_R8G8B8A8_UNORM;
        imageInfo.extent = {1, 1, 1};
        imageInfo.mipLevels = 1;
        imageInfo.arrayLayers = 1;
        imageInfo.samples = VK_SAMPLE_COUNT_1_BIT;
        imageInfo.tiling = VK_IMAGE_TILING_OPTIMAL;
        imageInfo.usage =
            VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
        check(vkCreateImage(device, &imageInfo, nullptr, &image), "create image");
        VkMemoryRequirements req;
        vkGetImageMemoryRequirements(device, image, &req);
        memory(req, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT, imageMemory);
        check(vkBindImageMemory(device, image, imageMemory, 0), "bind image");
        VkBufferCreateInfo readback{VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO};
        readback.size = 4;
        readback.usage = VK_BUFFER_USAGE_TRANSFER_DST_BIT;
        check(vkCreateBuffer(device, &readback, nullptr, &buffer), "create readback buffer");
        vkGetBufferMemoryRequirements(device, buffer, &req);
        memory(req, VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT, bufferMemory);
        check(vkBindBufferMemory(device, buffer, bufferMemory, 0), "bind buffer");
        submit([&](VkCommandBuffer cmd) {
            barrier(cmd, image, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 0,
                    VK_ACCESS_TRANSFER_WRITE_BIT);
            VkClearColorValue red{{1, 0, 0, 1}};
            auto region = range();
            vkCmdClearColorImage(cmd, image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, &red, 1, &region);
            barrier(cmd, image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                    VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_WRITE_BIT,
                    VK_ACCESS_SHADER_READ_BIT);
        });
    }
    uint32_t pixel() {
        submit([&](VkCommandBuffer cmd) {
            barrier(cmd, image, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
                    VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_ACCESS_SHADER_READ_BIT,
                    VK_ACCESS_TRANSFER_READ_BIT);
            VkBufferImageCopy copy{};
            copy.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1};
            copy.imageExtent = {1, 1, 1};
            vkCmdCopyImageToBuffer(cmd, image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, buffer, 1, &copy);
            barrier(cmd, image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                    VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_READ_BIT,
                    VK_ACCESS_SHADER_READ_BIT);
        });
        void *mapped = nullptr;
        check(vkMapMemory(device, bufferMemory, 0, 4, 0, &mapped), "map host readback");
        uint8_t bytes[4];
        memcpy(bytes, mapped, 4);
        vkUnmapMemory(device, bufferMemory);
        return (uint32_t(bytes[3]) << 24) | (uint32_t(bytes[0]) << 16) | (uint32_t(bytes[1]) << 8) | bytes[2];
    }
    VkSurfaceFormatKHR blitFormat() {
        uint32_t count = 0;
        check(vkGetPhysicalDeviceSurfaceFormatsKHR(physical, surface, &count, nullptr),
              "count surface formats");
        std::vector<VkSurfaceFormatKHR> formats(count);
        check(vkGetPhysicalDeviceSurfaceFormatsKHR(physical, surface, &count, formats.data()),
              "surface formats");
        for (auto format : formats) {
            if (format.format == VK_FORMAT_UNDEFINED)
                format.format = imageInfo.format;
            VkFormatProperties properties{};
            vkGetPhysicalDeviceFormatProperties(physical, format.format, &properties);
            if (properties.optimalTilingFeatures & VK_FORMAT_FEATURE_BLIT_DST_BIT)
                return format;
        }
        throw std::runtime_error("No Vulkan surface format accepts the image blit");
    }
    VkExtent2D openSwapchain() {
        VkAndroidSurfaceCreateInfoKHR target{VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR};
        target.window = window;
        check(vkCreateAndroidSurfaceKHR(instance, &target, nullptr, &surface),
              "create Android Vulkan surface");
        VkBool32 canPresent{};
        check(vkGetPhysicalDeviceSurfaceSupportKHR(physical, family, surface, &canPresent),
              "surface queue support");
        VkSurfaceCapabilitiesKHR caps{};
        check(vkGetPhysicalDeviceSurfaceCapabilitiesKHR(physical, surface, &caps), "surface capabilities");
        if (!canPresent || !(caps.supportedUsageFlags & VK_IMAGE_USAGE_TRANSFER_DST_BIT)) {
            throw std::runtime_error("The host queue cannot blit to this surface");
        }
        const auto format = blitFormat();
        VkSwapchainCreateInfoKHR info{VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR};
        info.surface = surface;
        info.minImageCount = caps.minImageCount;
        info.imageFormat = format.format;
        info.imageColorSpace = format.colorSpace;
        info.imageExtent = caps.currentExtent.width == UINT32_MAX ? caps.minImageExtent : caps.currentExtent;
        info.imageArrayLayers = 1;
        info.imageUsage = VK_IMAGE_USAGE_TRANSFER_DST_BIT;
        info.imageSharingMode = VK_SHARING_MODE_EXCLUSIVE;
        info.preTransform = caps.currentTransform;
        const uint32_t alpha = caps.supportedCompositeAlpha;
        info.compositeAlpha = static_cast<VkCompositeAlphaFlagBitsKHR>(alpha & (~alpha + 1));
        info.presentMode = VK_PRESENT_MODE_FIFO_KHR;
        info.clipped = VK_TRUE;
        check(vkCreateSwapchainKHR(device, &info, nullptr, &swapchain), "create host swapchain");
        return info.imageExtent;
    }
    void present(JNIEnv *env, jobject target) {
        window = ANativeWindow_fromSurface(env, target);
        if (!window)
            throw std::runtime_error("The Android surface has no native window");
        const auto extent = openSwapchain();
        uint32_t count = 0;
        check(vkGetSwapchainImagesKHR(device, swapchain, &count, nullptr), "count swapchain images");
        std::vector<VkImage> images(count);
        check(vkGetSwapchainImagesKHR(device, swapchain, &count, images.data()), "swapchain images");
        VkFenceCreateInfo fenceInfo{VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
        check(vkCreateFence(device, &fenceInfo, nullptr, &acquireFence), "create acquire fence");
        constexpr uint64_t timeoutNs = 5'000'000'000;
        uint32_t index = 0;
        check(vkAcquireNextImageKHR(device, swapchain, timeoutNs, VK_NULL_HANDLE, acquireFence, &index),
              "acquire swapchain image");
        check(vkWaitForFences(device, 1, &acquireFence, VK_TRUE, timeoutNs), "wait for acquired image");
        const auto output = images.at(index);
        submit([&](VkCommandBuffer commands) {
            barrier(commands, image, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
                    VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_ACCESS_SHADER_READ_BIT,
                    VK_ACCESS_TRANSFER_READ_BIT);
            barrier(commands, output, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 0,
                    VK_ACCESS_TRANSFER_WRITE_BIT);
            VkImageBlit blit{};
            blit.srcSubresource = blit.dstSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1};
            blit.srcOffsets[1] = {1, 1, 1};
            blit.dstOffsets[1] = {static_cast<int32_t>(extent.width), static_cast<int32_t>(extent.height), 1};
            vkCmdBlitImage(commands, image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, output,
                           VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &blit, VK_FILTER_NEAREST);
            barrier(commands, output, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR,
                    VK_ACCESS_TRANSFER_WRITE_BIT, 0);
            barrier(commands, image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                    VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_READ_BIT,
                    VK_ACCESS_SHADER_READ_BIT);
        });
        VkPresentInfoKHR info{VK_STRUCTURE_TYPE_PRESENT_INFO_KHR};
        info.swapchainCount = 1;
        info.pSwapchains = &swapchain;
        info.pImageIndices = &index;
        check(vkQueuePresentKHR(queue, &info), "present host texture");
        check(vkQueueWaitIdle(queue), "complete host presentation");
    }
    ~Host() {
        if (device) {
            if (acquireFence)
                vkDestroyFence(device, acquireFence, nullptr);
            if (swapchain)
                vkDestroySwapchainKHR(device, swapchain, nullptr);
            if (pool)
                vkDestroyCommandPool(device, pool, nullptr);
            if (buffer)
                vkDestroyBuffer(device, buffer, nullptr);
            if (image)
                vkDestroyImage(device, image, nullptr);
            if (imageMemory)
                vkFreeMemory(device, imageMemory, nullptr);
            if (bufferMemory)
                vkFreeMemory(device, bufferMemory, nullptr);
            vkDestroyDevice(device, nullptr);
        }
        if (plan)
            releasePlan(plan);
        if (surface)
            vkDestroySurfaceKHR(instance, surface, nullptr);
        if (window)
            ANativeWindow_release(window);
        if (instance)
            vkDestroyInstance(instance, nullptr);
        if (lib)
            dlclose(lib);
    }
};
static void report(JNIEnv *env, const std::exception &e) {
    env->ThrowNew(env->FindClass("java/lang/IllegalStateException"), e.what());
}
extern "C" JNIEXPORT jlong JNICALL Java_ewe_ui_compose_VulkanTestHost_open(JNIEnv *env, jobject, jint sdk) {
    try {
        auto host = std::make_unique<Host>();
        host->open(sdk);
        return reinterpret_cast<jlong>(host.release());
    } catch (const std::exception &e) {
        report(env, e);
        return 0;
    }
}
static jlong makeGrant(JNIEnv *env, jlong raw, jboolean texture, uint32_t queueIndex) {
    auto host = reinterpret_cast<Host *>(raw);
    host->withImage = texture;
    host->grantIndex = queueIndex;
    if (host->requirements(host->plan, host, Host::applyRequirements) != 0) {
        report(env, std::runtime_error("Host Vulkan device import failed"));
        return 0;
    }
    return static_cast<jlong>(host->grant);
}
extern "C" JNIEXPORT jlong JNICALL Java_ewe_ui_compose_VulkanTestHost_grant(JNIEnv *env, jobject, jlong raw,
                                                                            jboolean texture) {
    return makeGrant(env, raw, texture, 0);
}
extern "C" JNIEXPORT jlong JNICALL Java_ewe_ui_compose_VulkanTestHost_unsupportedGrant(JNIEnv *env, jobject,
                                                                                       jlong raw,
                                                                                       jboolean texture) {
    return makeGrant(env, raw, texture, UINT32_MAX);
}
extern "C" JNIEXPORT jint JNICALL Java_ewe_ui_compose_VulkanTestHost_pixel(JNIEnv *env, jobject, jlong raw) {
    try {
        return reinterpret_cast<Host *>(raw)->pixel();
    } catch (const std::exception &e) {
        report(env, e);
        return 0;
    }
}
extern "C" JNIEXPORT void JNICALL Java_ewe_ui_compose_VulkanTestHost_present(JNIEnv *env, jobject, jlong raw,
                                                                             jobject surface) {
    try {
        reinterpret_cast<Host *>(raw)->present(env, surface);
    } catch (const std::exception &error) {
        report(env, error);
    }
}
extern "C" JNIEXPORT void JNICALL Java_ewe_ui_compose_VulkanTestHost_destroy(JNIEnv *, jobject, jlong raw) {
    delete reinterpret_cast<Host *>(raw);
}
