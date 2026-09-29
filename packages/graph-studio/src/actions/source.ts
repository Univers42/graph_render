/** Where the graph comes from. Each of these loads, lays out and draws. */
import { FIXTURES } from "../source/fixtures.ts";
import { MAX_DEGREE, MAX_NODES, SHAPES } from "../source/synthetic.ts";
import type { StudioState } from "../state/model.ts";
import { OPENING_SOURCE, type Source, withSettings } from "../state/settings.ts";
import { type StudioAction, type StudioContext, chosen, numberArg, textArg } from "./context.ts";
import type { Outcome } from "./registry.ts";

const SEED_MAX = 4294967295;

function generated(state: StudioState): typeof OPENING_SOURCE {
  const { source } = state.settings;
  return source.kind === "synthetic" ? source : OPENING_SOURCE;
}

function open(context: StudioContext, source: Source): Promise<Outcome> {
  return context.apply(context.recall(source) ?? withSettings(context.state().settings, { source }));
}

const synthetic: StudioAction = {
  id: "source.synthetic", alias: "synthetic", title: "Generate", section: "Source",
  params: [
    { name: "nodes", kind: "int", title: "Nodes", min: 2, max: MAX_NODES, control: "number", value: (state) => generated(state).nodes },
    { name: "degree", kind: "int", title: "Links per node", min: 0, max: MAX_DEGREE, step: 1, control: "slider", value: (state) => generated(state).degree },
    { name: "seed", kind: "int", title: "Seed", min: 0, max: SEED_MAX, control: "number", value: (state) => generated(state).seed },
    { name: "shape", kind: "choice", title: "Shape", choices: () => SHAPES, control: "segmented", value: (state) => generated(state).shape },
  ],
  run: (context, args) => open(context, {
    kind: "synthetic",
    nodes: numberArg(args, "nodes"),
    degree: numberArg(args, "degree"),
    seed: numberArg(args, "seed"),
    shape: chosen(SHAPES, textArg(args, "shape"), OPENING_SOURCE.shape),
  }),
};

const fixture: StudioAction = {
  id: "source.fixture", alias: "fixture", title: "Bundled graph", section: "Source",
  params: [{
    name: "path", kind: "choice", title: "Fixture", choices: () => FIXTURES, control: "select",
    value: (state) => (state.settings.source.kind === "fixture" ? state.settings.source.path : (FIXTURES[0] ?? "")),
  }],
  run: (context, args) => open(context, { kind: "fixture", path: textArg(args, "path") }),
};

const document: StudioAction = {
  id: "source.document", alias: "document", title: "Open a file", section: "Source",
  params: [
    {
      name: "name", kind: "text", title: "Name", control: "text",
      value: (state) => (state.settings.source.kind === "document" ? state.settings.source.name : "document.json"),
    },
    {
      name: "text", kind: "text", title: "Ingest JSON", control: "file",
      value: (state) => (state.settings.source.kind === "document" ? state.settings.source.text : ""),
    },
  ],
  run: (context, args) => open(context, { kind: "document", name: textArg(args, "name"), text: textArg(args, "text") }),
};

export const SOURCE_ACTIONS: readonly StudioAction[] = [synthetic, fixture, document];
