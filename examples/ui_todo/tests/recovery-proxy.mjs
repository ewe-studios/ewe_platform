// Run a real todo server behind a gate. The test can disconnect live requests and hold snapshots.
import http from "node:http";
import {pipeline} from "node:stream";
import {once} from "node:events";
import {spawn} from "node:child_process";
import {createInterface} from "node:readline";

const backend = spawn(process.env.EWE_TODO_BIN ?? "target/uat/ui_todo", ["127.0.0.1:0"], {
  env: {...process.env, EWE_TODO_HISTORY:"1"}, stdio:["ignore","pipe","inherit"],
});
const [line] = await once(createInterface({input:backend.stdout}),"line");
const upstream = new URL(line.match(/http:\/\/\S+/)[0]);
const feeds = new Set(), waiting = new Set(), snapshots = new Set();
let blocked = false, holdSnapshots = false, loads = 0, resumes = 0;
function release(queue) { for (const run of queue) run(); queue.clear(); }
const proxy = http.createServer((req,res) => {
  const path = new URL(req.url,"http://localhost").pathname;
  if (path.startsWith("/test/")) {
    switch (path) {
      case "/test/pause":
        blocked = true;
        for (const live of feeds) live.destroy();
        break;
      case "/test/resume": blocked = false; release(waiting); break;
      case "/test/hold": holdSnapshots = true; break;
      case "/test/install": holdSnapshots = false; release(snapshots); break;
      case "/test/status": break;
      default: res.writeHead(404); res.end(); return;
    }
    res.end(JSON.stringify({feeds:feeds.size, waiting:waiting.size, snapshots:snapshots.size, loads, resumes}));
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
        feeds.add(res);
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
  if (path === "/live" && req.method === "GET" && blocked) {
    waiting.add(forward);
    res.on("close",() => waiting.delete(forward));
  } else forward();
});
proxy.listen(Number(process.argv[2] ?? 0),"127.0.0.1");
await once(proxy,"listening");
console.log("Open http://127.0.0.1:" + proxy.address().port + "/");
for (const name of ["SIGINT","SIGTERM"]) process.on(name,() => {
  backend.kill(); proxy.closeAllConnections(); proxy.close();
});
