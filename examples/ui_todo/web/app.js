import { moduleUrl } from "./ewe-app-config.js";
import * as bindings from "./bindings/ui_todo_bg.js";
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
    const params = new URL(location.href).searchParams;
    const notice = document.querySelector("#notice");
    const app = createApp({
        moduleLoader, moduleUrl, start: "init_todo_host",
        view: "todos", mount: document.querySelector("#page"),
        initialRoute: params.has("count") ? "/open-count" : "/initial",
        viewDataAction: "host:apply_todo_delivery",
        transport: params.has("socket") ? {socket: "/socket"} : params.has("rpc") ? connectSender : undefined,
        onError(error) { notice.value = error.code; console.error(error); },
        bindings,
        storage: {provider: "indexedDB", namespace: "todo:" + location.origin},
    });

    await app.boot();
    window.todoReady = true;
}
globalThis.todoResources = { open: openResource };
boot().catch(error => {
    document.querySelector("#notice").value = error?.message ?? String(error);
    console.error(error);
});
