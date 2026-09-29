/** The camera, the console and the running motor: what the viewer does, not the drawing. */
import type { GraphMeta } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import { type StudioAction, type StudioParam, numberArg, textArg } from "./context.ts";
import { ActionRefusal, matchChoice } from "./registry.ts";

const CANDIDATES_SHOWN = 8;

function described(state: StudioState): string | null {
  return state.meta === null ? "nothing is drawn" : null;
}

/** By id first, then by name the way a choice is matched: exact, or the one that holds it. */
function nodeNamed(meta: GraphMeta, wanted: string): number {
  const byId = meta.ids.indexOf(wanted);
  if (byId >= 0) return byId;
  const match = matchChoice(wanted, meta.labels);
  if (typeof match === "string") return meta.labels.indexOf(match);
  const some = match.slice(0, CANDIDATES_SHOWN).join(", ");
  throw new ActionRefusal("bad-value", match.length === 0 ? `no node is named \`${wanted}\`` : `\`${wanted}\` fits ${match.length} nodes: ${some}`);
}

function usage(action: StudioAction): string {
  const values = action.params.map((spec) => ` <${spec.name}>`).join("");
  return `${action.alias}${values} — ${action.title}`;
}

function valuesOf(spec: StudioParam, state: StudioState): string {
  if (spec.kind === "choice") return (spec.choices?.(state) ?? []).join(", ");
  if (spec.kind === "flag") return "on, off";
  if (spec.kind === "text") return "text";
  return `${spec.kind === "int" ? "a whole number" : "a number"} in ${spec.min ?? "-∞"}..${spec.max ?? "∞"}`;
}

const fit: StudioAction = {
  id: "view.fit", alias: "fit", title: "Fit the graph to the view", section: null, params: [],
  run: (context) => {
    context.view.fit();
    return { message: "fitted" };
  },
};

const zoom: StudioAction = {
  id: "view.zoom", alias: "zoom", title: "Zoom by a factor", section: null,
  params: [{ name: "factor", kind: "number", title: "Factor", min: 0.1, max: 10, value: () => 1.25 }],
  run: (context, args) => {
    context.view.zoomBy(numberArg(args, "factor"));
    return { message: `zoomed ×${numberArg(args, "factor")}` };
  },
};

const focus: StudioAction = {
  id: "view.focus", alias: "focus", title: "Centre a node by id or name", section: null,
  params: [{ name: "node", kind: "text", title: "Node", value: () => "" }],
  available: described,
  run: (context, args) => {
    const { meta } = context.state();
    if (meta === null) throw new ActionRefusal("unavailable", "nothing is drawn");
    const node = nodeNamed(meta, textArg(args, "node"));
    context.view.focus(node);
    return { message: `${meta.labels[node] ?? ""} (${meta.ids[node] ?? ""})` };
  },
};

const cancel: StudioAction = {
  id: "view.cancel", alias: "cancel", title: "Stop what the motor is doing", section: null, params: [],
  run: (context) => ({ message: context.stop() ? "stopped" : "nothing was running" }),
};

const clear: StudioAction = {
  id: "console.clear", alias: "clear", title: "Empty the console", section: null, params: [],
  run: (context) => {
    context.clearLog();
    return { message: "cleared" };
  },
};

const help: StudioAction = {
  id: "help", alias: "help", title: "The commands, or one command's values", section: null,
  params: [{ name: "command", kind: "text", title: "Command", value: () => "" }],
  run: (context, args) => {
    const word = textArg(args, "command");
    const actions = context.actions();
    if (word === "") return { message: `${actions.length} commands`, notes: actions.map(usage) };
    const action = actions.find((candidate) => candidate.alias === word || candidate.id === word);
    if (action === undefined) throw new ActionRefusal("bad-value", `\`${word}\` is not a command`);
    const state = context.state();
    return { message: usage(action), notes: action.params.map((spec) => `${spec.name}: ${valuesOf(spec, state)}`) };
  },
};

export const VIEW_ACTIONS: readonly StudioAction[] = [fit, zoom, focus, cancel, clear, help];
