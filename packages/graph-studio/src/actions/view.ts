/** The camera, the console and the running motor: what the viewer does, not the drawing. */
import type { GraphMeta } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import { type StudioAction, type StudioParam, numberArg, textArg, flagArg } from "./context.ts";
import { ActionRefusal, matchChoice } from "./registry.ts";

const CANDIDATES_SHOWN = 8;
/**
 * WHY a bound on a pan: a number in pixels with no bound is a number a typo turns into
 * 1e300, and the camera that comes back cannot be panned out of. Four screens wide is
 * already further than any view.
 */
export const MAX_PAN = 2000;

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

const reset: StudioAction = {
  id: "view.reset", alias: "reset", title: "Reset the camera to 1:1", section: null, params: [],
  run: (context) => {
    context.view.reset();
    return { message: "reset to 1:1" };
  },
};

const zoom: StudioAction = {
  id: "view.zoom", alias: "zoom", title: "Zoom by a factor", section: null,
  params: [{ name: "factor", kind: "number", title: "Factor", min: 0.02, max: 40, value: () => 1.25 }],
  run: (context, args) => {
    const factor = numberArg(args, "factor");
    context.view.zoomBy(factor);
    return { message: `zoomed ×${factor}` };
  },
};

const pan: StudioAction = {
  id: "view.pan", alias: "pan", title: "Pan by screen pixels", section: null,
  params: [
    { name: "dx", kind: "number", title: "Right", min: -MAX_PAN, max: MAX_PAN, value: () => 0 },
    { name: "dy", kind: "number", title: "Down", min: -MAX_PAN, max: MAX_PAN, value: () => 0 },
  ],
  run: (context, args) => {
    const dx = numberArg(args, "dx");
    const dy = numberArg(args, "dy");
    context.view.panBy({ x: dx, y: dy });
    return { message: `panned ${dx} × ${dy}` };
  },
};

const deselect: StudioAction = {
  id: "view.clear", alias: "deselect", title: "Clear the selection and leave the local graph", section: null, params: [],
  run: (context) => {
    context.view.select(-1);
    context.view.showAll();
    return { message: "selection cleared" };
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

const local: StudioAction = {
  id: "view.local", alias: "local", title: "Show the local graph around a node", section: null,
  params: [
    { name: "id", kind: "text", title: "Node", value: () => "" },
    { name: "depth", kind: "int", title: "Depth", min: 1, max: 5, value: () => 1 },
    { name: "incoming", kind: "flag", title: "Incoming", value: () => false },
    { name: "outgoing", kind: "flag", title: "Outgoing", value: () => false },
    { name: "neighbours", kind: "flag", title: "Neighbours", value: () => false },
  ],
  available: described,
  run: (context, args) => {
    const { meta } = context.state();
    if (meta === null) throw new ActionRefusal("unavailable", "nothing is drawn");
    const node = nodeNamed(meta, textArg(args, "id"));
    const depth = numberArg(args, "depth");
    const incoming = flagArg(args, "incoming");
    const outgoing = flagArg(args, "outgoing");
    const neighbours = flagArg(args, "neighbours");
    const options = { depth, incoming, outgoing, neighbours };
    const visible = context.view.local(node, options);
    return { message: `local graph depth ${depth}`, digest: JSON.stringify(visible) };
  },
};

const cancel: StudioAction = {
  id: "view.cancel", alias: "cancel", title: "Stop what the motor is doing", section: null, params: [],
  run: (context) => ({ message: context.stop() ? "stopped" : "nothing was running" }),
};

const emptyConsole: StudioAction = {
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

export const VIEW_ACTIONS: readonly StudioAction[] =
  [fit, reset, zoom, pan, focus, local, cancel, deselect, emptyConsole, help];
