// What the studio does with the motor's own schema: every name and bound comes from the motor,
// a value is checked once against it, a change costs exactly one run, the values are kept per
// layout id, and a change stops whatever run is in flight.
import assert from "node:assert/strict";
import { test } from "node:test";

import { studioActions } from "../src/actions/all.ts";
import { type Checked, RESET_ID, SET_ID, SET_MANY_ID, checkedValue } from "../src/actions/params.ts";
import { complete } from "../src/console/complete.ts";
import type { GraphSummary, LayoutParamSpec } from "../src/motor/protocol.ts";
import { type RunSummary, type StudioState } from "../src/state/model.ts";
import { DEFAULT_SETTINGS, type Settings, withParams, withSettings, withoutParams } from "../src/state/settings.ts";
import { type Desk, desk, scriptedClient } from "./desk.ts";

const LAYOUT = "layout.force.graphopt";
const OTHER = "layout.grid";

/** Why a value was refused. A test that expects a refusal must see one. */
function refused(checked: Checked): string {
  if (checked.ok) throw new Error(`the studio accepted ${String(checked.value)} where the schema refuses it`);
  return checked.reason;
}

function spec(over: Partial<LayoutParamSpec> = {}): LayoutParamSpec {
  return {
    name: "gravity", kind: "float", min: 0, max: 10, default: 1, step: 0.1, doc: "how hard the centre pulls",
    ...over,
  };
}

const NITER = spec({ name: "niter", kind: "int", min: 1, max: 1000, default: 50, step: 1 });
const SPACING = spec({ name: "spacing", kind: "int", min: 1, max: 100, default: 10, step: 1 });
const SCHEMA: Readonly<Record<string, readonly LayoutParamSpec[]>> = {
  [LAYOUT]: [spec(), NITER],
  [OTHER]: [SPACING],
};

const GRAPH: GraphSummary = { name: "scripted", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 };

function summary(over: Partial<RunSummary> = {}): RunSummary {
  return {
    layoutId: LAYOUT, postId: null, postError: null, digest: null, byteLength: 64,
    nodeKind: "point", edgeKind: "segment", dim: 0, layoutMs: 1, postMs: 0, notes: [], ...over,
  };
}

/** A desk with a graph loaded and a schema the motor published; nothing else is standing. */
function drawn(settings: Settings = withSettings(DEFAULT_SETTINGS, { layout: LAYOUT }), schemas = SCHEMA): Desk {
  const made = desk(scriptedClient(), settings);
  made.studio.store.set({ ...made.studio.store.get(), graph: GRAPH, schemas });
  return made;
}

function stateOf(made: Desk): StudioState {
  return made.studio.store.get();
}

test("a float, an int and a bool are each what the schema says they are", () => {
  assert.deepEqual(checkedValue(spec(), "2.5"), { ok: true, value: 2.5 });
  assert.deepEqual(checkedValue(NITER, "50"), { ok: true, value: 50 });
  assert.deepEqual(checkedValue(spec({ kind: "bool", min: 0, max: 1, default: 0, step: 1 }), "off"), { ok: true, value: false });
});

test("a value outside the schema's bounds is refused by name, never clamped", () => {
  const outside = refused(checkedValue(spec(), "11"));
  assert.match(outside, /`gravity` must be in 0\.\.10, not 11/);
});

test("an int is refused a value that is not whole, and a bool anything but on or off", () => {
  assert.match(refused(checkedValue(NITER, "2.5")), /`niter` must be a whole number/);
  const maybe = refused(checkedValue(spec({ kind: "bool", min: 0, max: 1, default: 0, step: 1 }), "maybe"));
  assert.match(maybe, /`gravity` must be on or off/);
});

test("the panel's names and bounds are the schema's, and nothing else", () => {
  const made = drawn();
  const one = made.studio.registry.find(SET_ID);
  assert.ok(one !== undefined, "the one-value action is in the registry");
  const names = one.params[0];
  assert.ok(names !== undefined, "and it names a parameter");
  const choose = names.choices;
  assert.ok(choose !== undefined, "and the names are the schema's to choose from");
  assert.deepEqual(choose(stateOf(made)), ["gravity", "niter"]);
  const quiet = drawn(withSettings(DEFAULT_SETTINGS, { layout: "layout.force.barnes_hut" }));
  assert.deepEqual(choose(stateOf(quiet)), [], "and nothing where the motor publishes nothing");
});

test("a name the schema does not publish is refused, never silently dropped", () => {
  const made = drawn();
  assert.throws(
    () => made.studio.registry.resolve(SET_ID, { param: "not_a_parameter", value: "1" }, stateOf(made)),
    /`param` is not one of: gravity, niter/,
  );
  // The names are the schema's, so a name another layout publishes is not a name of this one.
  assert.throws(
    () => made.studio.registry.resolve(SET_ID, { param: "spacing", value: "1" }, stateOf(made)),
    /`param` is not one of: gravity, niter/,
  );
});

test("`layoutset` refuses a value out of range before anything runs", async () => {
  const made = drawn();
  const before = stateOf(made).layoutCalls;
  const logged = await made.studio.run("layoutset gravity 99");
  assert.equal(logged.ok, false);
  assert.match(logged.error?.detail ?? "", /must be in 0\.\.10/);
  assert.equal(stateOf(made).layoutCalls, before, "a refused value costs no run");
  assert.deepEqual(stateOf(made).settings.params, {});
});

test("the console offers the names the schema published, and nothing else", () => {
  const made = drawn();
  assert.deepEqual(complete("layoutset ", made.studio.registry, stateOf(made)).candidates, ["gravity", "niter"]);
  assert.deepEqual(complete("layoutset n", made.studio.registry, stateOf(made)).candidates, ["niter"]);
});

