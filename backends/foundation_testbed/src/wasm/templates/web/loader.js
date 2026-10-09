export async function instantiate(name) {
  const response = await fetch(`./${name}.wasm`);
  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory: new WebAssembly.Memory({ initial: 256, maximum: 512 }),
    },
  });
  return instance.exports;
}
