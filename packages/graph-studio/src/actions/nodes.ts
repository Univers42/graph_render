/**
 * What a host asks of the nodes, routed through the registry like every other action: by exact
 * id in the current frame, never by name (`host-api.md`, verdict 2), so a host's string that
 * happens to be another node's label moves nothing.
 *
 * WHY not `view.focus`: that one falls back to label matching, which is right for a person
 * typing in the console and wrong for a program holding ids.
 */
import { OPEN_VIAS, type OpenVia } from "../host/contract.ts";
import { MAX_DOCUMENT_CHARS } from "../source/limits.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import { withSettings } from "../state/settings.ts";
import { type StudioAction, chosen, textArg } from "./context.ts";
import { ActionRefusal } from "./registry.ts";
import { described } from "./view.ts";

function drawn(state: StudioState): GraphMeta {
  if (state.meta === null) throw new ActionRefusal("unavailable", "nothing is drawn");
  return state.meta;
}

function exactNode(meta: GraphMeta, id: string): number {
  const node = meta.ids.indexOf(id);
  if (node < 0) throw new ActionRefusal("bad-value", `no node has the id \`${id}\``);
  return node;
}

function idList(text: string): readonly string[] {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new ActionRefusal("bad-value", "`ids` is not a JSON list");
  }
  if (!Array.isArray(value) || !value.every((id): id is string => typeof id === "string")) {
    throw new ActionRefusal("bad-value", "`ids` is not a JSON list of strings");
  }
  return [...new Set(value)];
}

/** One pass over the frame's ids for any number of wanted ones: O(nodes + ids), not their product. */
function exactNodes(meta: GraphMeta, ids: readonly string[]): number[] {
  const wanted = new Map(ids.map((id) => [id, -1]));
  meta.ids.forEach((id, node) => {
    if (wanted.get(id) === -1) wanted.set(id, node);
  });
  const unknown = ids.filter((id) => wanted.get(id) === -1);
  if (unknown.length > 0) throw new ActionRefusal("bad-value", `no node has the id \`${unknown[0] ?? ""}\` (${unknown.length} unknown)`);
  return ids.map((id) => wanted.get(id) ?? -1);
}

const goTo: StudioAction = {
  id: "node.focus", alias: "goto", title: "Centre and select the node with this exact id", section: null,
  params: [{ name: "id", kind: "text", title: "Node id", value: () => "" }],
  available: described,
  run: (context, args) => {
    const meta = drawn(context.state());
    const node = exactNode(meta, textArg(args, "id"));
    context.view.focus(node);
    return { message: `${meta.labels[node] ?? ""} (${meta.ids[node] ?? ""})` };
  },
};

const pick: StudioAction = {
  id: "node.select", alias: "select", title: "Select the nodes with these exact ids, the last one primary", section: null,
  params: [{ name: "ids", kind: "text", title: "Node ids, as a JSON list", max: MAX_DOCUMENT_CHARS, value: () => "[]" }],
  available: described,
  run: (context, args) => {
    const nodes = exactNodes(drawn(context.state()), idList(textArg(args, "ids")));
    context.view.selectMany(nodes);
    return { message: `${nodes.length} selected` };
  },
};

const open: StudioAction = {
  id: "node.open", alias: "open", title: "Ask the host to open a node", section: null,
  params: [
    { name: "id", kind: "text", title: "Node id", value: () => "" },
    { name: "via", kind: "choice", title: "Asked by", choices: () => OPEN_VIAS, value: () => OPEN_VIAS[0] },
  ],
  available: described,
  run: (context, args) => {
    const meta = drawn(context.state());
    const id = meta.ids[exactNode(meta, textArg(args, "id"))] ?? "";
    const via: OpenVia = chosen(OPEN_VIAS, textArg(args, "via"), OPEN_VIAS[0]);
    context.open(id, via);
    return { message: `asked the host to open ${id}` };
  },
};

/**
 * A document the host handed over. Never recalled and never remembered: `host: true` keeps its
 * text out of the page's storage (`state/persist.ts`, verdict 3).
 */
const host: StudioAction = {
  id: "source.host", alias: "load", title: "Draw a document the host handed over", section: null,
  params: [
    { name: "name", kind: "text", title: "Name", value: () => "host" },
    { name: "text", kind: "text", title: "Ingest JSON", max: MAX_DOCUMENT_CHARS, value: () => "" },
  ],
  run: (context, args) => {
    const source = { kind: "document", name: textArg(args, "name"), text: textArg(args, "text"), host: true } as const;
    return context.apply(withSettings(context.state().settings, { source }));
  },
};

export const NODE_ACTIONS: readonly StudioAction[] = [goTo, pick, open, host];
