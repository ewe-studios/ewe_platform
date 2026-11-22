// Run against a repository HTTP server and an isolated, graphics-capable Chrome.
import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {once} from "node:events";
import {mkdtemp, rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {createInterface} from "node:readline";

const [chromePath, baseUrl] = process.argv.slice(2);
if (!chromePath || !baseUrl) throw Error("Pass the Chrome executable path and repository server URL");
const directory = await mkdtemp(join(tmpdir(),"ewe-gpu-"));
const chrome = spawn(chromePath,["--headless=new","--no-first-run","--enable-unsafe-webgpu",
  "--remote-debugging-port=0","--user-data-dir="+directory,"about:blank"], {detached: true, stdio: ["ignore", "ignore", "pipe"]});
const exited = once(chrome,"exit");
let socket;
try {
  let endpoint;
  for await (const line of createInterface({input:chrome.stderr})) {
    const match = line.match(/DevTools listening on (.*)/);
    if (match) { endpoint = new URL(match[1]); break; }
  }
  assert.ok(endpoint, "Chrome did not open its debugging port");
  const pages = await (await fetch("http://"+endpoint.host+"/json/list")).json();
  socket = new WebSocket(pages.find(page => page.type === "page").webSocketDebuggerUrl);
  await once(socket,"open");
  let sequence = 0;
  const pending = new Map();
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    pending.get(message.id)?.(message);
    pending.delete(message.id);
  };
  async function call(method, params = {}) {
    const id = ++sequence;
    let timeout;
    const reply = new Promise((resolve, reject) => {
      pending.set(id, resolve);
      timeout = setTimeout(() => reject(Error(`Chrome did not complete ${method}`)), 30_000);
    });
    try {
      socket.send(JSON.stringify({id,method,params}));
      const response = await reply;
      if (response.error) throw Error(JSON.stringify(response.error));
      return response.result;
    } finally {
      clearTimeout(timeout);
      pending.delete(id);
    }
  }
  async function evaluate(expression) {
    const result = await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true});
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  }
  await call("Page.enable");
  await call("Page.navigate",{url:new URL("/backends/foundation_ui_gpu/tests/browser/index.html",baseUrl).href});
  for (let attempt = 0; attempt < 100 && !await evaluate("typeof verifyGraphics === 'function'"); attempt++) {
    await new Promise(resolve => setTimeout(resolve,100));
  }
  const adapter = await evaluate("verifyGraphics()");
  assert.deepEqual(await evaluate("verifyHostLease()"),[0,0,255,255]);
  const {data} = await call("Page.captureScreenshot",{format:"png"});
  const pixels = await evaluate(`(async () => {
    const image = new Image();
    image.src = "data:image/png;base64," + ${JSON.stringify(data)};
    await image.decode();
    const context = new OffscreenCanvas(image.width,image.height).getContext("2d");
    context.drawImage(image,0,0);
    return [32,96,160].map(x => [...context.getImageData(x,32,1,1).data]);
  })()`);
  assert.deepEqual(pixels, [[255,0,0,255],[0,0,255,255],[0,0,255,255]]);
  await evaluate("verifyHostLoss()");
  await evaluate("verifyCancelledImport()");
  await evaluate("verifyConnectedHost()");
  await evaluate("verifyDeviceIdentity()");
  await evaluate("verifyUnavailable()");
  console.log(adapter, JSON.stringify(pixels));
} finally {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({id: 0, method: "Browser.close"}));
  }
  const deadline = new Promise(resolve => setTimeout(resolve, 3_000).unref());
  await Promise.race([exited, deadline]);
  if (chrome.exitCode === null && chrome.signalCode === null) chrome.kill("SIGKILL");
  socket?.close();
  chrome.stderr.destroy();
  await exited;
  await rm(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100});
}
