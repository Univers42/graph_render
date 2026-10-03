// The force knobs as registry actions: mapped to the motor's units, and refused when wrong or off.
import assert from "node:assert/strict";
import { test } from "node:test";

import { type ForceLink, NO_FORCE_LINK, forceActions } from "../src/actions/forces.ts";
import { ActionRefusal, createRegistry } from "../src/actions/registry.ts";
import { DEFAULT_KNOBS, type ForceKnobs, NO_ADAPTER_REASON } from "../src/motor/live.ts";
import { type StudioState, initialState } from "../src/state/model.ts";

/** A link over a plain variable; `drawn` is the radius the presets read, `null` for nothing drawn. */
function live(drawn: number | null = 4): { readonly link: ForceLink; readonly sets: ForceKnobs[]; readonly animated: boolean[] } {
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
    drawn: () => drawn,
  };
  return { link, sets, animated };
}

function registryOf(link: ForceLink) {
  return createRegistry<StudioState, null>(forceActions<null>(link));
}

const STATE = initialState();
const context = null;

const NINE: readonly (readonly [string, number])[] = [
  ["center", 0.4], ["repel", 250], ["linkforce", 1.5], ["linkdistance", 120], ["spacing", 7.5],
  ["friction", 0.3], ["cooling", 0.02], ["repelrange", 900], ["accuracy", 1.2],
];

test("the nine knobs map to the motor's units; repel is sent negative, friction as 1 - decay", async () => {
  const { link, sets } = live();
  const registry = registryOf(link);
  for (const [alias, value] of NINE) {
    const { action, args } = registry.resolve(alias, { value }, STATE);
    await action.run(context, args);
  }
  assert.deepEqual(sets.at(-1), {
    gravity: 0.4, charge: -250, linkStrengthScale: 1.5, linkDistance: 120, collideRadius: 7.5,
    velocityDecay: 1 - 0.3, alphaDecay: 0.02, distanceMax: 900, theta: 1.2,
  });
});

test("a slider shows the current knob: repel as a magnitude, friction as 1 - decay, at the motor's defaults", () => {
  const { link } = live();
  const shown = Object.fromEntries(forceActions<null>(link).map((a) => [a.alias, a.params[0]?.value(STATE)]));
  assert.deepEqual(
    [shown["center"], shown["repel"], shown["linkforce"], shown["linkdistance"], shown["spacing"]],
    [0, 90, 0.15, 60, 16],
  );
  assert.deepEqual([shown["friction"], shown["cooling"], shown["repelrange"], shown["accuracy"]], [0.42, 0.06, 520, 0.9]);
});

test("every slider's range is the panel's limit, inside the motor's own", () => {
  const ranges = Object.fromEntries(forceActions<null>(live().link).flatMap((a) => {
    const param = a.params[0];
    return param?.kind === "number" ? [[a.alias, [param.min, param.max]]] : [];
  }));
  assert.deepEqual(ranges, {
    center: [0, 1], repel: [0, 1000], linkforce: [0, 2], linkdistance: [10, 500], spacing: [0, 400],
    friction: [0.01, 0.99], cooling: [0.005, 0.5], repelrange: [10, 5000], accuracy: [0.3, 1.5],
  });
});

test("out-of-range and NaN values are refused, and nothing reaches the motor", () => {
  const { link, sets } = live();
  const registry = registryOf(link);
  const cases: readonly [string, unknown][] = [
    ["center", 1.01], ["center", -0.1], ["repel", 1001], ["repel", -1], ["linkforce", 2.1], ["linkdistance", 9], ["linkdistance", 501],
    ["spacing", -0.5], ["spacing", 400.5], ["friction", 0], ["friction", 1], ["cooling", 0], ["cooling", 0.51],
    ["repelrange", 9], ["repelrange", 5001], ["accuracy", 0.29], ["accuracy", 1.51],
    ["center", Number.NaN], ["repel", "abc"], ["linkdistance", ""], ["center", Infinity],
  ];
  for (const [alias, value] of cases) {
    assert.throws(() => registry.resolve(alias, { value }, STATE), (error: unknown) => error instanceof ActionRefusal && error.code === "bad-value", `${alias} ${String(value)}`);
  }
  assert.equal(sets.length, 0);
});

test("the edges of every range are accepted", () => {
  const registry = registryOf(live().link);
  const edges: readonly (readonly [string, number])[] = [
    ["center", 0], ["center", 1], ["repel", 0], ["repel", 1000], ["linkforce", 2], ["linkdistance", 10], ["linkdistance", 500],
    ["spacing", 0], ["spacing", 400], ["friction", 0.01], ["friction", 0.99], ["cooling", 0.005], ["cooling", 0.5],
    ["repelrange", 10], ["repelrange", 5000], ["accuracy", 0.3], ["accuracy", 1.5],
  ];
  for (const [alias, value] of edges) {
    assert.doesNotThrow(() => registry.resolve(alias, { value }, STATE), `${alias} ${value}`);
  }
});

test("reset restores all nine defaults; animate reaches the loop", async () => {
  const { link, sets, animated } = live();
  const registry = registryOf(link);
  for (const [alias, value] of NINE) await registry.resolve(alias, { value }, STATE).action.run(context, { value });
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


async function pressed(link: ForceLink, alias: string): Promise<string> {
  const { action, args } = registryOf(link).resolve(alias, {}, STATE);
  return (await action.run(context, args)).message;
}

test("spread spaces nodes at the drawn radius and a half, on a half-unit grid, and repels three times harder", async () => {
  const { link, sets } = live(4.1);
  const message = await pressed(link, "spread");
  assert.deepEqual(sets.at(-1), { ...DEFAULT_KNOBS, collideRadius: 6.5, charge: -270 });
  assert.match(message, /node spacing 6\.5, repel 270/);
});

test("spread keeps a repel already stronger than its own, and the other knobs as they were", async () => {
  const { link, sets } = live(2);
  const registry = registryOf(link);
  for (const [alias, value] of [["repel", 800], ["linkdistance", 200]] as const) {
    await registry.resolve(alias, { value }, STATE).action.run(context, { value });
  }
  await pressed(link, "spread");
  assert.deepEqual(sets.at(-1), { ...DEFAULT_KNOBS, charge: -800, linkDistance: 200, collideRadius: 3 });
});

test("compact pulls in to the drawn radius with no margin, and spacing never passes its limit", async () => {
  const small = live(4.1);
  await pressed(small.link, "compact");
  assert.deepEqual(small.sets.at(-1), { ...DEFAULT_KNOBS, charge: -30, linkDistance: 30, gravity: 0.05, collideRadius: 4.5 });
  const huge = live(1000);
  await pressed(huge.link, "spread");
  assert.equal(huge.sets.at(-1)?.collideRadius, 400);
});

test("a preset with nothing drawn, or a link that cannot see the drawing, refuses and sets nothing", async () => {
  for (const link of [live(null).link, live(Number.NaN).link, { ...NO_FORCE_LINK, disabled: () => null }]) {
    for (const alias of ["spread", "compact"]) {
      await assert.rejects(
        async () => pressed(link, alias),
        (error: unknown) => error instanceof ActionRefusal && error.code === "unavailable",
        alias,
      );
    }
  }
});
