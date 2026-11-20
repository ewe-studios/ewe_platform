// Add one todo through the public action endpoint between the two starter screen checks.
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
      const reply = frame.kind(new wire.FrameMessage()).unpack();
      assert.ok(reply.messages.some(message => message.operation), "Expected a todo view update");
      assert.ok(reply.messages.every(message => !message.control), "The host rejected the action");
      received = true;
    },
  },
});
const input = { value: "Package starter check", closest: () => null, getAttribute: () => null };
await actions.trigger("server:add_todo", input, {type: "submit"});
assert.ok(received, "Missing action reply");
