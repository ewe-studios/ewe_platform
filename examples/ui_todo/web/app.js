import { WasmLoader } from "./foundation-wasm.js";
import { createApp } from "./foundation-ui-actions.js";

async function connectSender() {
    const { default: initialize, BrowserConnectSender } = await import("/rpc/foundation_ui_connectrpc.js");
    await initialize();
    const sender = new BrowserConnectSender(location.origin, window.fetch.bind(window));
    return { close: () => sender.free(), async send(bytes) {
        const reply = await sender.exchange(bytes);
        return reply === undefined ? [] : [reply];
    } };
}

let resourceHost;

async function installResources() {
    const { default: initialize, BrowserResources } = await import("/resources-wasm/foundation_resources.js");
    await initialize();
    return new BrowserResources(new URL("/", location.href).href);
}

// Return the next bytes only when the caller reads the stream.
export async function openResource(name) {
    resourceHost ??= installResources().catch(error => {
        resourceHost = undefined;
        throw error;
    });
    return (await resourceHost).open(name);
}

async function boot() {
    const moduleLoader = new WasmLoader();
    const storageKey = "ewe.todo.pending.v1";
    const encoder = new TextEncoder(), decoder = new TextDecoder();
    const memory = (offset, length) => new Uint8Array(moduleLoader.runtime.bridge.memory.buffer, offset, length);
    let stored;
    const params = new URL(location.href).searchParams;
    const notice = document.querySelector("#notice");
    const app = createApp({
        moduleLoader, moduleUrl: "/module.wasm", start: "init_todo_host",
        view: "todos", mount: document.querySelector("#page"),
        initialRoute: params.has("count") ? "/open-count" : "/initial",
        viewDataAction: "host:apply_todo_delivery",
        transport: params.has("socket") ? {socket: "/socket"} : params.has("rpc") ? connectSender : undefined,
        onError(error) { notice.value = error.code; console.error(error); },
        imports: {
            ewe_todo_storage_size() {
                try { stored = encoder.encode(localStorage.getItem(storageKey) ?? ""); return stored.length; }
                catch { return -1; }
            },
            ewe_todo_storage_read(offset, length) {
                if (!stored || stored.length !== length) return -1;
                memory(offset, length).set(stored); stored = undefined; return 0;
            },
            ewe_todo_storage_save(offset, length) {
                try { localStorage.setItem(storageKey, decoder.decode(memory(offset, length))); return 0; }
                catch { return -1; }
            },
        },
    });

    await app.boot();
    window.todoReady = true;
}
boot().catch(error => {
    document.querySelector("#notice").value = error?.message ?? String(error);
    console.error(error);
});
