// Drive the recovery proxy after the browser, iPhone, and Android show one open todo.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {setTimeout as delay} from "node:timers/promises";

const base = new URL(process.argv[2]);
const peers = Number(process.argv[3] ?? 3);
const command = async name => (await fetch(new URL("/test/" + name,base))).json();
async function waitFor(check, description) {
  const deadline = Date.now() + 60000;
  let status;
  while (Date.now() < deadline) {
    status = await command("status");
    if (check(status)) return status;
    await delay(100);
  }
  throw Error(description + " " + JSON.stringify(status));
}
const add = () => execFileSync(process.execPath,["scripts/ui_native/release/seed-starter.mjs",base.href]);
const before = await waitFor(s => s.feeds === peers && s.resumes >= peers,"Waiting for initial subscribers");
await command("pause");
await command("hold");
for (let i=0; i<3; i++) add();
await command("resume");
const during = await waitFor(s => s.snapshots === peers,"Waiting for snapshots through revision four");
assert.equal(during.loads,before.loads + peers);
add();
await command("install");
const after = await waitFor(s => s.resumes >= during.resumes + peers,"Waiting for replay of revision five");
assert.equal(after.loads,during.loads);
console.log(JSON.stringify({before,during,after}));
