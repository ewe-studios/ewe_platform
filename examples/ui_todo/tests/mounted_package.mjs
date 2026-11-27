// Check a built Todo package with a browser and a proxy that strips the mount.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer, request } from "node:http";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const [binary, chromium, assets = "pkg", services = "/todo/"] = process.argv.slice(2);
assert(binary && chromium, "Pass the Todo executable and Chromium executable");
assert(services.startsWith("/") && services.endsWith("/"));
const scratch = await mkdtemp(join(tmpdir(), "todo-mounted-"));
const children = [];
let proxy;
let devtools;
const requests = [];
const actionSockets = new Set();

async function waitFor(read, message) {
    const deadline = performance.now() + 30_000;
    while (performance.now() < deadline) {
        const value = await read();
        if (value) return value;
        await delay(50);
    }
    throw new Error(message);
}

function start(command, args) {
    const child = spawn(command, args, {stdio: ["ignore", "pipe", "pipe"]});
    children.push(child);
    child.stderr.pipe(process.stderr);
    return child;
}

function stop(child) {
    if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
    return new Promise(accept => {
        child.once("exit", accept);
        child.kill();
    });
}

function childLine(child, match) {
    return new Promise((accept, reject) => {
        let output = "";
        child.stdout.on("data", chunk => {
            output += chunk;
            const found = output.match(match);
            if (found) accept(found[1]);
        });
        child.once("error", reject);
        child.once("exit", code => reject(new Error("Child exited with " + code)));
    });
}

function route(path) {
    if (path.startsWith("/todo/" + assets + "/")) return path.slice("/todo".length);
    if (path.split("?")[0] === "/todo/") return "/" + (path.split("?")[1] ? "?" + path.split("?")[1] : "");
    for (const base of [services, "/other/"]) {
        if (path.startsWith(base)) return "/" + path.slice(base.length);
    }
    return undefined;
}

async function openProxy(upstream) {
    proxy = createServer((req, res) => {
        const path = route(req.url);
        if (path === undefined) { res.writeHead(404).end(); return; }
        const outgoing = request(upstream, {path, method: req.method, headers: req.headers}, reply => {
            requests.push({path: new URL(req.url, upstream).pathname, method: req.method,
                status: reply.statusCode, media: reply.headers["content-type"]});
            res.writeHead(reply.statusCode, reply.headers);
            reply.pipe(res);
            res.once("close", () => reply.destroy());
        });
        req.pipe(outgoing);
        outgoing.once("error", error => res.destroy(error));
    });
    proxy.on("upgrade", (req, client, head) => {
        const path = route(req.url);
        if (path === undefined) { client.end("HTTP/1.1 404 Not Found\r\n\r\n"); return; }
        const target = new URL(upstream);
        const server = connect(Number(target.port), target.hostname, () => {
            requests.push({path: req.url, method: "Upgrade"});
            actionSockets.add(server);
            const headers = req.rawHeaders.reduce((lines, value, i) =>
                i % 2 === 0 ? lines + value + ": " : lines + value + "\r\n", "");
            server.write(`GET ${path} HTTP/1.1\r\n${headers}\r\n`);
            server.write(head);
            client.pipe(server).pipe(client);
        });
        client.on("error", () => server.destroy());
        server.on("error", () => client.destroy());
        client.once("close", () => server.destroy());
        server.once("close", () => {
            actionSockets.delete(server);
            client.destroy();
        });
    });
    await new Promise((accept, reject) => {
        proxy.once("error", reject);
        proxy.listen(0, "127.0.0.1", accept);
    });
    return `http://127.0.0.1:${proxy.address().port}`;
}

