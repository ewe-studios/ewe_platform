export async function instantiate(name, ctx) {
  const { default: wasm } = await import(`./${name}.wasm`);
  const { instance } = await WebAssembly.instantiate(wasm, {
    // Workers provide ServiceWorkerGlobalScope APIs; add host imports here
  });
  return instance.exports;
}
