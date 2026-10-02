// What a panel re-reads: the selector holds its reference, and a panel is handed slices only.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement, type ReactElement } from "react";

import { Hud } from "../../src/ui/Hud.tsx";
import { Legend } from "../../src/ui/Legend.tsx";
import type { StudioState } from "../../src/state/model.ts";
import { sameWhenEqual, useStudioSelector } from "../../src/ui/useStudio.ts";
import { DRAWN, fakeView, markup, studioWith } from "./desk.ts";

/** A state whose field throws when it is read: a panel that draws it must never look. */
function unread(field: string): StudioState {
  const state: StudioState = { ...DRAWN, selected: 1 };
  Object.defineProperty(state, field, { get: () => { throw new Error(`${field} was read`); } });
  return state;
}

/** A probe that shows the one field it selects, so a stale selection is visible in the markup. */
function Selected(props: { readonly studio: ReturnType<typeof studioWith>["studio"] }): ReactElement {
  const selected = useStudioSelector(props.studio, (state) => state.selected);
  return createElement("p", null, `selected ${selected}`);
}

test("an equal selection keeps its reference, which is what stops React looping", () => {
  const held = { group: "red" };
  const sameGroup = (a: { readonly group: string }, b: { readonly group: string }): boolean => a.group === b.group;
  const equal = { group: "red" };
  assert.equal(sameWhenEqual(held, equal, sameGroup), held, "an equal-but-new object keeps the old one");
  const changed = { group: "blue" };
  assert.equal(sameWhenEqual(held, changed, sameGroup), changed, "a real change is taken");
  assert.equal(sameWhenEqual(undefined, changed, sameGroup), changed, "and nothing is held at first");
  assert.equal(sameWhenEqual(held, changed, Object.is), changed, "Object.is is the default, and it is strict");
});

test("a selection follows the store, and only its own field", () => {
  const { studio } = studioWith(DRAWN);
  assert.match(markup(createElement(Selected, { studio })), /selected -1/, "the state as it is drawn");
  studio.store.set({ ...DRAWN, selected: 2 });
  assert.match(markup(createElement(Selected, { studio })), /selected 2/, "and the state after the change");
});

test("the legend never reads a field it does not draw", () => {
  const state = unread("selected");
  const html = markup(createElement(Legend, { meta: state.meta, settings: state.settings, analysis: state.analysis }));
  assert.match(html, /gs-legend/, "and it still draws its rows");
});

test("the HUD never reads a field it does not draw", () => {
  const state = unread("selected");
  const html = markup(createElement(Hud, { run: state.run, view: fakeView() }));
  assert.match(html, /gs-hud-frame/, "and it still draws its line");
});

test("the panels are memoised, so a parent that re-renders does not redraw them", () => {
  // React puts `compare` on a memo component and nowhere else, and the props are references
  // the shell holds: that pair is the whole of the bailout.
  assert.ok(Object.hasOwn(Legend, "compare"), "the legend is memoised");
  assert.ok(Object.hasOwn(Hud, "compare"), "and so is the HUD");
});