async function openBrowser() {
    const profile = join(scratch, "browser");
    start(resolve(chromium), ["--headless=new", "--remote-debugging-port=0",
        "--user-data-dir=" + profile, "--enable-gpu", "--enable-unsafe-webgpu",
        "--disable-background-timer-throttling", "--disable-renderer-backgrounding",
        "--disable-backgrounding-occluded-windows", "--disable-background-networking",
        "--disable-component-update", "--no-first-run", "about:blank"]);
    const endpoints = await waitFor(async () => {
        try { return (await readFile(join(profile, "DevToolsActivePort"), "utf8")).trim().split("\n"); }
        catch (error) { if (error.code !== "ENOENT") throw error; }
    }, "Chromium did not start its local control endpoint");
    devtools = new WebSocket(`ws://127.0.0.1:${endpoints[0]}${endpoints[1]}`);
    await new Promise((accept, reject) => {
        devtools.addEventListener("open", accept, {once: true});
        devtools.addEventListener("error", reject, {once: true});
    });
    let seq = 0;
    const calls = new Map();
    const sockets = new Map();
    devtools.addEventListener("message", ({data}) => {
        const {id, result, error, method, params, sessionId} = JSON.parse(data);
        if (method === "Network.webSocketCreated") {
            sockets.set(sessionId, {id: params.requestId, frames: 0, replies: 0});
        }
        if (method === "Network.webSocketFrameSent" || method === "Network.webSocketFrameReceived") {
            const socket = sockets.get(sessionId);
            if (socket?.id === params.requestId && params.response.opcode === 2) {
                if (method === "Network.webSocketFrameSent") socket.frames++;
                else socket.replies++;
            }
        }
        if (calls.has(id)) {
            const {accept, reject} = calls.get(id);
            calls.delete(id);
            if (error) reject(new Error(JSON.stringify(error)));
            else accept(result);
        }
    });
    const cdp = (method, params = {}, sessionId) => new Promise((accept, reject) => {
        const id = ++seq;
        calls.set(id, {accept, reject});
        devtools.send(JSON.stringify({id, method, params, sessionId}));
    });
    cdp.socket = sessionId => sockets.get(sessionId);
    return cdp;
}

async function tab(cdp, url) {
    const {targetId} = await cdp("Target.createTarget", {url: "about:blank"});
    const {sessionId} = await cdp("Target.attachToTarget", {targetId, flatten: true});
    const call = (method, params) => cdp(method, params, sessionId);
    const evaluate = async expression => {
        const outcome = await call("Runtime.evaluate", {expression, awaitPromise: true, returnByValue: true});
        if (outcome.exceptionDetails) throw new Error(outcome.exceptionDetails.exception?.description);
        return outcome.result.value;
    };
    await call("Network.enable");
    const navigation = await call("Page.navigate", {url});
    assert(!navigation.errorText, navigation.errorText);
    await waitFor(() => evaluate("window.todoReady"), "Todo did not start at " + url)
        .catch(async error => { throw new Error(error.message + ": " + await evaluate("document.body.innerText")); });
    return {
        evaluate,
        socket: () => cdp.socket(sessionId),
        activate: () => cdp("Target.activateTarget", {targetId}),
        close: () => cdp("Target.closeTarget", {targetId}),
    };
}

async function add(page, name) {
    await page.evaluate(`[...document.querySelectorAll("button")].find(
        button => button.getAttribute("primal:onclick") === "host:open_add_dialog").click()`);
    await waitFor(() => page.evaluate("!!document.querySelector('dialog:modal input')"), "Local add dialog did not open");
    await page.evaluate(`(() => {
        const input = document.querySelector("dialog:modal input");
        input.value = ${JSON.stringify(name)};
        input.dispatchEvent(new Event("input", {bubbles: true}));
        document.querySelector("dialog:modal .ewe-submit-button").click();
    })()`);
    await sees(page, name);
}

async function seedLegacy(page, title) {
    await page.evaluate(`(() => {
        const id = crypto.randomUUID();
        const record = {change: {Add: {creation_id: id, title: ${JSON.stringify(title)}}}, retry: false};
        localStorage.setItem("ewe.todo.pending.v1", JSON.stringify({["add:" + id]: record}));
    })()`);
}