test("`layoutset <param> <value>` writes the value and costs exactly one run", async () => {
  const made = drawn();
  const before = stateOf(made).layoutCalls;
  const logged = await made.studio.run("layoutset gravity 2.5");
  assert.equal(logged.ok, true, logged.message);
  assert.deepEqual(stateOf(made).settings.params[LAYOUT], { gravity: 2.5 });
  assert.equal(stateOf(made).layoutCalls, before + 1);
});

test("the motor is asked for the schema of the layout that ran, once per layout", async () => {
  const asked: string[] = [];
  const base = scriptedClient();
  const made = desk(
    { ...base, params: (layoutId) => { asked.push(layoutId); return base.params(layoutId); } },
    DEFAULT_SETTINGS,
  );
  await made.studio.start();
  await made.studio.run("layout layout.grid");
  await made.studio.run("layout layout.grid");
  assert.deepEqual(asked, [DEFAULT_SETTINGS.layout, OTHER], "asked once for each layout, never again");
});

test("the schema the studio holds is the one the worker answered with", async () => {
  const base = scriptedClient();
  const made = desk({
    ...base,
    catalog: () => Promise.resolve({ layouts: [DEFAULT_SETTINGS.layout, LAYOUT], posts: [], analyses: [] }),
    params: (id) => Promise.resolve(id === LAYOUT ? [NITER] : []),
  }, DEFAULT_SETTINGS);
  await made.studio.start();
  assert.deepEqual(stateOf(made).schemas[DEFAULT_SETTINGS.layout], [], "a layout that publishes none is an answer");
  await made.studio.run(`layout ${LAYOUT}`);
  assert.deepEqual(stateOf(made).schemas[LAYOUT], [NITER]);
});

test("a refused schema is a note on the run, not a failed drawing", async () => {
  const base = scriptedClient();
  const made = desk({ ...base, params: () => Promise.reject(new Error("the motor is not open")) }, DEFAULT_SETTINGS);
  const logged = await made.studio.start();
  assert.equal(logged.ok, true, logged.message);
  assert.equal(stateOf(made).schemas[DEFAULT_SETTINGS.layout], undefined, "nothing was cached");
  assert.match(logged.notes.join(" "), /the parameters of .* are unknown: the motor is not open/);
});

test("the values are kept per layout id, and a reset touches one layout only", () => {
  const settings = withParams(DEFAULT_SETTINGS, LAYOUT, { gravity: 2 });
  const both = withParams(settings, OTHER, { spacing: 40 });
  assert.deepEqual(both.params, { [LAYOUT]: { gravity: 2 }, [OTHER]: { spacing: 40 } });
  assert.deepEqual(withoutParams(both, OTHER).params, { [LAYOUT]: { gravity: 2 } });
});

test("`layoutreset` puts the current layout back to the motor's defaults", async () => {
  const made = drawn(withParams(withSettings(DEFAULT_SETTINGS, { layout: LAYOUT }), LAYOUT, { gravity: 2 }));
  const logged = await made.studio.run("layoutreset");
  assert.equal(logged.ok, true, logged.message);
  assert.deepEqual(stateOf(made).settings.params, {});
  assert.equal(stateOf(made).layoutCalls, 1, "the run at the defaults drew something, and it is not the old one");
});

test("the reset refuses when the layout is already at the defaults, and says why", () => {
  const made = drawn();
  const reason = made.studio.registry.find(RESET_ID)?.available?.(stateOf(made));
  assert.match(reason ?? "", /already at the motor's defaults/);
});

test("with nothing published every panel action refuses, in place, with the reason", () => {
  const quiet = "layout.force.barnes_hut";
  const made = drawn(withSettings(DEFAULT_SETTINGS, { layout: quiet }), { [quiet]: [] });
  const state = stateOf(made);
  for (const id of [SET_ID, SET_MANY_ID, RESET_ID]) {
    assert.match(made.studio.registry.find(id)?.available?.(state) ?? "", /publishes no parameters/, id);
  }
});

test("the words the brief names are in the registry, once each", () => {
  const aliases = studioActions().map((action) => action.alias);
  for (const alias of ["layoutset", "layoutreset"]) assert.ok(aliases.includes(alias), alias);
  assert.equal(new Set(aliases).size, aliases.length, "no alias is shared");
});

test("a change of value stops the run in flight, on the one cancel path", async () => {
  let cancels = 0;
  const busy = {
    ...scriptedClient(),
    busy: () => true,
    cancel: () => { cancels += 1; return true; },
  };
  const running = desk(busy, withSettings(DEFAULT_SETTINGS, { layout: LAYOUT }));
  running.studio.store.set({ ...stateOf(running), graph: GRAPH, schemas: SCHEMA, run: summary(), runParams: "{}" });
  const logged = await running.studio.run("layoutset gravity 3");
  assert.equal(logged.ok, true, logged.message);
  assert.equal(cancels, 1, "the run that was in flight was stopped");
});

test("a run at the defaults is the run a run with no values is", async () => {
  const made = drawn();
  await made.studio.run(`layoutset gravity ${spec().default}`);
  assert.deepEqual(stateOf(made).settings.params[LAYOUT], { gravity: 1 });
  assert.equal(stateOf(made).runParams, '{"gravity":1}');
});

test("the values the motor was run at are what the next plan is compared against", async () => {
  const made = drawn();
  await made.studio.run("layoutset gravity 4");
  const after = stateOf(made);
  assert.equal(after.runParams, JSON.stringify({ gravity: 4 }));
  assert.equal(after.run?.layoutId, LAYOUT);
});
