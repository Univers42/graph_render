/**
 * `Studio.tell`: a line no command asked for that is news, such as the arm the forces tick on,
 * is logged as a success and raises no banner. `note`, beside it, is the control: it raises one.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { desk, scriptedClient } from "./desk.ts";

test("a_told_line_is_logged_and_raises_no_banner", () => {
  const { studio } = desk(scriptedClient());
  studio.tell("forces tick on the GPU (amd/rdna-2)");
  const told = studio.store.get().log.at(-1);
  assert.ok(told !== undefined);
  assert.equal(told.message, "forces tick on the GPU (amd/rdna-2)");
  assert.equal(told.ok, true);
  assert.equal(told.error, null);
  assert.equal(studio.store.get().error, null, "news is not a failure");
  studio.note("the motor worker stopped");
  assert.equal(studio.store.get().error?.title, "MotorWorkerLost", "the control: a note raises the banner");
});
