/**
 * `sameSource` decides whether a settings change is a new graph or a new look on the same one,
 * so a member it forgets to compare reloads the graph and a member it compares wrongly keeps the
 * old one on screen. Both are quiet: nothing is refused, the drawing is simply not what was
 * asked for. These rows are one per member of each of the three kinds.
 *
 * The studio's own documents are frozen and a change is a new object
 * (`state/settings.ts`), so `a === b` is the fast path and almost never the whole answer. Every
 * case here is a distinct object with equal or unequal members, which is the case that matters.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { MotorClient } from "../src/motor/client.ts";
import { type Source, withSettings } from "../src/state/settings.ts";
import { desk, scriptedClient } from "./desk.ts";

/** The scripted motor with its load counted, which is what a new source is asked for. */
function counting(): { readonly client: MotorClient; readonly loads: () => number } {
  let loads = 0;
  const scripted = scriptedClient();
  const client: MotorClient = { ...scripted, load: (source) => { loads += 1; return scripted.load(source); } };
  return { client, loads: () => loads };
}

const TEXT = '{"nodes":[],"edges":[]}';
const SYNTHETIC: Source = { kind: "synthetic", seed: 1, nodes: 400, degree: 2, shape: "vault" };
const FIXTURE: Source = { kind: "fixture", path: "fixtures/small.json" };
const DOCUMENT: Source = { kind: "document", name: "g.json", text: TEXT };

/** A fresh copy of `source`, member for member, so it is equal without being the same object. */
function copyOf(source: Source): Source {
  if (source.kind === "fixture") return { kind: "fixture", path: source.path };
  if (source.kind === "document") return { kind: "document", name: source.name, text: source.text };
  return { kind: "synthetic", seed: source.seed, nodes: source.nodes, degree: source.degree, shape: source.shape };
}

/**
 * How many times the motor was asked to open a graph while the settings went from `a` to `b`.
 * The studio is first moved onto `a`, so the second apply is the comparison being tested and not
 * a comparison against the opening document.
 */
async function loadsWhenGoingFromTo(a: Source, b: Source): Promise<number> {
  const motor = counting();
  const made = desk(motor.client);
  const started = await made.studio.start();
  assert.equal(started.ok, true, started.message);
  await made.pipeline.apply(withSettings(made.studio.store.get().settings, { source: a }));
  const before = motor.loads();
  await made.pipeline.apply(withSettings(made.studio.store.get().settings, { source: b }));
  return motor.loads() - before;
}

/** Every member of a synthetic source, one at a time: a forgotten member reloads the graph. */
const SYNTHETIC_MOVES: readonly Source[] = [
  { kind: "synthetic", seed: 2, nodes: 400, degree: 2, shape: "vault" },
  { kind: "synthetic", seed: 1, nodes: 401, degree: 2, shape: "vault" },
  { kind: "synthetic", seed: 1, nodes: 400, degree: 3, shape: "vault" },
  { kind: "synthetic", seed: 1, nodes: 400, degree: 2, shape: "random" },
];

test("the same source, as a new object, is the same graph", async () => {
  for (const source of [SYNTHETIC, FIXTURE, DOCUMENT]) {
    assert.equal(await loadsWhenGoingFromTo(source, copyOf(source)), 0,
      `${source.kind}: an equal source reloads nothing`);
  }
});

test("one moved member of a synthetic source is a new graph", async () => {
  for (const moved of SYNTHETIC_MOVES) {
    assert.equal(await loadsWhenGoingFromTo(SYNTHETIC, moved), 1,
      `seed/nodes/degree/shape moved: ${JSON.stringify(moved)}`);
  }
});

test("the members of a fixture and of a document each decide", async () => {
  assert.equal(await loadsWhenGoingFromTo(FIXTURE, { kind: "fixture", path: "fixtures/other.json" }), 1, "a new path");
  assert.equal(await loadsWhenGoingFromTo(DOCUMENT, { kind: "document", name: "g.json", text: "{}" }), 1, "new text, same name");
  assert.equal(await loadsWhenGoingFromTo(DOCUMENT, { kind: "document", name: "h.json", text: TEXT }), 1, "new name, same text");
});

test("a different kind is a different graph, even with a member in common", async () => {
  assert.equal(await loadsWhenGoingFromTo(FIXTURE, { kind: "synthetic", seed: 1, nodes: 400, degree: 2, shape: "vault" }), 1);
  assert.equal(await loadsWhenGoingFromTo(DOCUMENT, { kind: "fixture", path: "fixtures/small.json" }), 1);
});
