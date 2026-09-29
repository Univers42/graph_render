/**
 * The filter document as the two masks the painter already reads: which nodes are
 * hidden, and which the search lit up. The semantics of `text`, `hiddenGroups` and
 * `minDegree` are the ones `look/styleOf.ts` already had — any criterion hides the node
 * — and `query`, `hiddenKinds`, `orphans` and `existingOnly` join them here.
 *
 * `orphans` and `existingOnly` are the same set of nodes, read two ways: both hide a
 * node with no link, and they are separate members because a recipe says which of the
 * two the user meant. `relayout` is not a mask: it tells the layout to run again, and
 * is deliberately not read here.
 *
 * `null` means the mask is empty, so the renderer can skip the pass altogether: a
 * filter that would hide nothing and a filter that is not filtering are the same array.
 */
import { parseQuery } from "../console/parse.ts";
import type { Query } from "../console/parse.ts";
import { matchesQuery, rowOf } from "../console/queryMatch.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { Filter } from "../state/settings.ts";

interface Criteria {
  readonly text: string;
  readonly groups: ReadonlySet<string>;
  readonly kinds: ReadonlySet<string>;
  readonly minDegree: number;
  /** `orphans` and `existingOnly` are one predicate; both members are asked at once. */
  readonly unlinked: boolean;
  /** `null` when the filter names no query, or names one that does not parse. */
  readonly query: Query | null;
}

/** The empty text, as the substring test wants it. */
const NO_TEXT = "";

// Ponytail: a query that does not parse is dropped rather than refused. Failing input
// `kind:tag AND`: the studio would blank the canvas and show no reason for it, which
// reads as a broken graph rather than a broken line. Direction it errs: the filter is
// ignored, so more is drawn than the user asked for. Escape hatch: the console reports
// the refusal beside the line, which is where the user is already looking.
function queryOf(text: string): Query | null {
  if (text.trim() === "") return null;
  try {
    return parseQuery(text);
  } catch {
    return null;
  }
}

/** `null` when no criterion applies at all: that is the `null` mask. */
function criteriaOf(filter: Filter): Criteria | null {
  const text = filter.text.trim().toLowerCase();
  const query = queryOf(filter.query);
  const unlinked = filter.orphans || filter.existingOnly;
  const quiet = filter.hiddenGroups.length === 0 && filter.hiddenKinds.length === 0 && filter.minDegree === 0;
  if (query === null && !unlinked && quiet && text === NO_TEXT) return null;
  return {
    text,
    groups: new Set(filter.hiddenGroups),
    kinds: new Set(filter.hiddenKinds),
    minDegree: filter.minDegree,
    unlinked,
    query,
  };
}

function hides(criteria: Criteria, meta: GraphMeta, node: number): boolean {
  const row = rowOf(meta, node);
  const unnamed = criteria.text !== NO_TEXT && !row.label.toLowerCase().includes(criteria.text);
  const grouped = criteria.groups.has(meta.groups[meta.group[node] ?? 0] ?? "");
  const kinded = criteria.kinds.has(row.kind);
  const quiet = row.degree < criteria.minDegree;
  const alone = criteria.unlinked && row.degree === 0;
  const queried = criteria.query !== null && !matchesQuery(criteria.query, row);
  return unnamed || grouped || kinded || quiet || alone || queried;
}

/** 1 per hidden node, or `null` when the filter hides nothing at all. */
export function hiddenOf(meta: GraphMeta, filter: Filter): Uint8Array | null {
  const criteria = criteriaOf(filter);
  if (criteria === null) return null;
  const hidden = new Uint8Array(meta.nodeCount);
  for (let node = 0; node < meta.nodeCount; node += 1) {
    if (hides(criteria, meta, node)) hidden[node] = 1;
  }
  return hidden;
}

/** 1 per node the search matches, or `null` when there is no search. */
export function highlightOf(meta: GraphMeta, filter: Filter): Uint8Array | null {
  const query = queryOf(filter.query);
  if (query !== null) return matchedOf(meta, query);
  const text = filter.text.trim();
  if (text === NO_TEXT) return null;
  return matchedOf(meta, { kind: "text", text });
}

function matchedOf(meta: GraphMeta, query: Query): Uint8Array {
  const lit = new Uint8Array(meta.nodeCount);
  for (let node = 0; node < meta.nodeCount; node += 1) {
    if (matchesQuery(query, rowOf(meta, node))) lit[node] = 1;
  }
  return lit;
}
