// Exercise a standalone host with one initializer that returns success.
globalThis.todoLifecycleCheck = (async () => {
    const {WasmLoader} = await import("/foundation-wasm.js");
    const {createApp, FetchActionTransport} = await import("/foundation-ui-actions.js");
    const moduleBytes = Uint8Array.of(
        0, 97, 115, 109, 1, 0, 0, 0,
        1, 5, 1, 96, 0, 1, 127,
        3, 2, 1, 0,
        7, 8, 1, 4, 98, 111, 111, 116, 0, 0,
        10, 6, 1, 4, 0, 65, 0, 11,
    );
    const moduleUrl = URL.createObjectURL(new Blob([moduleBytes], {type: "application/wasm"}));
    const mount = document.createElement("main");
    document.body.append(mount);
    const options = {
        moduleUrl, moduleLoader: new WasmLoader(), mount, view: "counter",
        initialRoute: "/open-count", start: "boot",
    };
    let opened = 0, closed = 0;
    try {
        const defaults = createApp(options);
        await defaults.boot();
        if (!mount.querySelector("#open-count")) throw Error("Missing default view");
        await defaults.close();
        const app = createApp({...options, onError() {}, transport() {
            opened++;
            const transport = new FetchActionTransport({url: "/action"});
            return {
                send: bytes => {
                    if (opened === 1) throw Error("Handshake unavailable");
                    return transport.send(bytes);
                },
                close() { closed++; },
            };
        }});
        const first = app.boot();
        if (app.boot() !== first) throw Error("Startup created two flights");
        const failure = await first.then(() => null, error => error.message);
        if (failure !== "Handshake unavailable" || closed !== 1) throw Error("Failed startup retained its transport");
        const results = await Promise.all([app.boot(), app.boot()]);
        if (results[0] !== results[1]) throw Error("Concurrent startup returned two managers");
        const count = Number(mount.querySelector("#open-count").textContent);
        await app.close();
        if (mount.childNodes.length) throw Error("Closing retained the view");
        const bindings = await import("/bindings/ui_todo_bg.js");
        const storage = {provider: "invalid"};
        const faults = [];
        const client = createApp({...options, onError: error => faults.push(error), moduleUrl: "/module.wasm", start: "init_todo_host",
            bindings, storage, view: "todos", initialRoute: "/initial", viewDataAction: "host:apply_todo_delivery"});
        const rejected = await client.boot().then(() => "", String);
        if (!rejected.includes("Unknown storage provider")) throw Error("Missing storage boot error");
        for (const provider of ["memory", "localStorage", "indexedDB"]) {
            Object.assign(storage, {provider, namespace: "boot-test"});
            await client.boot();
            if (!mount.querySelector("#todo-list")) throw Error("Missing Todo list for " + provider);
            await client.close();
            if (mount.childNodes.length) throw Error("Closing retained the storage-backed view");
        }
        if (faults.length) throw Error(JSON.stringify(faults));
        return [opened, closed, count];
    } finally { URL.revokeObjectURL(moduleUrl); }
})()
