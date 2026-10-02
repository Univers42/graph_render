/** The filter and group actions: what is hidden, and what the group list holds. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { FILTER_ACTIONS } from "../src/actions/filter.ts";
import { GROUP_ACTIONS } from "../src/actions/groups.ts";
import type { StudioAction, StudioContext } from "../src/actions/context.ts";
import { type Args, ActionRefusal, type RawArgs, createRegistry } from "../src/actions/registry.ts";
import { formatCommand, parseCommand } from "../src/console/parse.ts";
import { GROUP_PALETTE } from "../src/look/palette.ts";
import { NODE_KINDS } from "../src/source/ingest.ts";
import { metaOf } from "../src/source/meta.ts";
import { initialState } from "../src/state/model.ts";
import { type Group, type Settings, DEFAULT_SETTINGS, withFilter, withGroups } from "../src/state/settings.ts";
import type { ViewFace } from "../src/studio/pipeline.ts";
import { node } from "./support.ts";

const ACTIONS: readonly StudioAction[] = [...FILTER_ACTIONS, ...GROUP_ACTIONS];
const REGISTRY = createRegistry(ACTIONS);

const NODES = [
  node("a", { group: "Alpha", weight: 1, kind: "database", label: "Graph notes" }),
  node("b", { group: "Beta", weight: 0.5, kind: "note", label: "Layout design" }),
  node("c", { group: "Alpha", weight: 0.25, label: "Memory log" }),
  node("d", { weight: 0, kind: "tag", label: "Orphan" }),
];
const META = metaOf(NODES, ["a", "b", "c", "d"], { source: Uint32Array.of(0, 0), target: Uint32Array.of(1, 2) });
const GROUPS: readonly Group[] = [
  { name: "Alpha", query: "kind:database", colour: "#7c9cf5" },
  { name: "Beta", query: "kind:note", colour: "#f2a65a" },
];
const BASE: Settings = withGroups(withFilter(DEFAULT_SETTINGS, { text: "graph" }), GROUPS);

/** The group at `i`, or the test stops where the document is not what it should be. */
function at(groups: readonly Group[], i: number): Group {
  const found = groups[i];
  if (found === undefined) throw new Error(`no group at ${i}`);
  return found;
}

function silentView(calls: string[]): ViewFace {
  const noop = (): void => undefined;
  return {
    setFrame: noop, setStyle: noop, setTheme: noop, setLabels: noop,
    fit: () => void calls.push("fit"),
    reset: () => void calls.push("reset"),
    zoomBy: () => void calls.push("zoomBy"), panBy: () => void calls.push("panBy"),
    limits: () => ({ min: 0.02, max: 40 }),
    focus: () => void calls.push("focus"), select: () => void calls.push("select"),
    local: (node) => [node], showAll: () => void calls.push("showAll"),
    pinned: () => [], togglePin: () => void calls.push("togglePin"), hide: () => void calls.push("hide"),
    on: () => noop,
    toPNG: () => Promise.resolve(new Blob(["png"], { type: "image/png" })),
    setCamera: () => void calls.push("setCamera"),
    // The 3D camera's four faces. A silent view holds no frame, so it has no orbit.
    orbit: () => null, projected: () => null, setOrbit: noop, resetOrbit: noop,
    frame: () => { throw new Error("the silent view holds no frame"); },
    viewport: () => ({ width: 800, height: 600 }),
  };
}

interface Studio {
  readonly context: StudioContext;
  readonly calls: string[];
  readonly settings: () => Settings;
}

function studio(settings: Settings = BASE): Studio {
  const calls: string[] = [];
  let held = settings;
  const look = (next: Settings): { message: string } => {
    held = next;
    return { message: "restyled" };
  };
  const context: StudioContext = {
    state: () => ({ ...initialState(held), meta: META }),
    view: silentView(calls),
    stop: () => false,
    save: () => undefined,
    clearLog: () => undefined,
    animation: { start: () => ({ message: "animating" }), cancel: () => ({ message: "cancelled" }) },
    actions: () => ACTIONS,
    recall: () => null,
    apply: (next) => Promise.resolve(look(next)),
    look,
    bytes: () => null,
    neighbours: () => [],
    fitResults: () => ({ message: "fitted" }),
    reveal: () => undefined,
  };
  return { context, calls, settings: () => held };
}

async function run(desk: Studio, id: string, raw: RawArgs = {}): Promise<string> {
  const { action, args } = REGISTRY.resolve(id, raw, desk.context.state());
  return (await action.run(desk.context, args)).message;
}

async function refusal(desk: Studio, id: string, raw: RawArgs): Promise<ActionRefusal> {
  const { action, args } = REGISTRY.resolve(id, raw, desk.context.state());
  try {
    await action.run(desk.context, args);
  } catch (error) {
    assert.ok(error instanceof ActionRefusal, `${id} threw ${String(error)}`);
    return error;
  }
  throw new Error(`\`${id}\` ran; it should have refused`);
}

