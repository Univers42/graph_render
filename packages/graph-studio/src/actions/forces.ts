/**
 * The nine force knobs, the Spread and Compact presets, Reset and Animate, as registry
 * actions: the panel and the console reach the same value through the same check. What they
 * drive is a `ForceLink`, so the studio ships the panel before the motor's live session
 * exists: with `NO_FORCE_LINK` every one is unavailable and says why.
 */
import { DEFAULT_KNOBS, type ForceKnobs, KNOB_LIMITS, type KnobName, NO_ADAPTER_REASON } from "../motor/live.ts";
import type { StudioState } from "../state/model.ts";
import { flagArg, numberArg } from "./context.ts";
import { presetActions } from "./forcePresets.ts";
import type { Action } from "./registry.ts";

/** No action here reads its context, so any context will do. */
type ForceAction<Context> = Action<StudioState, Context>;

export interface ForceLink {
  /** Why the live simulation cannot run, or `null`. */
  readonly disabled: () => string | null;
  /** The knobs as `state` holds them, or as they are now when no state is given. */
  readonly knobs: (state?: StudioState) => ForceKnobs;
  readonly set: (knobs: ForceKnobs) => void;
  /** Runs the settle from random positions, or stops it. */
  readonly animate: (on: boolean) => void;
  readonly animating: () => boolean;
  /** Stops the loop where it is, keeping the pins and the alpha it reached. */
  readonly pause: () => void;
  /** Carries on from where the pause left it. */
  readonly resume: () => void;
  readonly paused: () => boolean;
  /**
   * The largest node disc as drawn, as a radius in layout units at the zoom of the call, or
   * `null` when nothing is drawn. Absent on a link that cannot see the drawing; the presets
   * need it and refuse without it.
   */
  readonly drawn?: () => number | null;
}

export const NO_FORCE_LINK: ForceLink = {
  disabled: () => NO_ADAPTER_REASON,
  knobs: () => DEFAULT_KNOBS,
  set: () => undefined,
  animate: () => undefined,
  animating: () => false,
  pause: () => undefined,
  resume: () => undefined,
  paused: () => false,
};

export const SECTION = "Forces";

/** How the panel shows a knob: as the motor holds it, negated (repel), or as `1 - it` (friction). */
type Face = "same" | "negated" | "complement";

/** Each face is its own inverse, so the same call turns a shown value back into the motor's. */
function faced(face: Face, value: number): number {
  if (face === "negated") return -value;
  if (face === "complement") return 1 - value;
  return value;
}

/** Twelve digits: friction at the default shows 0.42, not `1 - 0.58` = 0.41999999999999993. */
function tidy(value: number): number {
  return Number(value.toPrecision(12)) || 0;
}

interface Knob {
  readonly id: string;
  readonly alias: string;
  readonly title: string;
  readonly name: KnobName;
  readonly step: number;
  readonly face?: Face;
}

function knob<Context>(link: ForceLink, spec: Knob): ForceAction<Context> {
  const { id, alias, title, name, step, face = "same" } = spec;
  const ends = [faced(face, KNOB_LIMITS[name].min), faced(face, KNOB_LIMITS[name].max)].map(tidy);
  return {
    id, alias, title, section: SECTION,
    available: () => link.disabled(),
    params: [{
      name: "value", kind: "number", title, control: "slider", min: Math.min(...ends), max: Math.max(...ends), step,
      value: (state) => tidy(faced(face, link.knobs(state)[name])),
    }],
    run: (_context, args) => {
      const shown = numberArg(args, "value");
      link.set({ ...link.knobs(), [name]: faced(face, shown) });
      return { message: `${alias} ${shown}` };
    },
  };
}

function knobActions<Context>(link: ForceLink): readonly ForceAction<Context>[] {
  return [
    knob(link, { id: "forces.center", alias: "center", title: "Center force", name: "gravity", step: 0.01 }),
    knob(link, { id: "forces.repel", alias: "repel", title: "Repel force", name: "charge", step: 10, face: "negated" }),
    knob(link, { id: "forces.link", alias: "linkforce", title: "Link force", name: "linkStrengthScale", step: 0.05 }),
    knob(link, { id: "forces.distance", alias: "linkdistance", title: "Link distance", name: "linkDistance", step: 1 }),
    knob(link, { id: "forces.spacing", alias: "spacing", title: "Node spacing", name: "collideRadius", step: 0.5 }),
    knob(link, {
      id: "forces.friction", alias: "friction", title: "Friction", name: "velocityDecay", step: 0.01, face: "complement",
    }),
    knob(link, { id: "forces.cooling", alias: "cooling", title: "Cooling", name: "alphaDecay", step: 0.005 }),
    knob(link, { id: "forces.range", alias: "repelrange", title: "Repel range", name: "distanceMax", step: 10 }),
    // Raw theta, not inverted: the console and the motor's docs both speak of theta.
    knob(link, { id: "forces.accuracy", alias: "accuracy", title: "Accuracy", name: "theta", step: 0.05 }),
  ];
}

function controlActions<Context>(link: ForceLink): readonly ForceAction<Context>[] {
  return [
    {
      id: "forces.reset", alias: "forcesreset", title: "Reset", section: SECTION, params: [],
      available: () => link.disabled(),
      run: () => { link.set(DEFAULT_KNOBS); return { message: "forces reset to defaults" }; },
    },
    {
      id: "forces.animate", alias: "forcesanimate", title: "Animate", section: SECTION,
      available: () => link.disabled(),
      params: [{ name: "on", kind: "flag", title: "Animate", control: "toggle", value: () => link.animating() }],
      run: (_context, args) => {
        const on = flagArg(args, "on");
        link.animate(on);
        return { message: on ? "forces animate on" : "forces animate off" };
      },
    },
    {
      id: "forces.pause", alias: "forcespause", title: "Pause", section: SECTION, params: [],
      available: () => link.disabled(),
      run: () => { link.pause(); return { message: "forces paused" }; },
    },
    {
      id: "forces.resume", alias: "forcesresume", title: "Resume", section: SECTION, params: [],
      available: () => link.disabled(),
      run: () => { link.resume(); return { message: "forces resumed" }; },
    },
  ];
}

export function forceActions<Context>(link: ForceLink): readonly ForceAction<Context>[] {
  return [...knobActions<Context>(link), ...presetActions<Context>(link), ...controlActions<Context>(link)];
}

// Re-exported for the panel and the tests, which know the knobs by these and nothing else.
export type { ForceKnobs };
