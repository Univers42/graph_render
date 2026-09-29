// What the studio does when the motor refuses, is stopped, or is asked twice at once:
// a scripted client in place of the worker, real snapshot bytes where a drawing is needed.
import assert from "node:assert/strict";
import { test } from "node:test";

import { CancelledError, type MotorClient, MotorFailure } from "../src/motor/client.ts";
import type { RunReport } from "../src/motor/protocol.ts";
import { OPENING_SOURCE } from "../src/state/settings.ts";
import { desk } from "./desk.ts";
import { SKIP, realRun } from "./motor.ts";

const CATALOG = { layouts: ["layout.forceatlas2", "layout.grid"], posts: [], analyses: ["analysis.depth.bfs"] };
const GRAPH = { name: "vault seed 1", nodeCount: 400, edgeCount: 798, notes: [], buildMs: 1 };
const REFUSED = { title: "RunRefusedError", code: "code 8 (LayoutFailed)", detail: "refused", hint: "" };

interface Script extends MotorClient {
  readonly asked: string[];
  /** Layout calls wait here until the test lets them go. */
  readonly waiting: { readonly resolve: (run: RunReport) => void; readonly reject: (error: Error) => void }[];
}

function scripted(): Script {
  const script: Script = {
    asked: [],
    waiting: [],
    catalog: () => Promise.resolve(CATALOG),
    load: (source) => {
      script.asked.push(`load ${source.kind}`);
      return Promise.resolve(GRAPH);
    },
    layout: (layoutId) => {
      script.asked.push(`layout ${layoutId}`);
      return new Promise<RunReport>((resolve, reject) => void script.waiting.push({ resolve, reject }));
    },
    analysis: () => Promise.reject(new MotorFailure({ ...REFUSED, title: "AnalysisRefusedError" })),
    cancel: () => {
      const stopped = script.waiting.splice(0);
      if (stopped.length > 0) script.asked.push("cancel");
      for (const waiting of stopped) waiting.reject(new CancelledError());
      return stopped.length > 0;
    },
    busy: () => script.waiting.length > 0,
    close: () => void script.asked.push("close"),
  };
  return script;
}

async function settled(): Promise<void> {
  for (let round = 0; round < 8; round += 1) await Promise.resolve();
}

test("a source whose layout is refused is loaded and shows nothing", async () => {
  const client = scripted();
  const { studio, seen } = desk(client);
  const starting = studio.start();
  await settled();
  client.waiting.shift()?.reject(new MotorFailure(REFUSED));
  const entry = await starting;
  const state = studio.store.get();
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "RunRefusedError", "code 8 (LayoutFailed)"]);
  assert.deepEqual([state.graph?.nodeCount, state.run, state.meta], [400, null, null]);
  assert.equal(seen.frames.at(-1)?.frame.nodeCount, 0);
  assert.equal(state.error?.title, "RunRefusedError");
  assert.deepEqual(state.busy, []);
});

test("while it runs the studio says what is running", async () => {
  const client = scripted();
  const { studio } = desk(client);
  const starting = studio.start();
  await settled();
  assert.deepEqual(studio.store.get().busy.map((running) => running.command), ["synthetic 400 2 1 vault"]);
  client.waiting.shift()?.reject(new MotorFailure(REFUSED));
  await starting;
});

test("a refused layout leaves the drawing and the settings as they were", { skip: SKIP }, async () => {
  const client = scripted();
  const { studio, seen } = desk(client);
  const starting = studio.start();
  await settled();
  client.waiting.shift()?.resolve(await realRun(OPENING_SOURCE, "layout.forceatlas2"));
  assert.equal((await starting).ok, true);
  const before = studio.store.get();
  const asking = studio.dispatch("layout", { id: "layout.grid" });
  await settled();
  client.waiting.shift()?.reject(new MotorFailure(REFUSED));
  assert.equal((await asking).ok, false);
  assert.deepEqual([studio.store.get().settings, studio.store.get().run], [before.settings, before.run]);
  assert.equal(seen.frames.length, 1);
});

test("the latest request wins: the run in flight is stopped, and stopping is not a failure", { skip: SKIP }, async () => {
  const client = scripted();
  const { studio } = desk(client);
  const starting = studio.start();
  await settled();
  client.waiting.shift()?.resolve(await realRun(OPENING_SOURCE, "layout.forceatlas2"));
  await starting;
  const first = studio.dispatch("layout", { id: "layout.grid" });
  await settled();
  const second = studio.dispatch("layout", { id: "layout.grid" });
  await settled();
  assert.deepEqual(client.asked.slice(-3), ["layout layout.grid", "cancel", "layout layout.grid"]);
  client.waiting.shift()?.resolve(await realRun(OPENING_SOURCE, "layout.grid"));
  assert.deepEqual([(await first).ok, (await first).error?.title], [false, "CancelledError"]);
  assert.equal((await second).ok, true);
  assert.deepEqual([studio.store.get().settings.layout, studio.store.get().error], ["layout.grid", null]);
});

test("cancel stops the run and reports whether there was one", async () => {
  const client = scripted();
  const { studio } = desk(client);
  assert.match((await studio.dispatch("cancel")).message, /nothing was running/);
  const starting = studio.start();
  await settled();
  assert.match((await studio.dispatch("cancel")).message, /stopped/);
  assert.equal((await starting).error?.title, "CancelledError");
});

test("an analysis that is refused is switched off, and the graph stays drawn", { skip: SKIP }, async () => {
  const client = scripted();
  const { studio, seen } = desk(client);
  const starting = studio.start();
  await settled();
  client.waiting.shift()?.resolve(await realRun(OPENING_SOURCE, "layout.forceatlas2"));
  await starting;
  const entry = await studio.dispatch("analysis", { id: "bfs" });
  const state = studio.store.get();
  assert.deepEqual([entry.ok, entry.error?.title], [false, "AnalysisRefusedError"]);
  assert.deepEqual([state.analysis, state.settings.analysis, state.settings.appearance.colourBy], [null, null, "group"]);
  assert.equal(seen.frames.length, 1);
});

test("a motor that does not open is an error on screen, not a blank page", async () => {
  const client = { ...scripted(), catalog: () => Promise.reject(new MotorFailure({ ...REFUSED, title: "WasmUnavailableError" })) };
  const { studio } = desk(client);
  const entry = await studio.start();
  assert.deepEqual([entry.ok, entry.command, studio.store.get().error?.title], [false, "open", "WasmUnavailableError"]);
});

test("destroying the studio closes the motor", () => {
  const client = scripted();
  desk(client).studio.destroy();
  assert.deepEqual(client.asked, ["close"]);
});
