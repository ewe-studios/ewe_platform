// Add one todo through the public action endpoint between the two starter screen checks.
import { randomUUID } from "node:crypto";
import assert from "node:assert/strict";
import * as wire from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js";
import { FetchActionTransport, JsActionRegistry, WebActionManager } from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-actions.js";

const registry = new JsActionRegistry();
registry.declare("server", "add_todo");
const server = new FetchActionTransport({
  url: new URL("/action", process.argv[2]),
  hello: () => actions.hello(),
});
let received = false;
const actions = new WebActionManager({
  registry,
  server,
  onError: detail => { throw new Error(JSON.stringify(detail)); },
  frames: {
    receive(bytes) {
      const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
      assert.equal(frame.kindType(), wire.FrameKind.FrameMessage);
      const body = frame.kind(new wire.FrameMessage());
      const caps = body.messages(0)?.control()?.caps();
      if (caps) return actions.applyCaps(caps);
      const reply = body.unpack();
      assert.ok(reply.messages.some(message => message.data), "Expected a todo confirmation");
      assert.ok(reply.messages.every(message => !message.control), "The host rejected the action");
      received = true;
    },
  },
});
const input = { value: "Package starter check", closest: () => null, getAttribute: () => null };
for (const reply of await server.send(actions.hello())) await actions.receive(reply);
await actions.triggerStructured("server:add_todo", input, {id: randomUUID(), title: input.value}, {type: "submit"});
assert.ok(received, "Missing action reply");
