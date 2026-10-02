// Every gesture in the inspector and the node menu is an action: it lands in the log, the
// line it logs is the line a reader can type, and its one argument is read and refused in one
// place. This fires the buttons for real and reads both ends of that.
import assert from "node:assert/strict";
import { test } from "node:test";
import * as React from "react";

import type { View } from "../../../graph-render/src/view.ts";
import { Inspector, type InspectorProps } from "../../src/ui/Inspector.tsx";
import { NodeMenu, type NodeMenuProps } from "../../src/ui/NodeMenu.tsx";
import { type Desk, desk, scriptedClient } from "../desk.ts";
import { DRAWN, studioWith } from "./desk.ts";

/** One element as React made it: a click reads a prop off one of these. */
interface Drawn {
  readonly type?: unknown;
  readonly props: {
    readonly children?: unknown;
    readonly onClick?: (event: unknown) => void;
    readonly [name: string]: unknown;
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isDrawn(value: unknown): value is Drawn {
  return isObject(value) && isObject(value["props"]);
}

/** `memo` keeps the render function on `type`, and a renderer calls it. */
function callable(value: unknown): value is (props: object) => unknown {
  return typeof value === "function";
}

function renderFunction(memoized: unknown): ((props: object) => unknown) | null {
  if (!isObject(memoized)) return null;
  const value: unknown = memoized["type"];
  return callable(value) ? value : null;
}

/** The children of an element, however deeply a map nested them, without ever holding `any`. */
function kidsOf(children: unknown): readonly unknown[] {
  if (Array.isArray(children)) return children.flatMap((kid: unknown) => [...kidsOf(kid)]);
  return isObject(children) ? [children] : [];
}

/** One turn of the loop, so a `void studio.dispatch(...)` from a click has written its line. */
function tick(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

const INTERNALS = "__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE";

/**
 * WHY a dispatcher stands in here: node runs no DOM, and `renderToStaticMarkup` throws every
 * `onClick` away, so a gesture cannot be fired the way a browser fires it. `NodeMenu` reads a
 * ref and an effect, so this puts the two hooks it needs where React looks for them, and takes
 * them out again. What is left is the element tree React would have built: the handler a click
 * runs below is the panel's own, not a copy of it. The object is React's own, reached by the
 * name React exports it under, and it is put back as it was found.
 */
function standingIn(): () => void {
  const value: unknown = Object.entries(React).find(([name]) => name === INTERNALS)?.[1];
  if (!isObject(value) || !("H" in value)) assert.fail(`react exports no ${INTERNALS}: nothing to fire`);
  const internals = value;
  const was = internals["H"];
  internals["H"] = { useRef: () => ({ current: null }), useEffect: () => undefined, useMemo: () => undefined };
  return () => {
    internals["H"] = was;
  };
}

/** What a renderer would put on the page for `node`: a component renders, a host element is itself. */
function laid(node: unknown): Drawn | null {
  if (!isDrawn(node)) return null;
  const inner = node.type;
  if (!callable(inner)) return node;
  const out: unknown = inner(node.props);
  return isDrawn(out) ? out : null;
}

/** Every element under `node`, the strings and numbers skipped: they carry no gesture. */
function every(node: unknown, into: Drawn[] = []): Drawn[] {
  const one = laid(node);
  if (one === null) return into;
  into.push(one);
  for (const kid of kidsOf(one.props.children)) every(kid, into);
  return into;
}

function render(memoized: unknown, props: object): Drawn[] {
  const inner = renderFunction(memoized);
  if (inner === null) assert.fail("the panel is not a memo of a render function");
  const restore = standingIn();
  try {
    return every(inner(props));
  } finally {
    restore();
  }
}

function control(nodes: readonly Drawn[], what: string): Drawn {
  const found = nodes.find((node) => node.props["aria-label"] === what || node.props.children === what);
  if (found === undefined) assert.fail(`nothing on the panel says ${what}`);
  return found;
}

function click(node: Drawn): void {
  const gesture = node.props.onClick;
  if (gesture === undefined) assert.fail("the control carries no gesture");
  gesture({ type: "click" });
}

/** The view face the chrome is handed: it records every gesture, and a routed one never reaches it. */
type Gestured = Pick<View, "focus" | "select" | "hide" | "togglePin" | "pinned">;

function direct(seen: string[], pinned: readonly number[] = []): Gestured {
  return {
    pinned: () => pinned,
    focus: (node) => void seen.push(`focus ${node}`),
    select: (node) => void seen.push(`select ${node}`),
    hide: (nodes) => void seen.push(`hide ${nodes.join(" ")}`),
    togglePin: (node) => void seen.push(`togglePin ${node}`),
  };
}

function inspectorProps(desk: Desk, seen: string[], selection: readonly number[], selected: number): InspectorProps {
  const { meta } = desk.studio.store.get();
  return { studio: desk.studio, meta, selected, analysis: null, selection, view: direct(seen) };
}

/** `pinned` names what the menu reads to write "Pin" or "Unpin"; [2] is the open node pinned. */
function menuProps(desk: Desk, seen: string[], closed: string[], node: number, pinned: readonly number[] = [2]): NodeMenuProps {
  const ids = desk.studio.store.get().meta?.ids ?? null;
  return {
    studio: desk.studio, ids, view: direct(seen, pinned),
    menu: { node, at: { x: 10, y: 20 } }, onClose: () => void closed.push("closed"),
  };
}

/** A desk with a laid-out graph: the neighbour buttons have neighbours only once one is held. */
async function laidOut(): Promise<Desk> {
  const made = desk(scriptedClient());
  assert.equal((await made.studio.start()).ok, true, "the scripted run is drawn");
  return made;
}

function logged(desk: Desk): string | undefined {
  return desk.studio.store.get().log.at(-1)?.command;
}

/** What the view was told since `from`: a laid-out desk drew first, and that was not the gesture. */
function since(desk: Desk, from: number): readonly string[] {
  return desk.seen.calls.slice(from);
}

test("a click on a neighbour is the focus action, not a call on the view", async () => {
  const seen: string[] = [];
  const desk = await laidOut();
  const from = desk.seen.calls.length;
  const nodes = render(Inspector, inspectorProps(desk, seen, [], 0));
  click(control(nodes, "Centre Beta"));
  await tick();
  assert.deepEqual(seen, [], "the panel did not reach past the registry for the camera");
  assert.equal(logged(desk), "focus b", "the log line is the line a reader can type");
  assert.deepEqual(since(desk, from), ["focus 1"], "and the action is what centred the node");
});

test("the panel's × clears the selection and leaves the local graph alone", async () => {
  const seen: string[] = [];
  const desk = studioWith(DRAWN);
  const nodes = render(Inspector, inspectorProps(desk, seen, [], 0));
  click(control(nodes, "Close the inspector"));
  await tick();
  assert.deepEqual(seen, [], "the panel did not reach past the registry for the selection");
  assert.equal(logged(desk), "unselect");
  assert.deepEqual(desk.seen.calls, ["select -1"], "no showAll: a local graph is not dropped out of");
});

test("a click in the selection list is the focus action too", async () => {
  const seen: string[] = [];
  const desk = studioWith(DRAWN);
  const nodes = render(Inspector, inspectorProps(desk, seen, [0, 2], 2));
  click(control(nodes, "Centre Alpha"));
  await tick();
  assert.deepEqual(seen, []);
  assert.equal(logged(desk), "focus a");
  assert.deepEqual(desk.seen.calls, ["focus 0"]);
});

test("the menu's Focus is the same action the inspector uses", async () => {
  const seen: string[] = [];
  const closed: string[] = [];
  const desk = studioWith(DRAWN);
  const nodes = render(NodeMenu, menuProps(desk, seen, closed, 2));
  click(control(nodes, "Focus"));
  await tick();
  assert.deepEqual(seen, [], "the menu did not reach past the registry for the camera");
  assert.equal(logged(desk), "focus c");
  assert.deepEqual(desk.seen.calls, ["focus 2"]);
  assert.deepEqual(closed, ["closed"], "and the menu still closes behind the gesture");
});

test("a panel drawn and not touched moves nothing and logs nothing", async () => {
  const seen: string[] = [];
  const desk = studioWith(DRAWN);
  render(Inspector, inspectorProps(desk, seen, [0, 2], 2));
  render(NodeMenu, menuProps(desk, seen, [], 2));
  await tick();
  assert.deepEqual(seen, [], "nothing was pressed");
  assert.deepEqual(desk.seen.calls, [], "the drawing was not moved");
  assert.deepEqual(desk.studio.store.get().log, [], "and the log holds no line nobody ran");
});

test("every action the panels dispatch is in the registry, under a word a reader can type", async () => {
  const desk = studioWith(DRAWN);
  for (const [id, word] of [["view.focus", "focus"], ["view.unselect", "unselect"], ["view.pin", "pin"], ["view.hide", "hide"]] as const) {
    assert.equal(desk.studio.registry.find(id)?.id, id);
    assert.equal(desk.studio.registry.find(word)?.id, id);
    assert.match(word, /^[a-z]+$/, word);
  }
  const typed = await desk.studio.run("focus b");
  assert.equal(typed.ok, true);
  assert.equal(typed.command, "focus b", "the typed line and the click are one action");
  assert.deepEqual(desk.seen.calls, ["focus 1"]);
});

test("the menu's Pin is the pin action, not a call on the view", async () => {
  const seen: string[] = [];
  const closed: string[] = [];
  const desk = studioWith(DRAWN);
  const nodes = render(NodeMenu, menuProps(desk, seen, closed, 2, []));
  click(control(nodes, "Pin"));
  await tick();
  assert.deepEqual(seen, [], "the menu did not reach past the registry for the pin");
  assert.equal(logged(desk), "pin c", "the log line is the line a reader can type");
  assert.deepEqual(desk.seen.calls, ["togglePin 2"], "and the action is what pinned the node");
  assert.deepEqual(closed, ["closed"], "and the menu still closes behind the gesture");
});

test("the menu's Hide is the hide action, and it hides the one node it names", async () => {
  const seen: string[] = [];
  const closed: string[] = [];
  const desk = studioWith(DRAWN);
  const nodes = render(NodeMenu, menuProps(desk, seen, closed, 2));
  click(control(nodes, "Hide"));
  await tick();
  assert.deepEqual(seen, [], "the menu did not reach past the registry for the hiding");
  assert.equal(logged(desk), "hide c");
  assert.deepEqual(desk.seen.calls, ["hide 2"]);
  assert.deepEqual(closed, ["closed"], "and the menu still closes behind the gesture");
});