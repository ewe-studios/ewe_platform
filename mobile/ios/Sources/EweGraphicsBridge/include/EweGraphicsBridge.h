#pragma once
#include <stdint.h>
uint64_t ewe_ui_gpu_retain_metal_layer(void *layer);
uint64_t ewe_ui_gpu_retain_metal_device(void *device, void *queue, void *texture);
