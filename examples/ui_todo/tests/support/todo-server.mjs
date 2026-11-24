import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

// Give each real server a private database, even when tests run together.
export async function startTodoServer(test) {
  const directory = await mkdtemp(join(tmpdir(), "ewe-todo-test-"));
  const child = spawn(process.env.EWE_TODO_BIN ?? "target/uat/ui_todo", ["127.0.0.1:0"], {
    env: { ...process.env, EWE_TODO_STORE: join(directory, "todo.sqlite3") },
    stdio: ["ignore", "pipe", "inherit"],
  });
  const closed = new Promise(resolve => child.once("close", resolve));
  const started = new Promise((resolve, reject) => {
    child.once("error", reject);
    const lines = createInterface({ input: child.stdout });
    lines.once("line", line => {
      const address = /^Open (http:\/\/127\.0\.0\.1:\d+)\/$/.exec(line);
      if (address) resolve(address[1]);
      else reject(new Error("Unexpected Todo server startup message: " + line));
    });
    child.once("close", code => reject(new Error("Todo server exited with code " + code)));
  });
  test.after(async () => {
    child.kill();
    await closed;
    await rm(directory, { recursive: true, force: true });
  });
  return started;
}
