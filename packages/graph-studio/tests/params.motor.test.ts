// The schema and the values, through the real motor: what `gm_layout_params` publishes, what a
// run at a value of the caller's choosing draws, and what a value out of range is answered with.
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Session } from "../src/motor/session.ts";
import { OPENING_SOURCE } from "../src/state/settings.ts";
import { FIXTURES_URL, SKIP, realSession } from "./motor.ts";

const GRAPHOPT = "layout.force.graphopt";
const SUGIYAMA = "layout.dag.sugiyama";
const BARNES_HUT = "layout.force.barnes_hut";

async function opened(): Promise<{ readonly session: Session }> {
  const session = realSession();
  await session.open("unused");
  await session.load(OPENING_SOURCE, FIXTURES_URL);
  return { session };
}

test("the motor publishes what it publishes, in one order", { skip: SKIP }, async () => {
  const { session } = await opened();
  const specs = session.params(GRAPHOPT);
  assert.ok(specs.length > 0, GRAPHOPT);
  assert.deepEqual(specs.map((spec) => spec.name), ["niter", "node_charge", "node_mass", "spring_length", "spring_constant", "max_sa_movement", "seed"]);
  for (const spec of specs) {
    assert.ok(spec.min <= spec.default && spec.default <= spec.max, `${spec.name}: ${spec.default} in ${spec.min}..${spec.max}`);
    assert.ok(spec.step > 0, `${spec.name}: a step is positive`);
    assert.notEqual(spec.doc, "", `${spec.name} says what it is`);
  }
});

test("a layout that publishes nothing answers with an empty list, not a refusal", { skip: SKIP }, async () => {
  const { session } = await opened();
  assert.deepEqual(session.params(BARNES_HUT), []);
});

test("a run at a value of the caller's choosing draws something else", { skip: SKIP }, async () => {
  const { session } = await opened();
  const plain = await session.layout(SUGIYAMA, null);
  const spec = session.params(SUGIYAMA)[0];
  assert.ok(spec !== undefined, "the layered layout publishes something");
  const moved = await session.layout(SUGIYAMA, null, { [spec.name]: spec.max });
  assert.equal(moved.params[spec.name], spec.max, "the report says what it ran at");
  assert.notEqual(Buffer.from(moved.bytes).toString("hex"), Buffer.from(plain.bytes).toString("hex"));
});

test("a run at the published default is the run with no values at all", { skip: SKIP }, async () => {
  const { session } = await opened();
  const spec = session.params(GRAPHOPT)[0];
  assert.ok(spec !== undefined);
  const plain = await session.layout(GRAPHOPT, null);
  const same = await session.layout(GRAPHOPT, null, { [spec.name]: spec.default });
  assert.deepEqual(same.digest, plain.digest);
});

test("a value out of range is refused by the motor, never clamped", { skip: SKIP }, async () => {
  const { session } = await opened();
  const spec = session.params(GRAPHOPT)[0];
  assert.ok(spec !== undefined);
  await assert.rejects(
    () => session.layout(GRAPHOPT, null, { [spec.name]: spec.max + 1 }),
    /ParamOutOfRange|range/i,
  );
});

test("a name the schema does not publish is refused by the SDK, before the motor", { skip: SKIP }, async () => {
  const { session } = await opened();
  await assert.rejects(() => session.layout(GRAPHOPT, null, { not_a_parameter: 1 }), /not a parameter this layout publishes/);
});
