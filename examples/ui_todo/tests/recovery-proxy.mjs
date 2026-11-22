// Run a real todo server behind a gate. The test can disconnect live requests and hold snapshots.
import http from "node:http";
import {pipeline} from "node:stream";
import {once} from "node:events";
import {spawn} from "node:child_process";
import {createInterface} from "node:readline";

const backend = spawn(process.env.EWE_TODO_BIN ?? "target/uat/ui_todo", ["127.0.0.1:0"], {
  env: {...process.env, EWE_TODO_HISTORY:process.env.EWE_TODO_HISTORY ?? "1"}, stdio:["ignore","pipe","inherit"],
});
const [line] = await once(createInterface({input:backend.stdout}),"line");
const upstream = new URL(line.match(/http:\/\/\S+/)[0]);
const feeds = new Map(), waiting = new Map(), snapshots = new Set();
const blocked = new Set();
const heldActions = new Set(), actions = new Map();
let holdSnapshots = false, loads = 0, resumes = 0;
function release(queue) { for (const run of queue) run(); queue.clear(); }
const proxy = http.createServer((req,res) => {
  const url = new URL(req.url,"http://localhost");
  const path = url.pathname;
  const agent = req.headers["user-agent"] ?? "";
  const client = agent.includes("Chrome/") ? "web" : agent.includes("CFNetwork/") ? "ios"
    : agent.includes("okhttp/") ? "android" : "other";
  const paused = peer => blocked.has("*") || blocked.has(peer);
  if (path.startsWith("/test/")) {
    const target = url.searchParams.get("client") ?? "*";
    switch (path) {
      case "/test/pause":
        blocked.add(target);
        for (const [live, peer] of feeds) if (paused(peer)) live.destroy();
        break;
      case "/test/resume":
        blocked.delete(target);
        for (const [run, peer] of waiting) {
          if (!paused(peer)) { waiting.delete(run); run(); }
        }
        break;
      case "/test/hold-actions": heldActions.add(target); break;
      case "/test/send-actions":
      case "/test/fail-actions":
        heldActions.delete(target);
        for (const [response, entry] of actions) {
          if (target !== "*" && entry.client !== target) continue;
          actions.delete(response);
          if (path === "/test/send-actions") entry.forward();
          else { entry.request.resume(); response.writeHead(503).end(); }
        }
        break;
      case "/test/hold": holdSnapshots = true; break;
      case "/test/install": holdSnapshots = false; release(snapshots); break;
      case "/test/status": break;
      default: res.writeHead(404); res.end(); return;
    }
    res.end(JSON.stringify({feeds:feeds.size, waiting:waiting.size, snapshots:snapshots.size, actions:actions.size, loads, resumes}));
    return;
  }
  if (process.env.EWE_TEST_EXPIRE_HTTP === "1" && path === "/live" && req.method === "POST" && holdSnapshots) {
    resumes++;
    req.resume();
    res.writeHead(410);
    res.end();
    return;
  }
  function forward() {
    if (res.destroyed) return;
    const out = http.request(new URL(req.url,upstream),{method:req.method,headers:req.headers}, reply => {
      if (path === "/initial") loads++;
      if (path === "/live" && req.method === "POST") resumes++;
      if (path === "/live" && req.method === "GET") {
        feeds.set(res, client);
        res.on("close",() => feeds.delete(res));
      }
      function send() {
        if (res.destroyed) { reply.destroy(); return; }
        res.writeHead(reply.statusCode,reply.headers);
        res.flushHeaders();
        pipeline(reply,res,() => {});
      }
      if (path === "/initial" && holdSnapshots) snapshots.add(send);
      else send();
    });
    res.on("close",() => out.destroy());
    out.on("error",error => { if (!res.destroyed) res.destroy(error); });
    req.pipe(out);
  }
  if (path === "/action" && req.method === "POST" && (heldActions.has("*") || heldActions.has(client))) {
    actions.set(res, {client, request:req, forward});
    res.once("close", () => actions.delete(res));
  } else if (path === "/live" && req.method === "GET" && paused(client)) {
    waiting.set(forward, client);
    res.on("close",() => waiting.delete(forward));
  } else forward();
});
proxy.listen(Number(process.argv[2] ?? 0),"127.0.0.1");
await once(proxy,"listening");
console.log("Open http://127.0.0.1:" + proxy.address().port + "/");
for (const name of ["SIGINT","SIGTERM"]) process.on(name,() => {
  backend.kill(); proxy.closeAllConnections(); proxy.close();
});
