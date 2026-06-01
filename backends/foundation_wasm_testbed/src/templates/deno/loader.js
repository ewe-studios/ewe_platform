export async function instantiate(name) {
  const wasmPath = new URL(`./${name}.wasm`, import.meta.url);
  const bytes = await Deno.readFile(wasmPath);
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory: new WebAssembly.Memory({ initial: 256, maximum: 512 }),
    },
  });
  return instance.exports;
}
