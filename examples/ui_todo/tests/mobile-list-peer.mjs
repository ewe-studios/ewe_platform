// Run a browser peer for the iPhone list synchronization test.
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import http from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";

async function waitFor(read, description, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await read();
    if (value) return value;
    await delay(50);
  }
  throw Error(description);
}

const profile = await mkdtemp(join(tmpdir(), "todo-list-peer-"));
const externalBase = process.env.EWE_TODO_PEER_BASE;
const title = process.env.EWE_TODO_PEER_TITLE ?? "Browser peer todo";
const proxy = externalBase ? null : spawn(process.execPath, ["examples/ui_todo/tests/recovery-proxy.mjs"], {
  env: { ...process.env, EWE_TODO_HISTORY: "16" }, stdio: ["ignore", "pipe", "inherit"],
});
const run = promisify(execFile);
const adb = process.env.EWE_TODO_PEER_ADB;
const serial = process.env.EWE_TODO_PEER_ANDROID_SERIAL ?? "emulator-5554";
function android(action, ...args) {
  if (adb) return run("python3", ["scripts/ui_native/check-android-todo.py", adb, serial, "peer-" + action, ...args],
    { env: { ...process.env, EWE_ANDROID_VIEW_TIMEOUT: "45" } });
}
let chrome, socket, server, reverse;
async function close() {
  server?.closeAllConnections();
  server?.close();
  socket?.close();
  if (reverse) await run(adb, ["-s", serial, "reverse", "--remove", reverse]).catch(() => {});
  for (const child of [chrome, proxy]) {
    if (child && child.exitCode == null && child.signalCode == null) {
      const exited = once(child, "exit");
      child.kill();
      await exited;
    }
  }
  await rm(profile, { recursive: true, force: true });
}
for (const signal of ["SIGTERM", "SIGINT"]) process.once(signal, () => { void close(); });

