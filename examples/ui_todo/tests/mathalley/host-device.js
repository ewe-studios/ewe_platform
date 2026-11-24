// The host processes one pixel from the game through its own device lease.
(async () => {
    const canvas = document.querySelector("#mathalley canvas");
    const {BrowserDeviceExchange} = await import("/mathalley/ui_todo.js");
    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter.requestDevice();
    const texture = device.createTexture({size:[1,1], format:"rgba8unorm",
        usage:GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST |
            GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING});
    const read = async () => {
        const buffer = device.createBuffer({size:4,usage:GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ});
        try {
            const commands=device.createCommandEncoder();
            commands.copyTextureToBuffer({texture},{buffer},[1,1]);
            device.queue.submit([commands.finish()]);
            await buffer.mapAsync(GPUMapMode.READ);
            return Array.from(new Uint8Array(buffer.getMappedRange()));
        } finally { buffer.destroy(); }
    };
    let requestReady, released;
    const request = new Promise(resolve => { requestReady = resolve; });
    const returned = new Promise(resolve => { released = resolve; });
    const exchange = new BrowserDeviceExchange(requestReady);
    let grant, ticket, lease;
    try {
        // A browser can discard its canvas back buffer after presentation.
        // The screenshot records the image which reached the compositor.
        const image = await createImageBitmap(new Blob([window.gamePresentedPng],{type:"image/png"}));
        const bounds=canvas.getBoundingClientRect();
        const scale=image.width/window.innerWidth;
        const bitmap=new OffscreenCanvas(1,1);
        const reader=bitmap.getContext("2d");
        reader.drawImage(image,Math.floor((bounds.left+3)*scale),Math.floor((bounds.top+3)*scale),1,1,0,0,1,1);
        image.close();
        const pixel=reader.getImageData(0,0,1,1).data;
        device.queue.writeTexture({texture},pixel,{},[1,1]);
        const before = await read();
        if (before[0] === before[2]) throw Error("Missing game background");
        ticket = exchange.request("mathalley-host", "mathalley");
        const pending = ticket.wait();
        grant = await (await request).ready(device,texture,1n,released);
        lease = await pending;
        await lease.process_texture(`
            @group(0) @binding(0) var source: texture_2d<f32>;
            @group(0) @binding(1) var result: texture_storage_2d<rgba8unorm,write>;
            @compute @workgroup_size(1) fn main() {
                let p=vec2<i32>(0);
                let c=textureLoad(source,p,0);
                textureStore(result,p,c.bgra);
            }`,1,1,1);
        lease.release();
        if (await returned) throw Error("Host device lost");
        const after = await read();
        if (String(after)!==String([before[2],before[1],before[0],before[3]]))
            throw Error("The game pixel did not change on the host texture");
        return {before,after};
    } finally {
        lease?.free(); grant?.free(); ticket?.free(); exchange.close(); exchange.free();
        texture.destroy(); device.destroy();
    }
})()
