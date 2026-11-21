import { FoundationWasm } from "./foundation-wasm.js";
import * as ui from "./foundation-wasm-ui.js";
import * as behaviours from "./foundation-ui-behaviours.js";
import { installFrameRuntime, base64urlEncode, readViewResponse } from "./foundation-ui-frames.js";
import { FetchActionTransport, WasmActionTransport, JsActionRegistry, installActionRuntime } from "./foundation-ui-actions.js";

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
    for (const name of ["add_todo", "toggle_todo", "confirm_delete"]) registry.declare("server", name);
    for (const name of ["open_add_dialog", "open_delete_dialog", "close_dialog", "filter_todos"]) registry.declare("host", name);
    ui.Patcher.runtime.dispatcher = new ui.EventDispatcher(() => {});
    const frames = installFrameRuntime(ui, { onReport: report, viewMount: document.querySelector("#page") });
    ui.registerWebComponents();
    frames.registry.register(41, document.querySelector("#dialog"));
    const server = new URL(location.href).searchParams.has("rpc")
        ? await connectSender()
        : new FetchActionTransport({ url: "/action", hello: () => manager.hello() });
    const manager = installActionRuntime(ui, { registry, frames, server, local, behaviours, document, onError: report });
    const scope = ui.Hydrator.createScope(document.body);
    scope.action("notify", () => { notice.value = "Todo added"; });
    scope.action("scroll_to_top", () => window.scrollTo(0, 0));

    document.addEventListener("ewe:data", event => {
        const bytes = event.detail.bytes;
        if (bytes?.[0] === 0xab && new TextDecoder().decode(bytes.subarray(1)) === "host:notify") {
            registry.hostFunction("notify")();
        }
    });
    for (const bytes of await server.send(manager.hello())) await manager.receive(bytes);
    const response = await fetch("/initial", { headers: { "X-Ewe-Hello": base64urlEncode(manager.hello()) } });
    if (!frames.receive(await readViewResponse(response, frames.maxFrameBytes))) throw new Error("Invalid initial view");
    window.todoReady = true;
}
boot().catch(error => {
    document.querySelector("#notice").value = error?.message ?? String(error);
    console.error(error);
});