test("every filter and group action is a named command in the Filters section", () => {
  for (const action of ACTIONS) {
    assert.match(action.id, /^[a-z]+(\.[a-z]+)+$/, action.id);
    assert.match(action.alias, /^[a-z]+$/, action.alias);
    assert.equal(action.section, "Filters", action.id);
    assert.equal(action.available?.(studio().context.state()) ?? null, null, action.id);
  }
});

test("the line a command formats is the line that runs it", () => {
  for (const action of ACTIONS) {
    const state = studio().context.state();
    const defaults: Args = Object.fromEntries(action.params.map((spec) => [spec.name, spec.value(state)]));
    const line = formatCommand(action, defaults);
    const command = parseCommand(line, REGISTRY);
    assert.equal(command.id, action.id, line);
    const { args } = REGISTRY.resolve(command.id, command.raw, state);
    assert.deepEqual(args, defaults, line);
  }
});

const TAG = { query: "", text: "", hiddenKinds: [], hiddenGroups: [], orphans: false, existingOnly: false, minDegree: 0, relayout: false };

test("a query changes the query and nothing else", async () => {
  const desk = studio();
  const query = "kind:database AND NOT (kind:note OR degree:>=2)";
  await run(desk, "filter.query", { query });
  assert.deepEqual(desk.settings().filter, { ...TAG, text: "graph", query });
});

test("a query the grammar refuses never lands in the document", async () => {
  const desk = studio();
  const refused = await refusal(desk, "filter.query", { query: "kind:(note" });
  assert.match(refused.message, /\S/);
  assert.equal(JSON.stringify(desk.settings()), JSON.stringify(BASE));
});

test("orphans hides the nodes with no link, and un-hides them", async () => {
  const desk = studio();
  await run(desk, "filter.orphans", { hide: "on" });
  assert.equal(desk.settings().filter.orphans, true);
  await run(desk, "filter.orphans", { hide: "off" });
  assert.equal(desk.settings().filter.orphans, false);
});

test("a kind toggle adds and removes one kind, in NODE_KINDS order", async () => {
  const desk = studio();
  await run(desk, "filter.kind", { name: "tag", hide: "on" });
  assert.deepEqual(desk.settings().filter.hiddenKinds, ["tag"]);
  await run(desk, "filter.kind", { name: "note", hide: "on" });
  assert.deepEqual(desk.settings().filter.hiddenKinds, ["note", "tag"]);
  await run(desk, "filter.kind", { name: "record", hide: "on" });
  assert.deepEqual(desk.settings().filter.hiddenKinds, ["record", "note", "tag"]);
  await run(desk, "filter.kind", { name: "note", hide: "off" });
  assert.deepEqual(desk.settings().filter.hiddenKinds, ["record", "tag"]);
  assert.deepEqual([...NODE_KINDS], ["record", "note", "database", "tag"]);
});

test("only the linked nodes are kept, and then all of them", async () => {
  const desk = studio();
  await run(desk, "filter.existing", { hide: "on" });
  assert.equal(desk.settings().filter.existingOnly, true);
  await run(desk, "filter.existing", { hide: "off" });
  assert.equal(desk.settings().filter.existingOnly, false);
});

test("relayout is asked for on and off", async () => {
  const desk = studio();
  await run(desk, "filter.relayout", { on: "on" });
  assert.equal(desk.settings().filter.relayout, true);
  await run(desk, "filter.relayout", { on: "off" });
  assert.equal(desk.settings().filter.relayout, false);
});

test("the search box sets the text, and the filter action reads the same member", async () => {
  const desk = studio();
  await run(desk, "filter.search", { text: "memory" });
  assert.equal(desk.settings().filter.text, "memory");
  await run(desk, "filter.text", { text: "graph" });
  assert.equal(desk.settings().filter.text, "graph");
});

test("a group is added, in order, and its colour is the first the palette has left", async () => {
  const desk = studio();
  await run(desk, "groups.add", { name: "Gamma", query: "kind:tag" });
  assert.deepEqual(desk.settings().groups, [
    ...GROUPS, { name: "Gamma", query: "kind:tag", colour: GROUP_PALETTE[2] ?? "#000000" },
  ]);
});

test("a group is added with the colour it was given", async () => {
  const desk = studio();
  await run(desk, "groups.add", { name: "Gamma", query: "kind:tag", colour: "#123abc" });
  assert.equal(desk.settings().groups[2]?.colour, "#123abc");
});

test("a group that already exists is replaced where it stands", async () => {
  const desk = studio();
  await run(desk, "groups.add", { name: "Alpha", query: "kind:note", colour: "#000000" });
  assert.deepEqual(desk.settings().groups, [
    { name: "Alpha", query: "kind:note", colour: "#000000" }, at(GROUPS, 1),
  ]);
});

