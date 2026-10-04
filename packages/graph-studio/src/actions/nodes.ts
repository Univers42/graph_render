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
import type { ColumnRowsLike } from "../source/synthetic-columns.ts";
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

/** `value` as a list of distinct strings in first-seen order, or null when it is not a list of strings. */
export function distinctIds(value: unknown): readonly string[] | null {
  if (!Array.isArray(value) || !value.every((id): id is string => typeof id === "string")) return null;
  return [...new Set(value)];
}

/**
 * The node of each id, in order, or null when any id is in no node. One pass over the frame's
 * ids for any number of wanted ones: O(nodes + ids), not their product.
 */
export function nodesWithIds(meta: GraphMeta, ids: readonly string[]): number[] | null {
  const wanted = new Map(ids.map((id) => [id, -1]));
  meta.ids.forEach((id, node) => {
    if (wanted.get(id) === -1) wanted.set(id, node);
  });
  const nodes = ids.map((id) => wanted.get(id) ?? -1);
  return nodes.includes(-1) ? null : nodes;
}

function idList(text: string): readonly string[] {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new ActionRefusal("bad-value", "`ids` is not a JSON list");
  }
  const ids = distinctIds(value);
  if (ids === null) throw new ActionRefusal("bad-value", "`ids` is not a JSON list of strings");
  return ids;
}

function exactNodes(meta: GraphMeta, ids: readonly string[]): number[] {
  const nodes = nodesWithIds(meta, ids);
  if (nodes === null) throw new ActionRefusal("bad-value", "an id in `ids` is in no node of this graph");
  return nodes;
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

/**
 * The columns a host handed over, waiting for the dispatch that applies them.
 *
 * WHY a slot and not a parameter: the registry's `ArgValue` is a string, a number or a boolean,
 * so a `Uint32Array` would be refused as "must be text" — and `formatCommand` writes every
 * argument into the log, which would put a 238 MB document in front of the reader. So
 * `host/api.ts` offers the rows and dispatches this action on the next statement, and the action
 * takes them here. That is safe because `dispatch` runs `run` before its first `await`: nothing
 * else can run between the offer and the read, and `run` reads before it yields.
 */
let offered: ColumnRowsLike | null = null;

/** Called by the host verb on the statement before it dispatches `source.columns`. */
export function offerColumns(rows: ColumnRowsLike): void {
  offered = rows;
}

/** The offered rows, and the slot emptied: nothing else may read them. */
function takeColumns(): ColumnRowsLike {
  const rows = offered;
  if (rows === null) throw new ActionRefusal("unknown-param", "`columns` needs the columns the host offered; nothing was offered");
  offered = null;
  return rows;
}

/**
 * The same road as `source.host`, for the columnar document (`docs/contract/ingest-columns.md`).
 * `host: true` is what keeps the rows out of the page's storage, exactly as it keeps the text out.
 */
const hostColumns: StudioAction = {
  id: "source.columns", alias: "columns", title: "Draw columns the host handed over", section: null,
  params: [{ name: "name", kind: "text", title: "Name", value: () => "host columns" }],
  run: (context, args) => {
    const source = { kind: "columns", name: textArg(args, "name"), rows: takeColumns(), host: true } as const;
    return context.apply(withSettings(context.state().settings, { source }));
  },
};

export const NODE_ACTIONS: readonly StudioAction[] = [goTo, pick, open, host, hostColumns];
