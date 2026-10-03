// The id-to-row table the live port pins by: which row a drag reaches, and when it moves.
import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_KNOBS, type ForceParams, type ForcePort, type LiveForce } from "../src/motor/live.ts";
import { createLiveForce } from "../src/motor/liveSession.ts";

const PARAMS: ForceParams = {
  charge: 0, theta: 0, distance_min: 0, distance_max: 0, link_distance: 0, link_strength_scale: 0,
  collide_radius: 0, center_strength: 0, gravity: 0, velocity_decay: 0, alpha_decay: 0, alpha_min: 0, initial_alpha: 0,
};

function port(): { readonly force: ForcePort; readonly calls: string[] } {
  const calls: string[] = [];
  const made = {
    tick: (ticks: number) => { calls.push(`tick ${ticks}`); return { alpha: 0 }; },
    pin: (row: number, x: number, y: number) => { calls.push(`pin ${row} ${x} ${y}`); },
    unpin: (row: number) => { calls.push(`unpin ${row}`); },
    reheat: (alpha: number) => { calls.push(`reheat ${alpha}`); },
    setParams: (params: Partial<ForceParams>) => { calls.push(`params ${JSON.stringify(params)}`); },
    positions: () => ({ xs: Float64Array.of(0), ys: Float64Array.of(0) }),
    params: () => PARAMS,
    release: () => { calls.push("release"); },
    alpha: 0,
  };
  return { calls, force: made };
}

interface Rowed {
  order: readonly string[];
  readonly set: (order: readonly string[]) => void;
}

function box(initial: readonly string[]): Rowed {
  const box: Rowed = { order: initial, set: (order) => { box.order = order; } };
  return box;
}

/** A port whose `restart` answers a second fake session, so a restart is observable. */
function restarts(force: ForcePort, table: Rowed, calls: string[]): LiveForce {
  return createLiveForce({
    session: force,
    ids: () => table.order,
    restart: () => {
      calls.push("restart");
      return { ...force, alpha: 1 };
    },
  });
}

function live(force: ForcePort, table: Rowed) {
  return restarts(force, table, []);
}

test("a pin reaches the row its id sits in, and an id the order does not hold is dropped", () => {
  const made = port();
  const rowed = box(["alpha", "beta", "gamma"]);
  const force = live(made.force, rowed);
  force.pin("gamma", 3, 4);
  force.unpin("gamma");
  force.pin("delta", 5, 6);
  assert.deepEqual(made.calls, ["pin 2 3 4", "unpin 2"]);
});

test("a duplicated id keeps its first row, as the scan it replaced answered", () => {
  const made = port();
  const rowed = box(["alpha", "beta", "alpha"]);
  live(made.force, rowed).pin("alpha", 1, 1);
  assert.deepEqual(made.calls, ["pin 0 1 1"]);
});

test("a new order is picked up: a layout rebuilds the table", () => {
  const made = port();
  const rowed = box(["alpha", "beta"]);
  const force = live(made.force, rowed);
  force.pin("beta", 1, 1);
  rowed.set(["gamma", "alpha", "beta"]);
  force.pin("beta", 2, 2);
  assert.deepEqual(made.calls, ["pin 1 1 1", "pin 2 2 2"]);
});

test("before a layout has run there are no rows, so a pin is dropped rather than refused", () => {
  const made = port();
  const rowed: Rowed = { order: [], set: () => undefined };
  createLiveForce({ session: made.force, ids: () => null, restart: () => made.force }).pin("alpha", 1, 1);
  live(made.force, rowed).pin("alpha", 1, 1);
  assert.deepEqual(made.calls, []);
});

test("a shuffle restarts the session, re-applies the knobs and answers the new alpha", () => {
  const made = port();
  const rowed = box(["alpha", "beta"]);
  const force = restarts(made.force, rowed, made.calls);
  force.setParams({ ...DEFAULT_KNOBS, gravity: 0.4 });
  const alpha = force.shuffle?.();
  assert.equal(alpha, 1, "the alpha is the new session's own, not a reheated 1");
  // Built from DEFAULT_KNOBS, in the port's wire order, so a moved default does not break it.
  const { charge, linkStrengthScale, linkDistance, collideRadius, velocityDecay, alphaDecay, distanceMax, theta } = DEFAULT_KNOBS;
  const params = `params ${JSON.stringify({
    gravity: 0.4, charge, link_strength_scale: linkStrengthScale, link_distance: linkDistance,
    collide_radius: collideRadius, velocity_decay: velocityDecay, alpha_decay: alphaDecay, distance_max: distanceMax, theta,
  })}`;
  assert.deepEqual(made.calls, [params, "restart", params]);
  force.pin("beta", 2, 3);
  assert.equal(made.calls.at(-1), "pin 1 2 3", "and every call after it goes to the new session");
});