test("a group with a query the grammar refuses is not added", async () => {
  const desk = studio();
  await refusal(desk, "groups.add", { name: "Gamma", query: "kind:(tag" });
  assert.equal(JSON.stringify(desk.settings()), JSON.stringify(BASE));
});

test("a group with a colour that is not a colour is not added", async () => {
  const desk = studio();
  const refused = await refusal(desk, "groups.add", { name: "Gamma", query: "kind:tag", colour: "burnt sienna" });
  assert.match(refused.message, /colour/);
  assert.equal(JSON.stringify(desk.settings()), JSON.stringify(BASE));
});

test("a group is dropped, and the rest keep their order", async () => {
  const desk = studio();
  await run(desk, "groups.remove", { name: "Alpha" });
  assert.deepEqual(desk.settings().groups, [at(GROUPS, 1)]);
});

test("a group moves to the position it was given, and the rest follow", async () => {
  const desk = studio(withGroups(withFilter(DEFAULT_SETTINGS, { text: "graph" }), [
    ...GROUPS, { name: "Gamma", query: "kind:tag", colour: GROUP_PALETTE[2] ?? "#000000" },
  ]));
  await run(desk, "groups.reorder", { name: "Gamma", to: "0" });
  assert.deepEqual(desk.settings().groups.map((group) => group.name), ["Gamma", "Alpha", "Beta"]);
  await run(desk, "groups.reorder", { name: "Gamma", to: "2" });
  assert.deepEqual(desk.settings().groups.map((group) => group.name), ["Alpha", "Beta", "Gamma"]);
  assert.equal(desk.settings().groups[2]?.query, "kind:tag");
});

test("a position the list does not have is refused, and the group's own place is named", async () => {
  const desk = studio();
  const refused = await refusal(desk, "groups.reorder", { name: "Beta", to: "9" });
  assert.match(refused.message, /Beta/);
  assert.match(refused.message, /1/);
  assert.deepEqual(desk.settings().groups, GROUPS);
});

test("one group is recoloured, and the others are not", async () => {
  const desk = studio();
  await run(desk, "groups.recolour", { name: "Beta", colour: "rgb(12 34 56)" });
  assert.deepEqual(desk.settings().groups[1], { name: "Beta", query: "kind:note", colour: "rgb(12 34 56)" });
  assert.deepEqual(at(desk.settings().groups, 0), at(GROUPS, 0));
});

test("a colour that is not a colour is refused", async () => {
  const desk = studio();
  await refusal(desk, "groups.recolour", { name: "Beta", colour: "#12" });
  assert.deepEqual(desk.settings().groups, GROUPS);
});

test("a node that fits two groups is coloured by the one that comes first", async () => {
  const desk = studio();
  const wide = "kind:database OR degree:>=1";
  await run(desk, "groups.add", { name: "Wide", query: wide });
  assert.deepEqual(desk.settings().groups.map((group) => group.name), ["Alpha", "Beta", "Wide"]);
  assert.deepEqual(desk.settings().groups.map((group) => group.query), ["kind:database", "kind:note", wide]);
  assert.deepEqual(desk.settings().groups.map((group) => group.colour), [GROUP_PALETTE[0], GROUP_PALETTE[1], GROUP_PALETTE[2]]);
});

test("the view is fitted to what the search found", async () => {
  const desk = studio();
  const message = await run(desk, "search.fit");
  assert.equal(message, "fit to 1 results");
  assert.deepEqual(desk.settings().groups, GROUPS);
});

test("the view is fitted to nothing when nothing matches", async () => {
  const desk = studio(withFilter(withGroups(DEFAULT_SETTINGS, GROUPS), { text: "zzz" }));
  assert.equal(await run(desk, "search.fit"), "fit to 0 results");
});

test("fitting the view is refused when nothing is drawn", () => {
  const empty = studio();
  const state = { ...empty.context.state(), meta: null };
  const fit = REGISTRY.find("search.fit");
  assert.ok(fit !== undefined);
  assert.notEqual(fit.available?.(state) ?? null, null);
});

test("the count in the message is the number of nodes the filter hides", async () => {
  const quiet = (): Studio => studio(withFilter(withGroups(DEFAULT_SETTINGS, GROUPS), { text: "" }));
  assert.equal(await run(quiet(), "filter.text", { text: "zzz" }), "4 of 4 nodes hidden");
  assert.equal(await run(quiet(), "filter.text", { text: "graph" }), "3 of 4 nodes hidden");
  assert.equal(await run(quiet(), "filter.orphans", { hide: "on" }), "1 of 4 nodes hidden");
  assert.equal(await run(quiet(), "filter.kind", { name: "note", hide: "on" }), "1 of 4 nodes hidden");
  assert.equal(await run(quiet(), "filter.query", { query: "kind:tag" }), "3 of 4 nodes hidden");
});
