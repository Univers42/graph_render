// The console's memory: what it keeps of what was typed, and where the cursor stands.
import assert from "node:assert/strict";
import { test } from "node:test";

import { EMPTY_HISTORY, pushLine, stepBack, stepForward } from "../src/ui/history.ts";

test("a line typed is a line remembered, in the order it was typed", () => {
  const once = pushLine(EMPTY_HISTORY, "fit");
  const twice = pushLine(once, "layout");
  assert.deepEqual(twice.lines, ["fit", "layout"]);
  assert.equal(twice.at, twice.lines.length, "the input is not standing on a line");
});

test("an empty line and an immediate repeat are not remembered", () => {
  const once = pushLine(EMPTY_HISTORY, "fit");
  assert.equal(pushLine(once, "   "), once);
  assert.equal(pushLine(once, "fit"), once);
  const twice = pushLine(once, "layout");
  assert.deepEqual(pushLine(twice, "layout"), twice);
});

test("at most a hundred lines are kept, oldest first dropped", () => {
  let history = EMPTY_HISTORY;
  for (let i = 0; i < 120; i += 1) history = pushLine(history, `line ${i}`);
  assert.equal(history.lines.length, 100);
  assert.equal(history.lines[0], "line 20");
  assert.equal(history.lines[99], "line 119");
});

test("walking back shows older lines and walking forward shows newer ones", () => {
  const history = pushLine(pushLine(EMPTY_HISTORY, "fit"), "layout");
  const first = stepBack(history);
  assert.equal(first.line, "layout");
  const second = stepBack(first.history);
  assert.equal(second.line, "fit");
  assert.deepEqual(stepBack(second.history), second, "there is nothing older");
  const forward = stepForward(second.history);
  assert.equal(forward.line, "layout");
  const back = stepForward(forward.history);
  assert.equal(back.line, null, "past the newest line the input is empty again");
  assert.deepEqual(stepForward(back.history), back, "there is nothing newer");
});
