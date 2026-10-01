// The force knobs as registry actions: mapped to the motor's units, and refused when wrong or off.
import assert from "node:assert/strict";
import { test } from "node:test";

import { type ForceLink, NO_FORCE_LINK, forceActions } from "../src/actions/forces.ts";
import { ActionRefusal, createRegistry } from "../src/actions/registry.ts";
import { DEFAULT_KNOBS, type ForceKnobs, NO_ADAPTER_REASON } from "../src/motor/live.ts";
import { type StudioState, initialState } from "../src/state/model.ts";

function live(): { readonly link: ForceLink; readonly sets: ForceKnobs[]; readonly animated: boolean[] } {
  const sets: ForceKnobs[] = [];
  const animated: boolean[] = [];
  let knobs = DEFAULT_KNOBS;
  let paused = false;
  const link: ForceLink = {
    disabled: () => null,
    knobs: () => knobs,
    set: (next) => { knobs = next; sets.push(next); },
    animate: (on) => { animated.push(on); },
    animating: () => animated.at(-1) ?? false,
    pause: () => { paused = true; },
    resume: () => { paused = false; },
    paused: () => paused,
  };
  return { link, sets, animated };
}

function registryOf(link: ForceLink) {
  return createRegistry<StudioState, null>(forceActions<null>(link));
}

const STATE = initialState();
const context = null;

test("the four knobs map to the motor's units; repel is shown positive and sent negative", async () => {
  const { link, sets } = live();
  const registry = registryOf(link);
  for (const [alias, value] of [["center", 0.4], ["repel", 250], ["linkforce", 1.5], ["linkdistance", 120]] as const) {
    const { action, args } = registry.resolve(alias, { value }, STATE);
    await action.run(context, args);
  }
  assert.deepEqual(sets.at(-1), { gravity: 0.4, charge: -250, linkStrengthScale: 1.5, linkDistance: 120 });
});

test("a slider shows the current knob, repel as a magnitude", () => {
  const { link } = live();
  const shown = Object.fromEntries(forceActions<null>(link).map((a) => [a.alias, a.params[0]?.value(STATE)]));
  assert.equal(shown["repel"], 300);
  assert.equal(shown["center"], 0.1);
  assert.equal(shown["linkdistance"], 30);
});

test("out-of-range and NaN values are refused, and nothing reaches the motor", () => {
  const { link, sets } = live();
  const registry = registryOf(link);
  const cases: readonly [string, unknown][] = [
    ["center", 1.01], ["center", -0.1], ["repel", 1001], ["repel", -1], ["linkforce", 2.1], ["linkdistance", 9], ["linkdistance", 501],
    ["center", Number.NaN], ["repel", "abc"], ["linkdistance", ""], ["center", Infinity],
  ];
  for (const [alias, value] of cases) {
    assert.throws(() => registry.resolve(alias, { value }, STATE), (error: unknown) => error instanceof ActionRefusal && error.code === "bad-value", `${alias} ${String(value)}`);
  }
  assert.equal(sets.length, 0);
});

test("the edges of every range are accepted", () => {
  const registry = registryOf(live().link);
  for (const [alias, value] of [["center", 0], ["center", 1], ["repel", 0], ["repel", 1000], ["linkforce", 2], ["linkdistance", 10], ["linkdistance", 500]] as const) {
    assert.doesNotThrow(() => registry.resolve(alias, { value }, STATE), `${alias} ${value}`);
  }
});

test("reset restores the defaults; animate reaches the loop", async () => {
  const { link, sets, animated } = live();
  const registry = registryOf(link);
  await registry.resolve("center", { value: 0.9 }, STATE).action.run(context, { value: 0.9 });
  const reset = registry.resolve("forcesreset", {}, STATE);
  await reset.action.run(context, reset.args);
  assert.deepEqual(sets.at(-1), DEFAULT_KNOBS);
  const animate = registry.resolve("forcesanimate", { on: true }, STATE);
  await animate.action.run(context, animate.args);
  assert.deepEqual(animated, [true]);
});

test("with no adapter every force action is unavailable, with the reason", () => {
  const registry = registryOf(NO_FORCE_LINK);
  for (const action of forceActions<null>(NO_FORCE_LINK)) {
    assert.equal(action.available?.(STATE), NO_ADAPTER_REASON, action.id);
  }
  assert.throws(() => registry.resolve("center", { value: 0.5 }, STATE), (error: unknown) => error instanceof ActionRefusal && error.code === "unavailable");
});