try {
  const base = externalBase ? new URL(externalBase) : new URL(
    (await once(createInterface({ input: proxy.stdout }), "line"))[0].slice("Open ".length),
  );
  if (adb) {
    assert.ok(proxy, "Use a separate test store for the Android peer");
    reverse = "tcp:" + base.port;
    await run(adb, ["-s", serial, "reverse", reverse, reverse]);
    await run(adb, ["-s", serial, "shell", "am", "force-stop", "ewe.todo"]);
    await run(adb, ["-s", serial, "shell", "am", "start", "-W", "-n", "ewe.todo/.MainActivity",
      "--es", "server", base.href]);
  }
  chrome = spawn(process.env.CHROME_PATH ?? (process.platform === "darwin"
    ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" : "google-chrome"), [
    "--headless", "--disable-background-timer-throttling", "--remote-debugging-port=0",
    `--user-data-dir=${profile}`, "about:blank",
  ], { stdio: "ignore" });

  const portFile = await waitFor(async () => {
    try { return await readFile(join(profile, "DevToolsActivePort"), "utf8"); }
    catch (error) { if (error.code !== "ENOENT") throw error; }
  }, "Chrome did not start the browser test interface");
  const [port] = portFile.split("\n");
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const target = targets.find(item => item.type === "page");
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await once(socket, "open");
  let id = 0;
  const requests = new Map();
  socket.addEventListener("message", ({ data }) => {
    const response = JSON.parse(data);
    const pending = requests.get(response.id);
    if (!pending) return;
    requests.delete(response.id);
    clearTimeout(pending.timeout);
    if (response.error) pending.reject(Error(response.error.message));
    else pending.resolve(response.result);
  });
  socket.addEventListener("close", () => {
    for (const { reject, timeout } of requests.values()) {
      clearTimeout(timeout);
      reject(Error("Chrome debug session closed"));
    }
    requests.clear();
  });
  function send(method, params) {
    assert.equal(socket.readyState, WebSocket.OPEN, "Chrome debug session must be open");
    const request = ++id;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        requests.delete(request);
        reject(Error(`Chrome debug request timed out: ${method}`));
      }, 10000);
      requests.set(request, { resolve, reject, timeout });
      socket.send(JSON.stringify({ id: request, method, params }));
    });
  }
  async function evaluate(expression) {
    const { exceptionDetails, result } = await send("Runtime.evaluate", {
      expression, returnByValue: true, userGesture: true,
    });
    assert.equal(exceptionDetails, undefined, JSON.stringify(exceptionDetails));
    return result.value;
  }
  const action = name => `button[primal\\:onclick="${name}"]:not([hidden] *)`;
  const global = selector => `document.querySelector(${JSON.stringify(selector)})`;
  const row = label => `[...document.querySelectorAll('.ewe-checklist-row')].find(
    row => !row.closest('[hidden]') && row.querySelector('.ewe-text')?.textContent === ${JSON.stringify(label)}
  )`;
  const rowAction = (label, name) => `(${row(label)})?.querySelector(${JSON.stringify(action(name))})`;
  const click = async lookup => {
    await waitFor(() => evaluate(`Boolean(${lookup})`), `Browser could not find ${lookup}`);
    await evaluate(`(${lookup}).click()`);
  };
  const seesRow = (label = title) => evaluate(`Boolean(${row(label)})`);
  async function gate(command) {
    const response = await fetch(new URL("/test/" + command, base));
    assert.equal(response.status, 200);
    return response.json();
  }
  const recordIds = new Map();
  async function status(label, expected) {
    if (expected === "absent") {
      await waitFor(async () => !(await seesRow(label)), `Browser still has ${label}`);
    } else {
      const toggle = rowAction(label, "server:toggle_todo");
      await waitFor(() => evaluate(`(${toggle})?.textContent === ${JSON.stringify(expected)}`),
        `Browser did not show ${label} with ${expected}`);
    }
    if (expected !== "absent") {
      const id = await evaluate(`(${rowAction(label, "server:toggle_todo")})?.value`);
      assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
      if (recordIds.has(label)) assert.equal(id, recordIds.get(label));
      else {
        assert.ok(![...recordIds.values()].includes(id), "Each distinct record needs its own UUID");
        recordIds.set(label, id);
      }
    }
    const display = await evaluate(`(${row(label)})?.textContent ?? null`);
    console.log(JSON.stringify({ host: "Chrome", title: label, id: recordIds.get(label), expected, row: display }));
  }
  const iosTitle = "iPhone peer todo";
  const androidTitle = "Android peer todo";
  const draft = "Keep this browser draft";
  const input = "document.querySelector('dialog:modal input')";
  async function browserDraft() {
    await click(global('button[value="done"]'));
    await click(global(action("host:open_add_dialog")));
    await waitFor(() => evaluate(`Boolean(${input})`), "Browser draft dialog");
    await evaluate(`${input}.value = ${JSON.stringify(draft)}`);
  }
  async function verifyBrowserDraft() {
    assert.equal(await evaluate(`${input}?.value`), draft);
    await click(global("dialog:modal .ewe-cancel-button"));
    assert.equal(await evaluate("document.querySelector('button[value=done]')?.getAttribute('aria-pressed')"), "true");
    assert.equal(await seesRow(iosTitle), false);
    await click(global('button[value="all"]'));
  }

  async function capturePhone(platform, state) {
    const directory = process.env.EWE_WEB_CAPTURES;
    if (!directory) return;
    await mkdir(directory, { recursive: true });
    const path = join(directory, platform + "-" + state + ".png");
    if (platform === "iphone") {
      await run("xcrun", ["simctl", "io", process.env.EWE_TODO_SIMULATOR ?? "booted", "screenshot", path]);
    } else {
      const remote = "/data/local/tmp/todo-capture-" + process.pid + ".png";
      try {
        await run(adb, ["-s", serial, "shell", "screencap", "-p", remote]);
        await run(adb, ["-s", serial, "pull", remote, path]);
      } finally { await run(adb, ["-s", serial, "shell", "rm", "-f", remote]); }
    }
  }
  const scenarios = {
    async "hold-iphone-add"() { await gate("hold-actions?client=ios"); },
    async "fail-iphone-add"() {
      await waitFor(async () => (await gate("status")).actions === 1, "Missing delayed iPhone submission");
      await capturePhone("iphone", "pending");
      await gate("fail-actions?client=ios");
    },
    async "capture-iphone-retry"() { await capturePhone("iphone", "retry"); },
    async "android-pending-retry"() {
      await gate("hold-actions?client=android");
      try {
        await Promise.all([
          android("pending-add", androidTitle),
          waitFor(async () => (await gate("status")).actions === 1, "Missing delayed Android submission", 120000),
        ]);
        await capturePhone("android", "pending");
        await gate("fail-actions?client=android");
        await android("check", androidTitle, "Try again");
        await capturePhone("android", "retry");
        await android("retry", androidTitle);
        await status(androidTitle, "Complete");
      } finally { await gate("send-actions?client=android"); }
    },
    async "pending-retry"() {
      assert.ok(proxy, "Use a separate server for pending requests");
      const record = "Retry this request";
      const capture = async name => {
        const directory = process.env.EWE_WEB_CAPTURES;
        if (!directory) return;
        await mkdir(directory, { recursive: true });
        const screenshot = await send("Page.captureScreenshot", { format: "png" });
        await writeFile(join(directory, name + ".png"), Buffer.from(screenshot.data, "base64"));
      };
      await gate("hold-actions?client=web");
      try {
        await click(global(action("host:open_add_dialog")));
        await waitFor(() => evaluate("Boolean(document.querySelector('dialog:modal input'))"), "Missing add dialog");
        await evaluate(`document.querySelector('dialog:modal input').value = ${JSON.stringify(record)}`);
        await click(global("dialog:modal .ewe-submit-button"));
        await waitFor(() => evaluate(`(${row(record)})?.textContent.includes("Pending")`), "Missing pending row");
        assert.equal(await evaluate("document.querySelector('#open-count').textContent"), "0 open todos");
        await capture("chrome-pending");
        await waitFor(async () => (await gate("status")).actions === 1, "The proxy did not receive the addition");
        await gate("fail-actions?client=web");
        await waitFor(() => evaluate(`(${row(record)})?.textContent.includes("Try again")`), "Missing retry action");
        await capture("chrome-retry");
        await click(rowAction(record, "server:add_todo"));
        await status(record,"Complete");
        assert.equal(await evaluate("document.querySelector('#open-count').textContent"), "1 open todo");
        await gate("hold-actions?client=web");
        await click(rowAction(record, "host:open_delete_dialog"));
        await click(global(action("server:confirm_delete")));
        await waitFor(() => evaluate(`(${row(record)})?.textContent.includes("Deleting…")`), "Missing pending deletion");
        assert.equal(await evaluate("document.querySelector('#open-count').textContent"), "1 open todo");
        await waitFor(async () => (await gate("status")).actions === 1, "The proxy did not receive the deletion");
        await gate("fail-actions?client=web");
        await waitFor(() => evaluate(`(${row(record)})?.textContent.includes("Try again")`), "Missing delete retry");
        await click(rowAction(record, "server:confirm_delete"));
        await status(record,"absent");
        assert.equal(await evaluate("document.querySelector('#open-count').textContent"), "0 open todos");
      } finally { await gate("send-actions?client=web"); }
    },
    async add() {
      if (proxy) await waitFor(async () => (await gate("status")).feeds >= (adb ? 3 : 2), "All clients need live subscriptions");
      if (adb) {
        await android("draft");
        await gate("pause?client=android");
      }
      await click(global(action("host:open_add_dialog")));
      await waitFor(() => evaluate("Boolean(document.querySelector('dialog:modal input'))"), "Browser add dialog");
      await evaluate(`document.querySelector('dialog:modal input').value = ${JSON.stringify(title)}`);
      await click(global("dialog:modal .ewe-submit-button"));
      await waitFor(() => seesRow(), "Browser did not receive its added row");
      if (adb) {
        await gate("resume?client=android");
        await android("check-draft", title);
        await android("show-all");
        await android("check", title, "Complete");
      }
    },
    async toggle() {
      await click(rowAction(title, "server:toggle_todo"));
      await status(title, "Reopen");
      await android("check", title, "Reopen");
    },
    async reopen() {
      await click(rowAction(title, "server:toggle_todo"));
      await status(title, "Complete");
      await android("check", title, "Complete");
    },
    async "pause-browser"() {
      await browserDraft();
      if (proxy) await gate("pause?client=web");
    },
    async "check-ios-add"() {
      if (proxy) await gate("resume?client=web");
      await verifyBrowserDraft();
      await status(iosTitle, "Complete");
      await android("check", iosTitle, "Complete");
    },
    async "check-ios-complete"() {
      await status(iosTitle, "Reopen");
      await android("check", iosTitle, "Reopen");
    },
    async "check-ios-reopen"() {
      await status(iosTitle, "Complete");
      await android("check", iosTitle, "Complete");
    },
    async "check-ios-delete"() {
      await status(iosTitle, "absent");
      await android("check", iosTitle, "absent");
    },
    async "android-add"() {
      await android("add", androidTitle);
      await status(androidTitle, "Complete");
    },
    async "android-complete"() {
      await android("complete", androidTitle);
      await status(androidTitle, "Reopen");
    },
    async "android-reopen"() {
      await android("reopen", androidTitle);
      await status(androidTitle, "Complete");
    },
    async "android-delete"() {
      await android("delete", androidTitle);
      await status(androidTitle, "absent");
    },
    async "delete-during-disconnect"() {
      if (proxy) await gate("pause?client=ios");
      try {
        await click(rowAction(title, "host:open_delete_dialog"));
        await click(global(action("server:confirm_delete")));
        await waitFor(() => evaluate("!document.querySelector('dialog:modal')"), "Browser did not confirm removal");
      } finally { if (proxy) await gate("resume?client=ios"); }
      await status(title, "absent");
      await android("check", title, "absent");
    },
  };
  await send("Page.navigate", { url: base.href });
  await android("ready");
  if (proxy) await waitFor(async () => (await gate("status")).feeds === (adb ? 2 : 1), "Peer did not subscribe");
  else assert.equal((await fetch(new URL("/initial", base))).status, 200);
  server = http.createServer(async ({ url }, response) => {
    const scenario = scenarios[url.slice(1)];
    if (!scenario) { response.writeHead(404).end(); return; }
    try { await scenario(); console.log(JSON.stringify({ scenario: url.slice(1), passed: true })); response.writeHead(204).end(); }
    catch (error) { console.error(error); response.writeHead(500).end(); }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  console.log(`EWE_TODO_URL=${base.href}`);
  console.log(`EWE_TODO_PEER_TEST_URL=http://127.0.0.1:${server.address().port}/`);
} catch (error) {
  await close();
  throw error;
}
