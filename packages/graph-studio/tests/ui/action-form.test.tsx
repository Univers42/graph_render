// The form of one action: a control per parameter, and one button when it takes none.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { studioActions } from "../../src/actions/all.ts";
import type { StudioAction } from "../../src/actions/context.ts";
import { ActionForm } from "../../src/ui/ActionForm.tsx";
import { DRAWN, markup, studioWith } from "./desk.ts";

function form(id: string): string {
  const action: StudioAction | undefined = studioActions().find((candidate) => candidate.id === id);
  if (action === undefined) throw new Error(`no action ${id}`);
  const { studio } = studioWith(DRAWN);
  return markup(createElement(ActionForm, { studio, action, state: DRAWN }));
}

test("a parameter is a labelled control of the kind it asks for", () => {
  const html = form("source.synthetic");
  assert.equal(html.match(/class="gs-field"/g)?.length, 3, "three parameters are fields");
  assert.match(html, /role="group" aria-label="Shape"/, "and the shape is a labelled group");
  for (const title of ["Nodes", "Links per node", "Seed", "Shape"]) {
    assert.ok(html.includes(title), title);
  }
  assert.match(html, /type="number"/, "Nodes is a number");
  assert.match(html, /type="range"/, "Links per node is a slider");
});

test("the choice that is chosen is the one pressed", () => {
  const html = form("source.synthetic");
  assert.match(html, /aria-pressed="true"[^>]*>vault</);
  assert.match(html, /aria-pressed="false"[^>]*>random</);
});

test("an action without parameters is one button carrying its title", () => {
  const html = form("filter.clear");
  assert.match(html, /<button[^>]*>Show everything<\/button>/);
  assert.equal(html.match(/<button/g)?.length, 1);
});
