// The draft a control edits: what an action is offered now, and when the form is remade.
import assert from "node:assert/strict";
import { test } from "node:test";

import { studioActions } from "../src/actions/all.ts";
import type { StudioAction } from "../src/actions/context.ts";
import { initialState } from "../src/state/model.ts";
import { commitsOnChange, signatureOf, valuesOf } from "../src/ui/draft.ts";

const STATE = initialState();
const ACTIONS: readonly StudioAction[] = studioActions();
const LONG = "ab".repeat(100);
const OTHER = `${"ab".repeat(16)}c`.repeat(3) + "tail";

function action(id: string): StudioAction {
  const found = ACTIONS.find((candidate) => candidate.id === id);
  if (found === undefined) throw new Error(`no action ${id}`);
  return found;
}

test("an action's values are what each parameter reads from the state", () => {
  assert.deepEqual(valuesOf(action("source.synthetic"), STATE), { nodes: 400, degree: 2, seed: 1, shape: "vault" });
  assert.deepEqual(valuesOf(action("source.document"), STATE), { name: "document.json", text: "" });
  assert.deepEqual(valuesOf(action("filter.clear"), STATE), {});
});

test("equal values have one signature, whatever order they were written down in", () => {
  assert.equal(signatureOf({ nodes: 4, seed: 1 }), signatureOf({ seed: 1, nodes: 4 }));
  assert.notEqual(signatureOf({ nodes: 4 }), signatureOf({ nodes: 5 }));
  assert.notEqual(signatureOf({ on: true }), signatureOf({ on: "true" }));
});

test("a long text contributes its length and its head, never the document", () => {
  const signature = signatureOf({ text: LONG });
  assert.ok(signature.length < 80, `the signature is ${signature.length} characters`);
  assert.ok(signature.includes("200"), "the length is in it");
  assert.ok(!signature.includes(LONG), "the text itself is not");
  assert.notEqual(signatureOf({ text: OTHER }), signature, "a text of another length differs");
});

test("a text of 96 characters or fewer is part of the signature as it is", () => {
  assert.ok(signatureOf({ text: "a".repeat(96) }).includes("a".repeat(96)));
  assert.equal(signatureOf({ text: "a" }), signatureOf({ text: "a" }));
  assert.notEqual(signatureOf({ text: "a" }), signatureOf({ text: "b" }));
});

test("a control that commits as it is typed is a choice, a flag or a file", () => {
  for (const control of ["list", "select", "segmented", "toggle", "file"] as const) {
    assert.equal(commitsOnChange(control), true, control);
  }
  for (const control of ["slider", "number", "text"] as const) {
    assert.equal(commitsOnChange(control), false, control);
  }
});
