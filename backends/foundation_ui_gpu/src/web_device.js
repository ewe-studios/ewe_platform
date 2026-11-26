// The host retains the original WebGPU objects. The Rust lease supplies each shader and dispatch.
const devices = new WeakMap();

export function watchHostDevice(device) {
  let state = devices.get(device);
  if (!state) {
    state = {lost: false, waiters: new Set()};
    devices.set(device, state);
    device.lost.then(() => {
      state.lost = true;
      for (const resolve of state.waiters) resolve(true);
      state.waiters.clear();
    });
  }
  const {promise, resolve} = Promise.withResolvers();
  if (state.lost) resolve(true);
  else state.waiters.add(resolve);
  return {
    promise,
    close() {
      if (state.waiters.delete(resolve)) resolve(false);
    },
  };
}

export async function inspectHostDevice(device, texture) {
  if (!device?.createComputePipelineAsync || !device.queue?.onSubmittedWorkDone || !device.lost?.then) {
    throw Error("Incompatible WebGPU device");
  }
  const watch = watchHostDevice(device);
  try {
    return await Promise.race([watch.promise, inspectResources(device, texture)]);
  } finally {
    watch.close();
  }
}

async function inspectResources(device, texture) {
  await device.queue.onSubmittedWorkDone();
  if (texture) {
    if (texture.format !== "rgba8unorm" || texture.dimension !== "2d" || texture.depthOrArrayLayers !== 1 ||
        texture.sampleCount !== 1 || !(texture.usage & GPUTextureUsage.TEXTURE_BINDING) || !(texture.usage & GPUTextureUsage.COPY_DST)) {
      throw Error("Incompatible texture format, size, or usage");
    }
    // WebGPU checks device identity when the bind group is created.
    await checked(device, () => {
      const layout = device.createBindGroupLayout({entries:[{binding:0,visibility:GPUShaderStage.COMPUTE,texture:{}}]});
      device.createBindGroup({layout,entries:[{binding:0,resource:texture.createView()}]});
    });
  }
  return false;
}
async function checked(device, run) {
  device.pushErrorScope("validation");
  let result, thrown;
  try { result = run(); } catch (error) { thrown = error; }
  const validation = await device.popErrorScope();
  if (validation || thrown) throw validation ?? thrown;
  return result;
}
async function dispatch(device, shader, resources, groups, finish) {
  if (groups.some(n => !Number.isSafeInteger(n) || n < 1 || n > device.limits.maxComputeWorkgroupsPerDimension)) throw Error("Invalid workgroup count");
  const module = await checked(device, () => device.createShaderModule({code:shader}));
  const pipeline = await device.createComputePipelineAsync({layout:"auto",compute:{module,entryPoint:"main"}});
  return checked(device, () => {
    const group = device.createBindGroup({layout:pipeline.getBindGroupLayout(0),
      entries:resources.map((resource,binding) => ({binding,resource}))});
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    pass.setPipeline(pipeline);
    pass.setBindGroup(0,group);
    pass.dispatchWorkgroups(...groups);
    pass.end();
    finish(encoder);
    device.queue.submit([encoder.finish()]);
  });
}
export async function computeHost(device, shader, words, count) {
  const size = words.byteLength;
  if (!size || size > device.limits.maxStorageBufferBindingSize || size > device.limits.maxBufferSize) throw Error("Invalid buffer size");
  const storage = device.createBuffer({size,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_SRC|GPUBufferUsage.COPY_DST});
  const output = device.createBuffer({size,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
  try {
    device.queue.writeBuffer(storage,0,words);
    await dispatch(device,shader,[{buffer:storage}],[count,1,1],encoder => encoder.copyBufferToBuffer(storage,0,output,0,size));
    await output.mapAsync(GPUMapMode.READ);
    return new Uint32Array(output.getMappedRange().slice(0));
  } finally { storage.destroy(); output.destroy(); }
}
export async function processHost(device, texture, shader, x, y, z) {
  if (!texture) throw Error("No leased texture");
  const size = [texture.width,texture.height,1];
  const output = device.createTexture({size,format:"rgba8unorm",usage:GPUTextureUsage.STORAGE_BINDING|GPUTextureUsage.COPY_SRC});
  try {
    await dispatch(device,shader,[texture.createView(),output.createView()],[x,y,z],
      encoder => encoder.copyTextureToTexture({texture:output},{texture},size));
  } finally { output.destroy(); }
}
export async function finishHost(device) { await device.queue.onSubmittedWorkDone(); }
