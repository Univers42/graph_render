/** The display panel's controls beyond theme, colour and size: each an action, so the console drives them too. */
import {
  BACKGROUNDS, EDGE_STYLES, NODE_PX, GLOW_STRENGTH, LINK_THICKNESS, TEXT_FADE,
} from "../state/settings.ts";
import { ActionRefusal } from "./registry.ts";
import { type StudioAction, chosen, flagArg, numberArg, textArg } from "./context.ts";
import { look } from "./look.ts";

const arrows: StudioAction = {
  id: "appearance.arrows", alias: "arrows", title: "Arrows", section: "Appearance",
  params: [{ name: "on", kind: "flag", title: "Arrows", control: "toggle", value: (state) => state.settings.appearance.arrows }],
  run: (context, args) => look(context, { arrows: flagArg(args, "on") }),
};

const fade: StudioAction = {
  id: "appearance.fade", alias: "fade", title: "Text fade", section: "Appearance",
  params: [{
    name: "value", kind: "number", title: "Text fade", control: "slider",
    min: TEXT_FADE.min, max: TEXT_FADE.max, step: 0.1, value: (state) => state.settings.appearance.textFade,
  }],
  run: (context, args) => look(context, { textFade: numberArg(args, "value") }),
};

const thickness: StudioAction = {
  id: "appearance.thickness", alias: "thickness", title: "Link thickness", section: "Appearance",
  params: [{
    name: "factor", kind: "number", title: "Link thickness", control: "slider",
    min: LINK_THICKNESS.min, max: LINK_THICKNESS.max, step: 0.05, value: (state) => state.settings.appearance.linkThickness,
  }],
  run: (context, args) => look(context, { linkThickness: numberArg(args, "factor") }),
};

const edgeStyle: StudioAction = {
  id: "appearance.edgestyle", alias: "edgestyle", title: "Edge style", section: "Appearance",
  params: [{
    name: "style", kind: "choice", title: "Edge style", control: "segmented",
    choices: () => EDGE_STYLES, value: (state) => state.settings.appearance.edgeStyle,
  }],
  run: (context, args) => look(context, { edgeStyle: chosen(EDGE_STYLES, textArg(args, "style"), "straight") }),
};

const glow: StudioAction = {
  id: "appearance.glow", alias: "glow", title: "Glow", section: "Appearance",
  params: [{ name: "on", kind: "flag", title: "Glow", control: "toggle", value: (state) => state.settings.appearance.glow }],
  run: (context, args) => look(context, { glow: flagArg(args, "on") }),
};

const glowStrength: StudioAction = {
  id: "appearance.glowstrength", alias: "glowstrength", title: "Glow strength", section: "Appearance",
  params: [{
    name: "value", kind: "number", title: "Glow strength", control: "slider",
    min: GLOW_STRENGTH.min, max: GLOW_STRENGTH.max, step: 0.1, value: (state) => state.settings.appearance.glowStrength,
  }],
  run: (context, args) => look(context, { glowStrength: numberArg(args, "value") }),
};

const background: StudioAction = {
  id: "appearance.background", alias: "background", title: "Background", section: "Appearance",
  params: [{
    name: "mode", kind: "choice", title: "Background", control: "segmented",
    choices: () => BACKGROUNDS, value: (state) => state.settings.appearance.background,
  }],
  run: (context, args) => look(context, { background: chosen(BACKGROUNDS, textArg(args, "mode"), "theme") }),
};

const minPx: StudioAction = {
  id: "appearance.minpx", alias: "minpx", title: "Smallest node (px)", section: "Appearance",
  params: [{
    name: "px", kind: "number", title: "Smallest node (px)", control: "number",
    min: NODE_PX.min, max: NODE_PX.max, step: 0.5, value: (state) => state.settings.appearance.minRadius,
  }],
  run: (context, args) => {
    const px = numberArg(args, "px");
    const max = context.state().settings.appearance.maxRadius;
    if (px > max) throw new ActionRefusal("bad-value", `smallest node ${px} px is above the maximum ${max} px`);
    return look(context, { minRadius: px });
  },
};

const maxPx: StudioAction = {
  id: "appearance.maxpx", alias: "maxpx", title: "Largest node (px)", section: "Appearance",
  params: [{
    name: "px", kind: "number", title: "Largest node (px)", control: "number",
    min: NODE_PX.min, max: NODE_PX.max, step: 0.5, value: (state) => state.settings.appearance.maxRadius,
  }],
  run: (context, args) => {
    const px = numberArg(args, "px");
    const min = context.state().settings.appearance.minRadius;
    if (px < min) throw new ActionRefusal("bad-value", `largest node ${px} px is below the minimum ${min} px`);
    return look(context, { maxRadius: px });
  },
};

const ANIMATE_MS = { min: 0, max: 30_000, initial: 10_000 } as const;

const animate: StudioAction = {
  id: "appearance.animate", alias: "animate", title: "Animate", section: "Appearance",
  params: [{
    name: "ms", kind: "number", title: "Duration (ms)", control: "number",
    min: ANIMATE_MS.min, max: ANIMATE_MS.max, step: 100, value: () => ANIMATE_MS.initial,
  }],
  run: (context, args) => context.animation.start(numberArg(args, "ms")),
};

const animateStop: StudioAction = {
  id: "appearance.animatestop", alias: "animatestop", title: "Stop animation", section: "Appearance",
  params: [],
  run: (context) => context.animation.cancel(),
};

export const DISPLAY_ACTIONS: readonly StudioAction[] = [
  arrows, fade, thickness, edgeStyle, glow, glowStrength, background, minPx, maxPx, animate, animateStop,
];
