/**
 * The single colour path for the studio.
 *
 * All metric colouring routes through graph-render's `normalise()` + `coloursOf()`
 * so the studio's colours are byte-identical to the engine's for the same data.
 * Categorical modes (group, kind, tag, db) assign one palette entry per distinct
 * key in first-seen order, bounded by MAX_KEYS = 512; overflow keys share one
 * MUTED slot.
 *
 * Groups override everything: when `groups` is non-empty, the base colouring
 * (from `by`) is computed first, then every node that matches a group's query
 * takes that group's colour. The group colours are PREPENDED to the base palette,
 * so a group gets its own slot and a node matching two groups takes the FIRST
 * group's. This precedence applies for every `by` value including "group".
 *
 * A group whose query `parseQuery` refuses is skipped (matches nothing) rather
 * than throwing — a stale group query blanks no nodes.
 *
 * Ponytail: MAX_KEYS bounds categorical palettes. Past 512 distinct keys the
 * extra keys collapse to one MUTED slot; the direction of error is fewer
 * colours than keys, and the escape hatch is to reduce the key space (e.g.
 * fewer tags) or use a metric colouring instead.
 *
 * Ponytail: a refused group query matches nothing rather than throwing. The
 * direction of error is a group silently matching no nodes; the escape hatch is
 * to fix the query so it parses.
 *
 * Ponytail: metric colouring with null or mismatched `values` falls back to a
 * flat MUTED palette named "nodes" rather than guessing. The direction of error
 * is a flat drawing; the escape hatch is to provide values of the correct length.
 */
import { normalise } from "../../../graph-render/src/colour/normalise.ts";
import { coloursOf } from "../../../graph-render/src/colour/colormap.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { Group } from "../state/settings.ts";
import type { COLOUR_BY } from "../state/settings.ts";
import { MUTED } from "./palette.ts";
import { categoricalOf } from "./categorical.ts";
import { documentGroups, groupsOnly, overlayGroups } from "./groupOverlay.ts";

export interface Colouring {
  readonly colours: Uint16Array;
  readonly palette: readonly string[];
  readonly names: (slot: number) => string;
}

export interface ColourInput {
  readonly meta: GraphMeta;
  readonly by: (typeof COLOUR_BY)[number];
  readonly values: Float64Array | Uint32Array | null;
  readonly groups: readonly Group[];
}

type Values = Float64Array | Uint32Array | null;

function flat(nodeCount: number): Colouring {
  return { colours: new Uint16Array(nodeCount), palette: [MUTED], names: () => "nodes" };
}

function metricColouring(nodeCount: number, values: Values): Colouring {
  if (values === null || values.length !== nodeCount) return flat(nodeCount);
  const { palette, slots } = coloursOf(normalise(values, { mode: "LINEAR", gamma: 1 }), "inferno");
  // Palette entries are numbered in first-seen order, so a slot is named by a node that wears it.
  // Ponytail: two values that quantise to one colour share the name of the first of them.
  const wearer = new Map<number, number>();
  slots.forEach((slot, node) => { if (!wearer.has(slot)) wearer.set(slot, node); });
  const names = (slot: number): string => String(values[wearer.get(slot) ?? 0] ?? 0);
  return { colours: slots, palette, names };
}

/** The base colouring for a `by` mode, before any group overlay. */
function baseColouring(meta: GraphMeta, by: ColourInput["by"], values: Values): Colouring {
  switch (by) {
    case "group":
      return documentGroups(meta);
    case "kind":
      return categoricalOf(meta.kinds, { empty: "", overflow: "(other kinds)" });
    case "tag":
      return categoricalOf(meta.tags.map((tags) => tags[0] ?? ""), { empty: "(no tag)", overflow: "(other tags)" });
    case "db":
      return categoricalOf(meta.dbs, { empty: "(no database)", overflow: "(other databases)" });
    case "analysis":
      return metricColouring(meta.nodeCount, values);
    default:
      return flat(meta.nodeCount);
  }
}

export function colouringOf(input: ColourInput): Colouring {
  const { meta, by, values, groups } = input;
  if (by === "group" && groups.length > 0) return groupsOnly(meta, groups);
  return overlayGroups(baseColouring(meta, by, values), meta, groups);
}
