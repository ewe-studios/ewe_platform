import { FoundationWasm } from "./foundation-wasm.js";
import * as ui from "./foundation-wasm-ui.js";
import * as behaviours from "./foundation-ui-behaviours.js";
import { installFrameRuntime, base64urlEncode, readViewResponse } from "./foundation-ui-frames.js";
import { FetchActionTransport, WebSocketActionTransport, WasmActionTransport, JsActionRegistry, installActionRuntime } from "./foundation-ui-actions.js";

import { installMathalley } from "./game-view.js";

async function connectSender() {
    const { default: initialize, BrowserConnectSender } = await import("/rpc/foundation_ui_connectrpc.js");
    await initialize();
    const sender = new BrowserConnectSender(location.origin, window.fetch.bind(window));
    return { async send(bytes) {
        const reply = await sender.exchange(bytes);
        return reply === undefined ? [] : [reply];
    } };
}

async function boot() {
    const runtime = new FoundationWasm();
    const dom = new ui.DomHeap({ window, document });
    let local;
    const imports = {
        abi: { ...runtime.web_abi, ...ui.domAbi(runtime, dom) },
        env: { ewe_ui_action_done: (call, slot) => local.complete(call, slot) },
    };
    const { instance } = await WebAssembly.instantiateStreaming(fetch("/module.wasm"), imports);
    runtime.init(instance);
    local = new WasmActionTransport({ exports: instance.exports, memory: () => instance.exports.memory });
    if (instance.exports.init_todo_host() !== 0) throw new Error("Cannot start the todo host");

    const notice = document.querySelector("#notice");
    const report = error => { notice.value = error.code; console.error(error); };
    const registry = new JsActionRegistry();
    for (const name of ["add_todo", "toggle_todo", "confirm_delete", "refresh_count"]) registry.declare("server", name);
    for (const name of ["open_mathalley", "open_add_dialog", "open_delete_dialog", "close_dialog", "filter_todos"]) registry.declare("host", name);
    ui.Patcher.runtime.dispatcher = new ui.EventDispatcher(() => {});
    const connections = ui.frameConnections();
    const frames = installFrameRuntime(ui, { connections, onReport: report, viewMount: document.querySelector("#page") });
    ui.registerWebComponents();
    frames.registerView("todos", document.querySelector("#page"));
    const game = installMathalley(frames);
    const params = new URL(location.href).searchParams;
    const server = params.has("socket") ? WebSocketActionTransport.connect(connections, "/socket") : params.has("rpc")
        ? await connectSender()
        : new FetchActionTransport({ url: "/action", hello: () => manager.hello() });
    const manager = installActionRuntime(ui, { registry, frames, server, local, behaviours, document, onError: report });
    const scope = ui.Hydrator.createScope(document.body);
    scope.action("notify", () => { notice.value = "Todo added"; });

    document.addEventListener("ewe:data", event => {
        const bytes = event.detail.bytes;
        if (bytes?.[0] === 0xad && new TextDecoder().decode(bytes.subarray(1)) === "open:mathalley") game.open();
        if (bytes?.[0] === 0xab && new TextDecoder().decode(bytes.subarray(1)) === "host:notify") {
            registry.hostFunction("notify")();
        }
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
