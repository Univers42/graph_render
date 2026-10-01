/**
 * The force console commands, end to end: the line as typed in the console goes through
 * `parseCommand` and `resolve` into the link, and a link with nothing behind it refuses with
 * the reason rather than answering. Every forces action is reachable by its dotted id and by
 * its alias, and is registered exactly once.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type ForceLink, NO_FORCE_LINK, forceActions } from "../src/actions/forces.ts";
import type { StudioContext } from "../src/actions/context.ts";
import { studioActions } from "../src/actions/all.ts";
import { parseCommand } from "../src/console/parse.ts";
import { ActionRefusal, createRegistry } from "../src/actions/registry.ts";
import { DEFAULT_KNOBS, type ForceKnobs } from "../src/motor/live.ts";
import { type StudioState, initialState } from "../src/state/model.ts";

interface Calls {
  readonly sets: ForceKnobs[];
  readonly animated: boolean[];
  readonly paused: boolean[];
}

function live(): { readonly link: ForceLink; readonly calls: Calls } {
  const sets: ForceKnobs[] = [];
  const animated: boolean[] = [];
  const paused: boolean[] = [];
  let knobs = DEFAULT_KNOBS;
  let pausedNow = false;
  const link: ForceLink = {
    disabled: () => null,
    knobs: () => knobs,
    set: (next) => { knobs = next; sets.push(next); },
    animate: (on) => { animated.push(on); },
    animating: () => animated.at(-1) ?? false,
    pause: () => { pausedNow = true; paused.push(true); },
    resume: () => { pausedNow = false; paused.push(false); },
    paused: () => pausedNow,
  };
  return { link, calls: { sets, animated, paused } };
}

function registryOf(link: ForceLink) {
  return createRegistry<StudioState, null>(forceActions<null>(link));
}

const STATE = initialState();
const context = null;

/** The whole console path for one typed line: the words, the resolve, then the run. */
async function typed(link: ForceLink, line: string): Promise<{ readonly ok: boolean; readonly message: string }> {
  const registry = registryOf(link);
  try {
    const command = parseCommand(line, registry);
    const { action, args } = registry.resolve(command.id, command.raw, STATE);
    const outcome = await action.run(context, args);
    return { ok: true, message: outcome.message };
  } catch (error) {
    return { ok: false, message: error instanceof Error ? error.message : String(error) };
  }
}

test("every forces command is callable from the console as typed", async () => {
  const { link, calls } = live();
  const lines = ["forces.center 0.4", "forces.repel 300", "forces.link 1.5", "forces.distance 120"];
  for (const line of lines) {
    const out = await typed(link, line);
    assert.equal(out.ok, true, `${line}: ${out.message}`);
  }
  assert.deepEqual(calls.sets.at(-1), { gravity: 0.4, charge: -300, linkStrengthScale: 1.5, linkDistance: 120 });
});

test("the alias is the console name too, and it reaches the same link", async () => {
  const { link, calls } = live();
  await typed(link, "repel 250");
  assert.deepEqual(calls.sets.at(-1)?.charge, -250);
});

test("forces.animate starts the settle and forces.reset puts the knobs back", async () => {
  const { link, calls } = live();
  await typed(link, "forces.center 0.9");
  assert.deepEqual((await typed(link, "forces.reset")).ok, true);
  assert.deepEqual(calls.sets.at(-1), DEFAULT_KNOBS);
  await typed(link, "forces.animate on");
  assert.deepEqual(calls.animated, [true]);
});

test("forces.pause stops the loop and forces.resume starts it again, and the two are distinct", async () => {
  const { link, calls } = live();
  await typed(link, "forces.pause");
  assert.deepEqual(calls.paused, [true]);
  assert.equal(link.paused(), true);
  await typed(link, "forces.resume");
  assert.deepEqual(calls.paused, [true, false]);
  assert.equal(link.paused(), false);
  assert.deepEqual(calls.animated, [], "pause and resume are not the animate toggle");
});

test("a pause says so in the log, and says again when it is already paused", async () => {
  const { link, calls } = live();
  assert.match((await typed(link, "forces.pause")).message, /paused/);
  await typed(link, "forces.pause");
  assert.deepEqual(calls.paused, [true, true], "pausing twice is not an error");
});

test("a forces command with a bad value is refused with the range, and the link is untouched", async () => {
  const { link, calls } = live();
  const out = await typed(link, "forces.repel 5000");
  assert.equal(out.ok, false);
  assert.match(out.message, /must be in -1000\.\.0|0\.\.1000/, out.message);
  assert.equal(calls.sets.length, 0);
});

test("a value the console cannot read as a number is refused, not applied as NaN", async () => {
  const { link, calls } = live();
  for (const line of ["forces.center abc", "forces.repel -", 'forces.link distance=wide']) {
    const out = await typed(link, line);
    assert.equal(out.ok, false, `${line} was accepted`);
    assert.doesNotMatch(out.message, /NaN was sent/, line);
  }
  assert.equal(calls.sets.length, 0, "nothing unreadable reached the motor");
});

test("an unknown forces word is refused with the nearest names, not run as a knob", async () => {
  const { calls } = live();
  const out = await typed(live().link, "forces.pull 3");
  assert.equal(out.ok, false);
  assert.match(out.message, /not a command|nearest/);
  assert.equal(calls.sets.length, 0);
});

test("a knob with no value keeps the motor's own, which is what the panel is showing", async () => {
  const { link, calls } = live();
  assert.equal((await typed(link, "forces.distance")).ok, true);
  assert.equal(calls.sets.length, 1);
  assert.equal(calls.sets[0]?.linkDistance, DEFAULT_KNOBS.linkDistance);
});

test("with no session behind it every forces command says why, and none reaches a motor", () => {
  const { calls } = live();
  const registry = registryOf(NO_FORCE_LINK);
  for (const action of forceActions<null>(NO_FORCE_LINK)) {
    assert.equal(action.available?.(STATE), NO_FORCE_LINK.disabled(), action.id);
    assert.throws(
      () => registry.resolve(action.id, {}, STATE),
      (error: unknown) => error instanceof ActionRefusal && error.code === "unavailable",
      action.id,
    );
  }
  assert.equal(calls.sets.length, 0);
});

test("the forces actions are registered once each, in one section, under ids that cannot collide", () => {
  const registry = createRegistry<StudioState, StudioContext>(studioActions());
  const forces = registry.actions.filter((action) => action.section === "Forces");
  const ids = forces.map((action) => action.id);
  assert.deepEqual(ids, ["forces.center", "forces.repel", "forces.link", "forces.distance", "forces.reset", "forces.animate", "forces.pause", "forces.resume"]);
  assert.equal(new Set(ids).size, ids.length, "an id twice would throw at registry build");
  for (const action of forces) {
    assert.equal(registry.find(action.id), action, action.id);
    assert.equal(registry.find(action.alias), action, action.alias);
  }
});

test("every forces alias is offered by suggest, so the console autocompletes it", () => {
  const registry = createRegistry<StudioState, StudioContext>(studioActions());
  // suggest speaks the console's names, which are the aliases: a dotted id is typed by hand.
  const aliases = forceActions<null>(NO_FORCE_LINK).map((action) => action.alias);
  for (const alias of aliases) {
    assert.ok(registry.suggest(alias).includes(alias), `${alias} was not offered for itself`);
  }
  assert.deepEqual(registry.suggest("forcespause"), ["forcespause"]);
});