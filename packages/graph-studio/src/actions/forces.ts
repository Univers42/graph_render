/**
 * The nine force knobs, the Spread and Compact presets, Reset and Animate, as registry
 * actions: the panel and the console reach the same value through the same check. What they
 * drive is a `ForceLink`, so the studio ships the panel before the motor's live session
 * exists: with `NO_FORCE_LINK` every one is unavailable and says why.
 */
import { DEFAULT_KNOBS, type ForceKnobs, KNOB_LIMITS, type KnobName, NO_ADAPTER_REASON } from "../motor/live.ts";
import type { StudioState } from "../state/model.ts";
import { flagArg, numberArg } from "./context.ts";
import { type Action, ActionRefusal } from "./registry.ts";

/** No action here reads its context, so any context will do. */
type ForceAction<Context> = Action<StudioState, Context>;

export interface ForceLink {
  /** Why the live simulation cannot run, or `null`. */
  readonly disabled: () => string | null;
  /** The knobs as `state` holds them, or as they are now when no state is given. */
  readonly knobs: (state?: StudioState) => ForceKnobs;
  /** `heat`: the alpha the settle is reheated to at least; absent, the loop's own nudge. */
  readonly set: (knobs: ForceKnobs, heat?: number) => void;
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

const SECTION = "Forces";

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

/**
 * Spread leaves half a radius again around the largest drawn disc, and repels at three times
 * the motor's default unless the repel is already stronger.
 *
 * Ponytail: both numbers were picked, then checked on two sources only (the clustered
 * fixture and a 10 000-node synthetic graph, `docs/measurements/ux-forces-full.md`). A graph
 * with long chains may need more repel than this, which the Repel slider still gives.
 */
const SPREAD_MARGIN = 0.5;
const SPREAD_CHARGE = 3 * DEFAULT_KNOBS.charge;
/** Ponytail: picked, not measured — tighter than the defaults, still kept apart by the spacing. */
const COMPACT = { charge: -30, linkDistance: 30, gravity: 0.05 };
/**
 * A preset reheats as hot as a fresh settle, from the drawing on screen. WHY not the loop's
 * 0.3 nudge: a preset moves several knobs at once, and at 0.3 the settle cools before the new
 * spacing has pushed the discs apart — `spread` on the 10k fixture cut overlaps by 49.3% at
 * 0.3 and by 69.7% at 1 (docs/measurements/ux-forces-full.md).
 */
export const PRESET_HEAT = 1;

/**
 * The spacing that keeps two discs of the drawn size from touching, plus `margin` of a radius,
 * on the slider's half-unit grid.
 *
 * Ponytail: one spacing for every node, sized to the largest disc — small nodes in a graph
 * of very mixed sizes get room meant for the big one (looser, never tighter). The 1.25 px
 * floor of a tiny disc is taken at the zoom of the press: zoom in later and the spacing is
 * wider than the discs now drawn; pressing again re-sizes it.
 */
function spacingFor(link: ForceLink, margin: number): number {
  const drawn = link.drawn?.() ?? null;
  if (drawn === null || !Number.isFinite(drawn)) {
    throw new ActionRefusal("unavailable", "nothing is drawn to size the node spacing on");
  }
  return Math.min(KNOB_LIMITS.collideRadius.max, Math.ceil(drawn * (1 + margin) * 2) / 2);
}

function spoken(knobs: ForceKnobs): string {
  return `node spacing ${knobs.collideRadius}, repel ${-knobs.charge}, link distance ${knobs.linkDistance}`;
}

function presetActions<Context>(link: ForceLink): readonly ForceAction<Context>[] {
  return [
    {
      id: "forces.spread", alias: "spread", title: "Spread", section: SECTION, params: [],
      available: () => link.disabled(),
      run: () => {
        const now = link.knobs();
        const next = { ...now, collideRadius: spacingFor(link, SPREAD_MARGIN), charge: Math.min(now.charge, SPREAD_CHARGE) };
        link.set(next, PRESET_HEAT);
        return { message: `spread: ${spoken(next)}` };
      },
    },
    {
      id: "forces.compact", alias: "compact", title: "Compact", section: SECTION, params: [],
      available: () => link.disabled(),
      run: () => {
        const next = { ...link.knobs(), ...COMPACT, collideRadius: spacingFor(link, 0) };
        link.set(next, PRESET_HEAT);
        return { message: `compact: ${spoken(next)}` };
      },
    },
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

