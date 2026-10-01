/**
 * The four Obsidian-style force knobs, Reset and Animate, as registry actions: the panel and
 * the console reach the same value through the same check. What they drive is a `ForceLink`,
 * so the studio ships the panel before the motor's live session exists: with `NO_FORCE_LINK`
 * every one is unavailable and says why.
 */
import { DEFAULT_KNOBS, type ForceKnobs, KNOB_LIMITS, NO_ADAPTER_REASON } from "../motor/live.ts";
import type { StudioState } from "../state/model.ts";
import { flagArg, numberArg } from "./context.ts";
import type { Action } from "./registry.ts";

/** No action here reads its context, so any context will do. */
type ForceAction<Context> = Action<StudioState, Context>;

export interface ForceLink {
  /** Why the live simulation cannot run, or `null`. */
  readonly disabled: () => string | null;
  readonly knobs: () => ForceKnobs;
  readonly set: (knobs: ForceKnobs) => void;
  /** Runs the settle from the motor's seed, or stops it. */
  readonly animate: (on: boolean) => void;
  readonly animating: () => boolean;
  /** Stops the loop where it is, keeping the pins and the alpha it reached. */
  readonly pause: () => void;
  /** Carries on from where the pause left it. */
  readonly resume: () => void;
  readonly paused: () => boolean;
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

interface Knob {
  readonly id: string;
  readonly alias: string;
  readonly title: string;
  readonly name: keyof ForceKnobs;
  readonly step: number;
}

function knob<Context>(link: ForceLink, spec: Knob): ForceAction<Context> {
  const { id, alias, title, name, step } = spec;
  // The panel shows repel as a positive magnitude; the motor's charge is its negative.
  const flipped = name === "charge";
  const limits = KNOB_LIMITS[name];
  const min = flipped ? -limits.max : limits.min;
  const max = flipped ? -limits.min : limits.max;
  const sign = flipped ? -1 : 1;
  return {
    id, alias, title, section: SECTION,
    available: () => link.disabled(),
    params: [{
      name: "value", kind: "number", title, control: "slider", min, max, step,
      value: () => sign * link.knobs()[name] || 0,
    }],
    run: (_context, args) => {
      const value = sign * numberArg(args, "value");
      link.set({ ...link.knobs(), [name]: value });
      return { message: `${alias} ${numberArg(args, "value")}` };
    },
  };
}

export function forceActions<Context>(link: ForceLink): readonly ForceAction<Context>[] {
  return [
    knob(link, { id: "forces.center", alias: "center", title: "Center force", name: "gravity", step: 0.01 }),
    knob(link, { id: "forces.repel", alias: "repel", title: "Repel force", name: "charge", step: 10 }),
    knob(link, { id: "forces.link", alias: "linkforce", title: "Link force", name: "linkStrengthScale", step: 0.05 }),
    knob(link, { id: "forces.distance", alias: "linkdistance", title: "Link distance", name: "linkDistance", step: 1 }),
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
