// Drive the application through its normal seed, animation, and fetch interfaces.
(() => {
    const requests = performance.getEntriesByType("resource");
    if (requests.some(e => new URL(e.name).pathname.includes("/mathalley/"))) {
        throw Error("Mathalley loaded before its view opened");
    }
    const random = crypto.getRandomValues.bind(crypto);
    crypto.getRandomValues = array =>
        array instanceof BigUint64Array && array.length === 1 ? array.fill(13n) : random(array);
    const frame = window.requestAnimationFrame.bind(window);
    window.gameTestFrame = frame;
    window.gameTestTime = 0;
    window.requestAnimationFrame = callback => frame(() => {
        callback(window.gameTestTime);
        window.lastGameTick = window.gameTestTime;
    });
    const fetch = window.fetch.bind(window);
    window.fetch = async (request, ...options) => {
        const response = await fetch(request, ...options);
        if (new URL(response.url).pathname === "/mathalley/ui_todo_bg.wasm") {
            window.gameTestBinary = WebAssembly.compileStreaming(response.clone());
        }
        return response;
    };
    return true;
})()
