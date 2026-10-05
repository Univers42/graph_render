/**
 * User groups laid over a base colouring. A group's colour takes the FIRST slots of the
 * palette; a node takes the first group whose query matches it, else its base colour shifted
 * past the groups.
 *
 * Ponytail: a group whose query `parseQuery` refuses matches nothing instead of throwing, so a
 * stale query silently colours no node. Fix the query so it parses.
 */
import { parseQuery } from "../console/parse.ts";
import { rowOf, matchesQuery } from "../console/queryMatch.ts";
import { UNGROUPED, type GraphMeta } from "../source/meta.ts";
import type { Group } from "../state/settings.ts";
import { MUTED } from "./palette.ts";
import { categoricalOf } from "./categorical.ts";
import type { Colouring } from "./colourBy.ts";

interface Parsed {
  readonly name: string;
  readonly colour: string;
  readonly query: ReturnType<typeof parseQuery>;
}

function parsedOf(groups: readonly Group[]): Parsed[] {
  const parsed: Parsed[] = [];
  for (const group of groups) {
    try {
      parsed.push({ name: group.name, colour: group.colour, query: parseQuery(group.query) });
    } catch {
      continue;
    }
  }
  return parsed;
}

/** The index of the first group matching `node`, or -1. */
function firstMatch(parsed: readonly Parsed[], meta: GraphMeta, node: number): number {
  const row = rowOf(meta, node);
  return parsed.findIndex((group) => matchesQuery(group.query, row));
}

function slotsOf(parsed: readonly Parsed[], meta: GraphMeta): Int32Array {
  const slots = new Int32Array(meta.nodeCount);
  for (let i = 0; i < slots.length; i += 1) slots[i] = firstMatch(parsed, meta, i);
  return slots;
}

/**
 * The document's own group column, used when no user group matches anything. One slot per
 * distinct group name, in node order: past ten the colours repeat but the legend rows do not.
 */
export function documentGroups(meta: GraphMeta): Colouring {
  return categoricalOf(Array.from(meta.group, (index) => meta.groups[index] ?? ""), { empty: UNGROUPED, overflow: "(other groups)" });
}

/** `by === "group"` with user groups: they win completely; unmatched nodes are MUTED. */
export function groupsOnly(meta: GraphMeta, groups: readonly Group[]): Colouring {
  const parsed = parsedOf(groups);
  const slots = slotsOf(parsed, meta);
  if (parsed.length === 0 || slots.every((slot) => slot < 0)) return documentGroups(meta);
  const colours = new Uint16Array(meta.nodeCount);
  slots.forEach((slot, i) => {
    colours[i] = slot < 0 ? parsed.length : slot;
  });
  const palette = parsed.map((group) => group.colour);
  const names = parsed.map((group) => group.name);
  if (slots.some((slot) => slot < 0)) {
    palette.push(MUTED);
    names.push("(no group)");
  }
  return { colours, palette, names: (slot) => names[slot] ?? `#${slot}` };
}

/** Group colours prepended to `base`. */
export function overlayGroups(base: Colouring, meta: GraphMeta, groups: readonly Group[]): Colouring {
  const parsed = parsedOf(groups);
  if (parsed.length === 0) return base;
  const offset = parsed.length;
  const slots = slotsOf(parsed, meta);
  const colours = new Uint16Array(meta.nodeCount);
  slots.forEach((slot, i) => {
    colours[i] = slot >= 0 ? slot : (base.colours[i] ?? 0) + offset;
  });
  return {
    colours,
    palette: [...parsed.map((group) => group.colour), ...base.palette],
    names: (slot) => (slot < offset ? (parsed[slot]?.name ?? `#${slot}`) : base.names(slot - offset)),
  };
}
