// The toast: what is running, and what went wrong, in one place above the graph.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { ShownError } from "../../src/state/errors.ts";
import { Toast } from "../../src/ui/Toast.tsx";
import { DRAWN, IDLE, markup, studioWith } from "./desk.ts";

const FAILURE: ShownError = {
  title: "RunRefusedError",
  code: "code 9 (E_REFUSED)",
  detail: "the layout does not take this graph",
  hint: "Pick another layout, or load a graph it accepts.",
};

function toast(state = IDLE): string {
  const { studio } = studioWith(state);
  return markup(createElement(Toast, { studio, state }));
}

test("nothing is running and nothing failed, so there is no toast", () => {
  assert.equal(toast(), "");
  assert.equal(toast(DRAWN), "");
});

test("what runs is named, and can be stopped", () => {
  const html = toast({ ...IDLE, busy: [{ seq: 3, command: "layout layout.forceatlas2" }] });
  assert.match(html, /layout layout\.forceatlas2/);
  assert.match(html, /aria-label="Stop what the motor is doing"/);
  assert.ok(!html.includes('role="alert"'), "running is not a failure");
});

test("a failure is an alert, with its title, code, detail, hint and a way past it", () => {
  const html = toast({ ...IDLE, error: FAILURE });
  assert.match(html, /role="alert"/);
  assert.ok(html.includes("RunRefusedError"));
  assert.ok(html.includes("code 9 (E_REFUSED)"));
  assert.ok(html.includes("the layout does not take this graph"));
  assert.ok(html.includes("Pick another layout, or load a graph it accepts."));
  assert.match(html, /aria-label="Dismiss the error"/);
});

test("both at once: what runs above, what failed below", () => {
  const html = toast({ ...IDLE, busy: [{ seq: 3, command: "synthetic 400 2 1 vault" }], error: FAILURE });
  assert.match(html, /gs-busy/);
  assert.match(html, /role="alert"/);
});
