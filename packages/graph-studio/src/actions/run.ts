/** The stages the motor runs over the loaded graph: a layout, an edge pass, an analysis. */
import { MotorFailure } from "../motor/client.ts";
import type { StudioState } from "../state/model.ts";
import { type Settings, withAppearance, withSettings } from "../state/settings.ts";
import { type StudioAction, type StudioContext, textArg } from "./context.ts";
import type { Outcome } from "./registry.ts";

export const OFF = "off";

function loaded(state: StudioState): string | null {
  return state.graph === null ? "no graph is loaded" : null;
}

function idOrNull(value: string): string | null {
  return value === OFF ? null : value;
}

async function restyleEdges(context: StudioContext, postId: string | null): Promise<Outcome> {
  const outcome = await context.apply(withSettings(context.state().settings, { edges: postId }));
  const refused = context.state().run?.postError ?? null;
  // Asked for by name and refused: the drawing shows the layout's edges, and that is a failure here.
  if (postId !== null && refused !== null) throw new MotorFailure(refused);
  return outcome;
}

/** An analysis is switched on to be seen; switching it off gives back what it replaced. */
function withAnalysis(settings: Settings, analysis: string | null): Settings {
  const { colourBy, sizeBy } = settings.appearance;
  const look = analysis === null
    ? { colourBy: colourBy === "analysis" ? "group" : colourBy, sizeBy: sizeBy === "analysis" ? "weight" : sizeBy } as const
    : { colourBy: "analysis" } as const;
  return withSettings(withAppearance(settings, look), { analysis });
}

async function analyse(context: StudioContext, analysis: string | null): Promise<Outcome> {
  const before = context.state().settings.appearance;
  try {
    return await context.apply(withAnalysis(context.state().settings, analysis));
  } catch (error) {
    context.look(withAppearance(context.state().settings, { colourBy: before.colourBy, sizeBy: before.sizeBy }));
    throw error;
  }
}

const layout: StudioAction = {
  id: "layout.run", alias: "layout", title: "Layout", section: "Layout",
  params: [{
    name: "id", kind: "choice", title: "Layout", control: "list",
    choices: (state) => state.catalog?.layouts ?? [state.settings.layout],
    value: (state) => state.settings.layout,
  }],
  available: loaded,
  run: (context, args) => context.apply(withSettings(context.state().settings, { layout: textArg(args, "id") })),
};

const edges: StudioAction = {
  id: "edges.style", alias: "edges", title: "Edge pass", section: "Edges",
  params: [{
    name: "id", kind: "choice", title: "Edges", control: "list",
    choices: (state) => [OFF, ...(state.catalog?.posts ?? [])],
    value: (state) => state.settings.edges ?? OFF,
  }],
  available: loaded,
  run: (context, args) => restyleEdges(context, idOrNull(textArg(args, "id"))),
};

const analysis: StudioAction = {
  id: "analysis.run", alias: "analysis", title: "Analysis", section: "Analysis",
  params: [{
    name: "id", kind: "choice", title: "Analysis", control: "list",
    choices: (state) => [OFF, ...(state.catalog?.analyses ?? [])],
    value: (state) => state.settings.analysis ?? OFF,
  }],
  available: loaded,
  run: (context, args) => analyse(context, idOrNull(textArg(args, "id"))),
};

export const RUN_ACTIONS: readonly StudioAction[] = [layout, edges, analysis];
