import { FoundationWasm } from "./foundation-wasm.js";
import * as ui from "./foundation-wasm-ui.js";
import * as wire from "./foundation-ui-wire.js";
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
    const prepared = new Map();
    let nextCall = 0;
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
            ewe_todo_prepared(call, pointer, length, keyPointer, keyLength) {
                if (!prepared.has(call)) throw Error("Unknown Todo submission");
                prepared.set(call, { request: memory(pointer, length).slice(), key: decoder.decode(memory(keyPointer, keyLength)) });
            },
            ewe_ui_action_done: (call, slot) => local.complete(call, slot),
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
    const send = server.send.bind(server);
    const page = document.querySelector("#page");
    const awaitingReply = new Map();
    async function reportFailure(bytes, key) {
        const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
        if (frame.kindType() !== wire.FrameKind.FrameMessage) return;
        const body = frame.kind(new wire.FrameMessage());
        for (let i = 0; i < body.messagesLength(); i++) {
            const error = body.messages(i)?.control()?.error();
            if (error) {
                const validation = error.code() === wire.ErrorCode.InvalidInput ? error.message() || "Check the title" : null;
                await manager.deliverHostPayload("host:uncertain_todo_request", page, { key, validation });
                return;
            }
        }
    }
    if (server instanceof WebSocketActionTransport) {
        const receive = server.onReply;
        server.onReply = async bytes => {
            const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
            const ack = frame.kind(new wire.FrameMessage()).ack();
            const key = awaitingReply.get(ack);
            awaitingReply.delete(ack);
            try {
                if (key) await reportFailure(bytes, key);
            } catch (error) { report(error); }
            return receive(bytes);
        };
        const disconnect = server.onDisconnect;
        server.onDisconnect = cause => {
            const pending = [...awaitingReply.values()];
            awaitingReply.clear();
            for (const key of pending) {
                manager.deliverHostPayload("host:uncertain_todo_request", page, { key }).catch(report);
            }
            disconnect(cause);
        };
    }
    server.send = async original => {
        const reader = wire.Frame.getRootAsFrame(new wire.ByteBuffer(original));
        if (reader.kindType() !== wire.FrameKind.FrameRequest || !reader.kind(new wire.FrameRequest()).action()) return send(original);
        if (nextCall >= 0xffffffff) throw Error("Todo request counter exhausted");
        const call = ++nextCall;
        prepared.set(call, null);
        let plan;
        try {
            await manager.deliverHostPayload("host:prepare_todo_request", page, { call, bytes: Array.from(original) });
            plan = prepared.get(call);
        } finally { prepared.delete(call); }
        if (!plan) throw Error("Cannot prepare Todo submission");
        if (!plan.request.length) return [];
        try {
            if (plan.key && server instanceof WebSocketActionTransport) awaitingReply.set(reader.sequenceId(), plan.key);
            const replies = await send(plan.request);
            if (plan.key) {
                for (const bytes of replies) await reportFailure(bytes, plan.key);
            }
            return replies;
        } catch (cause) {
            awaitingReply.delete(reader.sequenceId());
            if (plan.key) await manager.deliverHostPayload("host:uncertain_todo_request", page, {key:plan.key});
            throw cause;
        }
    };
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