async function sees(page, name) {
    await waitFor(() => page.evaluate(`document.querySelector("#page").innerText.includes(${JSON.stringify(name)})`),
        "Todo list did not receive " + name);
}

const store = join(scratch, "todo.sqlite3");
let app = start(resolve(binary), ["127.0.0.1:0", "--store", store]);
try {
    let upstream = await childLine(app, /Open (http:\/\/[^\s]+)\n/);
    const cdp = await openBrowser();
    if (services === "/todo/") {
        const root = await tab(cdp, upstream);
        await add(root, "Root namespace");
        await waitFor(() => root.evaluate('document.querySelector("#notification")?.textContent === "Todo added"'),
            "Root mutation was not confirmed before restart");
        const rootName = "ewe.kv.todo:" + new URL(upstream).origin;
        assert(await root.evaluate(`indexedDB.databases().then(dbs => dbs.some(db => db.name === ${JSON.stringify(rootName)}))`));
        await seedLegacy(root, "Original legacy record");
        const previous = await tab(cdp, upstream);
        await sees(previous, "Original legacy record");
        await previous.close();
        await root.close();
        await stop(app);

        app = start(resolve(binary), ["127.0.0.1:0", "--store", store]);
        upstream = await childLine(app, /Open (http:\/\/[^\s]+)\n/);
        const restored = await tab(cdp, upstream);
        await sees(restored, "Root namespace");
        await restored.close();
    }

    const publicOrigin = await openProxy(upstream);
    const appMount = publicOrigin + "/todo/";
    assert.equal((await fetch(publicOrigin + "/bundle.js")).status, 404);
    const first = await tab(cdp, appMount);
    await seedLegacy(first, "Root legacy record");
    const second = await tab(cdp, appMount + "?socket");
    assert(!await second.evaluate('document.querySelector("#page").innerText.includes("Root legacy record")'));
    const rpc = await tab(cdp, appMount + "?rpc");
    for (const [page, name] of [[first, "Fetch action"], [second, "Socket action"], [rpc, "Connect action"]]) {
        await add(page, name);
        await Promise.all([first, second, rpc].map(peer => sees(peer, name)));
    }

    const priorSocket = second.socket();
    assert(priorSocket && actionSockets.size === 1);
    const hidden = await second.evaluate("document.hidden");
    const {frames: priorFrames, replies: priorReplies} = priorSocket;
    await second.activate();
    if (hidden) {
        await waitFor(() => priorSocket.frames > priorFrames && priorSocket.replies > priorReplies,
            "Visible tab did not complete its countdown refresh");
    }
    for (const socket of actionSockets) socket.destroy();
    await waitFor(() => {
        const current = second.socket();
        return current && current.id !== priorSocket.id && current.frames > 1;
    }, "Reconnected socket did not send a handshake and countdown refresh")
        .catch(async error => {
            const detail = {priorSocket, currentSocket: second.socket(),
                notice: await second.evaluate("document.querySelector('#notice').value")};
            throw new Error(error.message + " " + JSON.stringify(detail));
        });

    const named = await first.evaluate(`(async () => {
        const resource = await window.todoResources.open("art/todo.svg");
        const decoder = new TextDecoder();
        let contents = "";
        try {
            for (let chunk; (chunk = await resource.nextChunk()) !== undefined;) {
                contents += decoder.decode(chunk, {stream: true});
            }
            return contents + decoder.decode();
        } finally { resource.free(); }
    })()`);
    assert(named.includes("<svg"));
    const assetRoute = "/todo/" + assets + "/";
    assert(!requests.some(entry => entry.path.startsWith(assetRoute + "mathalley/")));
    await first.activate();
    await first.evaluate(`[...document.querySelectorAll("button")].find(
        button => button.getAttribute("primal:onclick") === "host:open_mathalley").click()`);
    await waitFor(() => first.evaluate(`document.querySelector('#mathalley[aria-busy="false"] canvas') !== null`),
        "Mathalley did not start");
    await first.evaluate(`document.querySelector("#mathalley button").click()`);
    await sees(first, "Connect action");
    assert(await first.evaluate("!document.querySelector('#mathalley') && !document.querySelector('#page').hidden"));

    const peers = [first, second, rpc];
    const row = `[...document.querySelectorAll(".ewe-checklist-row")].find(item =>
        item.querySelector(".ewe-text")?.textContent === "Connect action")`;
    const rowControl = action => `[...(${row}).querySelectorAll("button")].find(
        button => button.getAttribute("primal:onclick") === ${JSON.stringify(action)})`;
    const toggle = rowControl("server:toggle_todo");
    for (const [page, expected] of [[first, "Reopen"], [second, "Complete"]]) {
        await page.evaluate(toggle + ".click()");
        await Promise.all(peers.map(peer => waitFor(
            () => peer.evaluate(`(${row}) && ${toggle}.textContent === ${JSON.stringify(expected)}`),
            "Peer did not update the row to " + expected)));
    }
    await rpc.evaluate(rowControl("host:open_delete_dialog") + ".click()");
    await waitFor(() => rpc.evaluate("!!document.querySelector('dialog:modal')"),
        "Delete dialog did not open");
    await rpc.evaluate(`[...document.querySelectorAll("dialog:modal button")].find(
        button => button.getAttribute("primal:onclick") === "host:submit_delete").click()`);
    await Promise.all(peers.map(peer => waitFor(
        () => peer.evaluate(`!(${row})`), "Peer still shows the deleted row")));

    for (const [path, media] of [
        ["/todo/", "text/html"], [assetRoute + "theme.css", "text/css"],
        [assetRoute + "art/todo.svg", "image/svg+xml"], [assetRoute + "bundle.js", "text/javascript"],
        [assetRoute + "module.wasm", "application/wasm"],
        [assetRoute + "rpc/foundation_ui_connectrpc_bg.wasm", "application/wasm"],
        [assetRoute + "resources-wasm/foundation_resources_bg.wasm", "application/wasm"],
        [assetRoute + "mathalley/ui_todo_bg.wasm", "application/wasm"],
    ]) {
        assert(requests.some(entry => entry.path === path && entry.status === 200 &&
            entry.media?.split(";")[0] === media), path + " did not return " + media);
    }
    for (const [path, method] of [["initial", "GET"], ["action", "POST"], ["live", "GET"],
        ["socket", "Upgrade"], ["ewe.ui.v1.Actions/Exchange", "POST"]]) {
        assert(requests.some(entry => entry.path === services + path && entry.method === method),
            method + " did not reach " + services + path);
    }
    const serviceName = "ewe.kv.todo:" + publicOrigin + services;
    assert(await first.evaluate(`indexedDB.databases().then(dbs => dbs.some(db => db.name === ${JSON.stringify(serviceName)}))`));
    if (services === "/todo/") {
        const other = await tab(cdp, publicOrigin + "/other/");
        assert(!await other.evaluate('document.querySelector("#page").innerText.includes("Root legacy record")'));
        await add(other, "Other mount");
        const otherName = "ewe.kv.todo:" + publicOrigin + "/other/";
        assert(await other.evaluate(`indexedDB.databases().then(dbs =>
            dbs.some(db => db.name === ${JSON.stringify(serviceName)}) &&
            dbs.some(db => db.name === ${JSON.stringify(otherName)}))`));
    }
    const scriptPaths = await first.evaluate(`performance.getEntriesByType("resource")
        .map(entry => new URL(entry.name).pathname).filter(path => path.endsWith(".js"))`);
    assert.deepEqual(scriptPaths, [assetRoute + "bundle.js"]);
    console.log(JSON.stringify(requests));
} finally {
    devtools?.close();
    proxy?.close();
    await Promise.all(children.map(stop));
    await rm(scratch, {recursive: true, force: true, maxRetries: 5});
}
