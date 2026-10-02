/**
 * Neighbour lists per node, built once per edge set instead of once per question. The order
 * is the one a full edge scan produced — first encounter, duplicates folded, self-loops
 * dropped — so a list built here and a list scanned there are the same array.
 */
import type { Ends } from "../source/meta.ts";

export interface Adjacency {
  /** The nodes one edge away, in edge order of first encounter; `[]` for a node with none. */
  at(node: number): readonly number[];
}

/** Shared so a node with no neighbours answers the same reference every time. */
const NONE: readonly number[] = Object.freeze([]);

/** `adjacencyOf` memoises here. Caveat: the key is the identity of `ends`, so an
 * equal-but-new `Ends` misses and is rebuilt, and an `ends` whose arrays are written
 * after the build keeps the list it was given. */
const CACHE = new WeakMap<Ends, Adjacency>();

function link(pending: Map<number, Set<number>>, node: number, other: number): void {
  const seen = pending.get(node);
  if (seen === undefined) pending.set(node, new Set([other]));
  else seen.add(other);
}

function listsOf(ends: Ends): Map<number, readonly number[]> {
  const pending = new Map<number, Set<number>>();
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    // A self-loop is no neighbour of itself in either direction, so it reaches no list.
    if (s === t) continue;
    link(pending, s, t);
    link(pending, t, s);
  }
  const lists = new Map<number, readonly number[]>();
  for (const [node, seen] of pending) lists.set(node, Object.freeze([...seen]));
  return lists;
}

/** Builds the lists without consulting the cache; for the tests and for a caller that holds an `Ends` it just built. */
export function buildAdjacency(ends: Ends): Adjacency {
  const lists = listsOf(ends);
  return { at: (node) => lists.get(node) ?? NONE };
}

export function adjacencyOf(ends: Ends): Adjacency {
  const held = CACHE.get(ends);
  if (held !== undefined) return held;
  const built = buildAdjacency(ends);
  CACHE.set(ends, built);
  return built;
}

export function neighboursOf(ends: Ends, node: number): readonly number[] {
  return adjacencyOf(ends).at(node);
}