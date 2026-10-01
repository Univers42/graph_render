/** The studio's live-drag port: world coordinates out as force messages, disabled while no adapter exists. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { createLiveDrag } from "../src/motor/liveDrag.ts";
import type { ForceRequest } from "../src/motor/protocol.ts";

function rig(reason: string | null, ids: readonly string[] | null = ["a", "b", "c"]) {
  const sent: ForceRequest[] = [];
  const port = createLiveDrag({ ids: () => ids, disabled: () => reason, send: (request) => sent.push(request) });
  return { port, sent };
}

test("enabled follows the disabled reason", () => {
  assert.equal(rig(null).port.enabled(), true);
  assert.equal(rig("no session").port.enabled(), false);
});

test("a drag becomes force.drag with the node's id and the world point", () => {
  const { port, sent } = rig(null);
  port.drag(1, { x: 3, y: -4 });
  port.release(1);
  assert.deepEqual(sent, [{ type: "force.drag", id: "b", x: 3, y: -4 }, { type: "force.release", id: "b" }]);
});

test("a node with no id, or no graph, sends nothing", () => {
  const out = rig(null);
  out.port.drag(9, { x: 0, y: 0 });
  out.port.release(-1);
  const none = rig(null, null);
  none.port.drag(0, { x: 0, y: 0 });
  assert.deepEqual([...out.sent, ...none.sent], []);
});

test("a disabled port sends nothing", () => {
  const { port, sent } = rig("no session");
  port.drag(0, { x: 1, y: 1 });
  port.release(0);
  assert.deepEqual(sent, []);
});
