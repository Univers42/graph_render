/** How the drawing looks. None of these asks the motor for anything. */
import {
  COLOUR_BY, LABEL_MODES, NODE_SCALE, SIZE_BY, THEMES, type Appearance, withAppearance,
} from "../state/settings.ts";
import { type StudioAction, type StudioContext, chosen, numberArg, textArg } from "./context.ts";
import { ActionRefusal, type Outcome } from "./registry.ts";

function look(context: StudioContext, patch: Partial<Appearance>): Outcome {
  const byAnalysis = patch.colourBy === "analysis" || patch.sizeBy === "analysis";
  if (byAnalysis && context.state().analysis === null) {
    throw new ActionRefusal("unavailable", "no analysis has run; pick one with `analysis` first");
  }
  return context.look(withAppearance(context.state().settings, patch));
}

const theme: StudioAction = {
  id: "appearance.theme", alias: "theme", title: "Theme", section: "Appearance",
  params: [{
    name: "name", kind: "choice", title: "Theme", control: "segmented",
    choices: () => THEMES, value: (state) => state.settings.appearance.theme,
  }],
  run: (context, args) => look(context, { theme: chosen(THEMES, textArg(args, "name"), "dark") }),
};

const colour: StudioAction = {
  id: "appearance.colour", alias: "colour", title: "Colour by", section: "Appearance",
  params: [{
    name: "by", kind: "choice", title: "Colour by", control: "segmented",
    choices: () => COLOUR_BY, value: (state) => state.settings.appearance.colourBy,
  }],
  run: (context, args) => look(context, { colourBy: chosen(COLOUR_BY, textArg(args, "by"), "group") }),
};

const size: StudioAction = {
  id: "appearance.size", alias: "size", title: "Size by", section: "Appearance",
  params: [{
    name: "by", kind: "choice", title: "Size by", control: "segmented",
    choices: () => SIZE_BY, value: (state) => state.settings.appearance.sizeBy,
  }],
  run: (context, args) => look(context, { sizeBy: chosen(SIZE_BY, textArg(args, "by"), "weight") }),
};

const scale: StudioAction = {
  id: "appearance.scale", alias: "scale", title: "Node size", section: "Appearance",
  params: [{
    name: "factor", kind: "number", title: "Node size", control: "slider",
    min: NODE_SCALE.min, max: NODE_SCALE.max, step: 0.05, value: (state) => state.settings.appearance.nodeScale,
  }],
  run: (context, args) => look(context, { nodeScale: numberArg(args, "factor") }),
};

const labels: StudioAction = {
  id: "appearance.labels", alias: "labels", title: "Labels", section: "Appearance",
  params: [{
    name: "mode", kind: "choice", title: "Labels", control: "segmented",
    choices: () => LABEL_MODES, value: (state) => state.settings.appearance.labels,
  }],
  run: (context, args) => look(context, { labels: chosen(LABEL_MODES, textArg(args, "mode"), "auto") }),
};

export const APPEARANCE_ACTIONS: readonly StudioAction[] = [theme, colour, size, scale, labels];
