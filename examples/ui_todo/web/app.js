import { FoundationWasm } from "./foundation-wasm.js";
import * as ui from "./foundation-wasm-ui.js";
import * as behaviours from "./foundation-ui-behaviours.js";
import { installFrameRuntime, base64urlEncode, readViewResponse } from "./foundation-ui-frames.js";
import { FetchActionTransport, WebSocketActionTransport, WasmActionTransport, JsActionRegistry, installActionRuntime } from "./foundation-ui-actions.js";

async function connectSender() {
    const { default: initialize, BrowserConnectSender } = await import("/rpc/foundation_ui_connectrpc.js");
    await initialize();
    const sender = new BrowserConnectSender(location.origin, window.fetch.bind(window));
    return { async send(bytes) {
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
    const runtime = new FoundationWasm();
    const dom = new ui.DomHeap({ window, document });
    let local;
    const storageKey = "ewe.todo.pending.v1";
    const encoder = new TextEncoder(), decoder = new TextDecoder();
    const memory = (offset, length) => new Uint8Array(instance.exports.memory.buffer, offset, length);
    let stored;
    const registry = new JsActionRegistry();
    const imports = {
        abi: { ...runtime.web_abi, ...ui.domAbi(runtime, dom) },
        env: {
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
            ...WasmActionTransport.imports(() => local),
            ewe_todo_declare: (origin, pointer, length, scope) => {
                const name = new TextDecoder().decode(new Uint8Array(instance.exports.memory.buffer, pointer, length));
                registry.declare([null, "host", "server"][origin], name, ["all", "web", "mobile"][scope]);
            },
        },
    };
    const { instance } = await WebAssembly.instantiateStreaming(fetch("/module.wasm"), imports);
    runtime.init(instance);
    local = new WasmActionTransport({ exports: instance.exports, memory: () => instance.exports.memory });
    if (instance.exports.init_todo_host() !== 0) throw new Error("Cannot start the todo host");

    const notice = document.querySelector("#notice");
    const report = error => { notice.value = error.code; console.error(error); };
    ui.Patcher.runtime.dispatcher = new ui.EventDispatcher(() => {});
    const connections = ui.frameConnections();
    const frames = installFrameRuntime(ui, { connections, onReport: report, viewMount: document.querySelector("#page") });
    ui.registerWebComponents();
    frames.registerView("todos", document.querySelector("#page"));
    const params = new URL(location.href).searchParams;
    const server = params.has("socket") ? WebSocketActionTransport.connect(connections, "/socket") : params.has("rpc")
        ? await connectSender()
        : new FetchActionTransport({ url: "/action", hello: () => manager.hello() });
    const manager = installActionRuntime(ui, { registry, frames, server, local, behaviours, document, onError: report });
    document.addEventListener("ewe:data", event => {
        void manager.deliverViewData("host:apply_todo_delivery", event.target, event.detail.bytes).catch(report);
    });
    for (const bytes of await server.send(manager.hello())) await manager.receive(bytes);
    const loader = frames.registerLoader(document.querySelector("#page"), async signal => {
        const response = await fetch(params.has("count") ? "/open-count" : "/initial",
            { signal, headers: { "X-Ewe-Hello": base64urlEncode(manager.hello()) } });
        return readViewResponse(response, frames.maxFrameBytes);
    }, {onError: error => report({code:"ViewRecovery", detail:error.message})});
    await loader.open();
    window.todoReady = true;
}
boot().catch(error => {
    document.querySelector("#notice").value = error?.message ?? String(error);
    console.error(error);
});
