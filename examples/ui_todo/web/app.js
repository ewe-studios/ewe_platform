import { installMathalley } from "./game-view.js";
import { assetBase, moduleUrl, serviceBase, serviceUrl } from "./ewe-app-config.js";
import * as bindings from "./bindings/ui_todo_bg.js";
import { WasmLoader } from "./foundation-wasm.js";
import { createApp } from "./foundation-ui-actions.js";

async function connectSender() {
    const { default: initialize, BrowserConnectSender } = await import("./rpc/foundation_ui_connectrpc.js");
    await initialize();
    const sender = new BrowserConnectSender(serviceBase, window.fetch.bind(window));
    return { close: () => sender.free(), async send(bytes) {
        const reply = await sender.exchange(bytes);
        return reply === undefined ? [] : [reply];
    } };
}

const serviceAddress = new URL(serviceBase);
const serviceIdentity = serviceAddress.pathname === "/" ? serviceAddress.origin : serviceBase;

let resourceHost;

async function installResources() {
    const { default: initialize, BrowserResources } = await import("./resources-wasm/foundation_resources.js");
    await initialize();
    return new BrowserResources(assetBase);
}

// Return the next bytes only when the caller reads the stream.
export async function openResource(name) {
    resourceHost ??= installResources().catch(error => {
        resourceHost = undefined;
        throw error;
    });
    return (await resourceHost).open(name);
}

async function tabSession() {
    const key = serviceIdentity === location.origin
        ? "ewe.todo.countdown-session"
        : "ewe.todo.countdown-session:" + serviceIdentity;
    if (!navigator.locks) return crypto.randomUUID();
    let session = sessionStorage.getItem(key) ?? crypto.randomUUID();
    for (;;) {
        const release = await new Promise((resolve, reject) => {
            navigator.locks.request(key + ":" + session, {ifAvailable: true}, async lock => {
                if (!lock) {
                    resolve(null);
                    return;
                }
                await new Promise(unlock => resolve(unlock));
            }).catch(reject);
        });
        if (release) {
            window.addEventListener("pagehide", event => {
                if (!event.persisted) release();
            });
            break;
        }
        session = crypto.randomUUID();
    }
    sessionStorage.setItem(key, session);
    return session;
}

async function runCountdown(manager, page) {
    const session = await tabSession();
    let pending = false;
    let refresh = true;
    let timer;
    let paused = false;
    const announcement = document.querySelector("#pause-announcement");

    function schedule() {
        clearTimeout(timer);
        if (paused) timer = setTimeout(tick, 1000);
    }

    async function tick(force = false) {
        if (pending || (!force && document.hidden && !paused)) return;
        pending = true;
        const reconcile = refresh;
        refresh = false;
        try {
            await manager.deliverHostPayload("host:tick_todo_countdown", page, {session, refresh: reconcile});
        } catch (error) {
            manager.onError({code: "Countdown", detail: String(error)});
        } finally {
            pending = false;
            if (refresh && !document.hidden) void tick();
            else schedule();
        }
    }

    function resume() {
        refresh = true;
        if (!document.hidden) void tick();
    }

    function keepFocusInModal(event) {
        const title = page.querySelector("#todo-pause-modal[open] #todo-pause-title");
        if (event.key !== "Tab" || !title) return;
        event.preventDefault();
        title.focus();
    }

    const observer = new MutationObserver(() => {
        const modal = page.querySelector("#todo-pause-modal[open]");
        if (Boolean(modal) === paused) return;
        paused = Boolean(modal);
        announcement.textContent = paused ? "" : "Input resumed";
        if (paused) {
            const title = modal.querySelector("#todo-pause-title");
            if (title) {
                title.tabIndex = 0;
                title.focus();
            }
        } else {
            page.querySelector("#todo-pause-button")?.focus();
        }
        schedule();
    });
    observer.observe(page, {childList: true, subtree: true, attributes: true, attributeFilter: ["open"]});
    document.addEventListener("visibilitychange", resume);
    document.addEventListener("keydown", keepFocusInModal, {capture: true});
    window.addEventListener("online", resume);
    window.addEventListener("pageshow", event => {
        if (event.persisted) resume();
    });
    window.addEventListener("pagehide", event => {
        clearTimeout(timer);
        if (event.persisted) return;
        observer.disconnect();
        document.removeEventListener("visibilitychange", resume);
        document.removeEventListener("keydown", keepFocusInModal, {capture: true});
        window.removeEventListener("online", resume);
    });
    await tick(true);
    return resume;
}

async function boot() {
    const moduleLoader = new WasmLoader();
    const params = new URL(location.href).searchParams;
    const notice = document.querySelector("#notice");
    let hostBindings;
    let resumeCountdown;
    const app = createApp({
        moduleLoader, moduleUrl, start: "init_todo_host",
        startArgs: [Number(serviceIdentity !== location.origin)],
        view: "todos", mount: document.querySelector("#page"),
        initialRoute: serviceUrl(params.has("count") ? "open-count" : "initial"),
        resolveLiveUrl: serviceUrl,
        onReconnect() {
            if (notice.value === "Transport") notice.value = "";
            resumeCountdown?.();
        },
        viewDataAction: "host:apply_todo_delivery",
        transport: params.has("socket") ? {socket: serviceUrl("socket")}
            : params.has("rpc") ? connectSender : {http: serviceUrl("action")},
        onError(error) { notice.value = error.code; console.error(error); },
        bindings: {
            importName: bindings.importName,
            createBindings() { hostBindings = bindings.createBindings(); return hostBindings; },
        },
        storage: {provider: "indexedDB", namespace: "todo:" + serviceIdentity},
    });

    const manager = await app.boot();
    const game = installMathalley(manager.deps.frames);
    document.querySelector("#page").addEventListener("ewe:data", event => {
        const bytes = event.detail.bytes;
        if (bytes && hostBindings.is_mathalley_open(bytes)) {
            event.stopImmediatePropagation();
            game.open();
        }
    }, { capture: true });
    if (!params.has("count")) {
        resumeCountdown = await runCountdown(manager, document.querySelector("#page"));
    }
    window.todoReady = true;
}
globalThis.todoResources = { open: openResource };
boot().catch(error => {
    document.querySelector("#notice").value = error?.message ?? String(error);
    console.error(error);
});
