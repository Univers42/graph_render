/**
 * Tests for colourBy.ts — the single colour path through the look library.
 * These tests pin the exact behaviour so any change to the colormap library
 * or the colouring logic must fail here.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import { normalise } from "../../graph-render/src/colour/normalise.ts";
import { coloursOf } from "../../graph-render/src/colour/colormap.ts";
import { colouringOf, type ColourInput } from "../src/look/colourBy.ts";
import type { GraphMeta } from "../src/source/meta.ts";
import type { Group } from "../src/state/settings.ts";
import { GROUP_PALETTE, MUTED } from "../src/look/palette.ts";

function makeMeta(overrides: Partial<GraphMeta> = {}): GraphMeta {
  const nodeCount = overrides.nodeCount ?? 6;
  return {
    nodeCount,
    ids: overrides.ids ?? Array.from({ length: nodeCount }, (_, i) => `n${i}`),
    labels: overrides.labels ?? Array.from({ length: nodeCount }, (_, i) => `Node ${i}`),
    kinds: overrides.kinds ?? (["record", "note", "database", "tag", "record", "note"] as const).slice(0, nodeCount),
    groups: overrides.groups ?? ["G1", "G2"],
    group: overrides.group ?? new Uint16Array([0, 1, 0, 1, 0, 1]).slice(0, nodeCount),
    weight: overrides.weight ?? new Float32Array(nodeCount).fill(0.5), versions: overrides.versions ?? new Float64Array(nodeCount),
    degree: overrides.degree ?? new Uint32Array(nodeCount).fill(1),
    maxDegree: overrides.maxDegree ?? 1,
    tags: overrides.tags ?? Array.from({ length: nodeCount }, () => []),
    dbs: overrides.dbs ?? Array.from({ length: nodeCount }, () => ""),
    paths: overrides.paths ?? Array.from({ length: nodeCount }, () => ""),
    ...overrides,
  };
}

function makeInput(
  meta: GraphMeta,
  by: ColourInput["by"],
  values: Float64Array | Uint32Array | null = null,
  groups: readonly Group[] = [],
): ColourInput {
  return { meta, by, values, groups };
}

test("by=group: groups non-empty wins, first-match order, unmatched gets MUTED", () => {
  const meta = makeMeta({
    nodeCount: 4,
    kinds: ["record", "note", "record", "note"],
    groups: ["A", "B"],
    group: new Uint16Array([0, 1, 0, 1]),
  });
  const groups: readonly Group[] = [
    { name: "G1", query: "kind:record", colour: "#ff0000" },
    { name: "G2", query: "kind:note", colour: "#00ff00" },
  ];
  const input = makeInput(meta, "group", null, groups);
  const c = colouringOf(input);

  assert.deepEqual(c.colours, new Uint16Array([0, 1, 0, 1]));
  assert.deepEqual(c.palette, ["#ff0000", "#00ff00"]);
  assert.equal(c.names(0), "G1");
  assert.equal(c.names(1), "G2");
});

test("by=group: groups empty falls back to document group column", () => {
  const meta = makeMeta({ nodeCount: 4, groups: ["A", "B"], group: new Uint16Array([0, 1, 0, 1]) });
  const input = makeInput(meta, "group", null, []);
  const c = colouringOf(input);

  assert.deepEqual(c.colours, new Uint16Array([0, 1, 0, 1]));
  assert.deepEqual(c.palette.slice(0, 2), [GROUP_PALETTE[0], GROUP_PALETTE[1]]);
  assert.equal(c.names(0), "A");
  assert.equal(c.names(1), "B");
});

test("by=group: one slot per group, so group 11 does not share group 1's colour", () => {
  const meta = makeMeta({
    nodeCount: 12,
    kinds: Array.from({ length: 12 }, () => "record" as const),
    groups: Array.from({ length: 12 }, (_, i) => `G${i}`),
    group: Uint16Array.from(Array.from({ length: 12 }, (_, i) => i)),
  });
  const c = colouringOf(makeInput(meta, "group", null, []));

  // Past the ten colours of GROUP_PALETTE the slots keep counting: 10 and 11 are their own.
  assert.deepEqual(c.colours, Uint16Array.from(Array.from({ length: 12 }, (_, i) => i)));
  assert.equal(c.names(10), "G10");
  assert.equal(c.names(11), "G11");
});

test("by=group: group query refused is skipped", () => {
  const meta = makeMeta({ nodeCount: 2, kinds: ["record", "note"] });
  const groups: readonly Group[] = [
    { name: "Bad", query: "degree:>x", colour: "#ff0000" }, // refused (needs a whole number)
    { name: "Good", query: "kind:note", colour: "#00ff00" },
  ];
  const input = makeInput(meta, "group", null, groups);
  const c = colouringOf(input);

  assert.deepEqual(c.colours, new Uint16Array([1, 0]));
  assert.deepEqual(c.palette, ["#00ff00", MUTED]);
  assert.equal(c.names(0), "Good");
  assert.equal(c.names(1), "(no group)");
});

test("by=kind: one colour per kind, first-seen order", () => {
  const meta = makeMeta({ nodeCount: 4, kinds: ["record", "note", "record", "database"] });
  const input = makeInput(meta, "kind", null, []);
  const c = colouringOf(input);

  assert.deepEqual(c.colours, new Uint16Array([0, 1, 0, 2]));
  assert.equal(c.names(0), "record");
  assert.equal(c.names(1), "note");
  assert.equal(c.names(2), "database");
});

test("by=tag: first tag is key, empty gets MUTED named (no tag)", () => {
  const meta = makeMeta({
    nodeCount: 4,
    tags: [["a", "b"], ["a"], [], ["c"]],
  });
  const input = makeInput(meta, "tag", null, []);
  const c = colouringOf(input);

  // First tags: "a", "a", "", "c" -> distinct keys in first-seen order: "a", "", "c"
  // But "" gets MUTED and name "(no tag)"
  assert.deepEqual(c.colours, new Uint16Array([0, 0, 1, 2]));
  assert.equal(c.names(0), "a");
  assert.equal(c.names(1), "(no tag)");
  assert.equal(c.names(2), "c");
  assert.deepEqual(c.palette[1], MUTED);
});

test("by=db: database is key, empty gets MUTED named (no database)", () => {
  const meta = makeMeta({
    nodeCount: 4,
    dbs: ["db1", "db1", "", "db2"],
  });
  const input = makeInput(meta, "db", null, []);
  const c = colouringOf(input);

  // Distinct keys: "db1", "", "db2" -> first-seen order
  assert.deepEqual(c.colours, new Uint16Array([0, 0, 1, 2]));
  assert.equal(c.names(0), "db1");
  assert.equal(c.names(1), "(no database)");
  assert.equal(c.names(2), "db2");
  assert.deepEqual(c.palette[1], MUTED);
});

test("by=analysis: metric colouring uses normalise + coloursOf with inferno, LINEAR, gamma=1", () => {
  const meta = makeMeta({ nodeCount: 5 });
  const values = new Float64Array([10, 20, 30, 40, 50]);
  const input = makeInput(meta, "analysis", values, []);
  const c = colouringOf(input);

  // Compute expected using the library directly
  const norm = normalise(values, { mode: "LINEAR", gamma: 1 });
  const expected = coloursOf(norm, "inferno");

  // Palette and slots must be BYTE-IDENTICAL
  assert.deepEqual(c.palette, expected.palette);
  assert.deepEqual(c.colours, expected.slots);
  // names(0) = min value, names(last) = max value
  assert.equal(c.names(0), "10");
  assert.equal(c.names(expected.palette.length - 1), "50");
});

test("by=analysis: values null or length mismatch -> all MUTED named 'nodes'", () => {
  const meta = makeMeta({ nodeCount: 3 });

  // null values
  let input = makeInput(meta, "analysis", null, []);
  let c = colouringOf(input);
  assert.deepEqual(c.colours, new Uint16Array([0, 0, 0]));
  assert.deepEqual(c.palette, [MUTED]);
  assert.equal(c.names(0), "nodes");

  // length mismatch
  input = makeInput(meta, "analysis", new Float64Array([1, 2]), []);
  c = colouringOf(input);
  assert.deepEqual(c.colours, new Uint16Array([0, 0, 0]));
  assert.deepEqual(c.palette, [MUTED]);
  assert.equal(c.names(0), "nodes");
});

test("by=analysis: equal values get equal colours, different values different when colormap distinguishes", () => {
  const meta = makeMeta({ nodeCount: 4 });
  const values = new Float64Array([10, 10, 20, 20]);
  const input = makeInput(meta, "analysis", values, []);
  const c = colouringOf(input);

  // First two equal, last two equal but different from first
  assert.equal(c.colours[0], c.colours[1]);
  assert.equal(c.colours[2], c.colours[3]);
  assert.notEqual(c.colours[0], c.colours[2]);
});

test("groups override metric: group colours prepended to base palette", () => {
  const meta = makeMeta({ nodeCount: 4, kinds: ["record", "note", "record", "note"] });
  const values = new Float64Array([10, 20, 30, 40]);
  const groups: readonly Group[] = [
    { name: "G1", query: "kind:record", colour: "#ff0000" },
  ];
  const input = makeInput(meta, "analysis", values, groups);
  const c = colouringOf(input);

  // Group colours are prepended: #ff0000 at slot 0, then metric palette starts at slot 1
  assert.equal(c.colours[0], 0);
  assert.equal(c.colours[2], 0);
  assert.deepEqual(c.palette[0], "#ff0000");
  assert.equal(c.names(0), "G1");

  // The metric part should match what coloursOf would give, just shifted
  const norm = normalise(values, { mode: "LINEAR", gamma: 1 });
  const expected = coloursOf(norm, "inferno");
  // Metric palette should appear after group palette
  for (let i = 0; i < expected.palette.length; i++) {
    assert.deepEqual(c.palette[i + 1], expected.palette[i]);
  }
});

test("groups override kind: group colours prepended to kind palette", () => {
  const meta = makeMeta({ nodeCount: 3, kinds: ["record", "note", "record"] });
  const groups: readonly Group[] = [
    { name: "G1", query: "kind:note", colour: "#00ff00" },
  ];
  const input = makeInput(meta, "kind", null, groups);
  const c = colouringOf(input);

  assert.equal(c.colours[1], 0);
  assert.deepEqual(c.palette[0], "#00ff00");
  assert.equal(c.names(0), "G1");
  // record should be at slot 1 (first kind, shifted)
  assert.equal(c.colours[0], 1);
  assert.equal(c.colours[2], 1);
});

test("MAX_KEYS = 512: categorical mode past 512 distinct keys shares one MUTED slot", () => {
  const nodeCount = 515;
  const meta = makeMeta({
    nodeCount,
    tags: Array.from({ length: nodeCount }, (_, i) => [`tag${i}`]),
  });
  const input = makeInput(meta, "tag", null, []);
  const c = colouringOf(input);

  // First 512 distinct tags get their own slots, the rest share the overflow slot
  const distinctKeys = new Set(c.colours.slice(0, 512));
  assert.equal(distinctKeys.size, 512);

  // The overflow keys (512, 513, 514) should all have the same slot
  const overflowSlot = c.colours[512] ?? -1;
  assert.equal(c.colours[513], overflowSlot);
  assert.equal(c.colours[514], overflowSlot);
  assert.deepEqual(c.palette[overflowSlot], MUTED);
  assert.equal(c.names(overflowSlot), "(other tags)");
});

test("by=group: past MAX_KEYS = 512 groups the rest share one MUTED slot named (other groups)", () => {
  const nodeCount = 514;
  const meta = makeMeta({
    nodeCount,
    kinds: Array.from({ length: nodeCount }, () => "record" as const),
    groups: Array.from({ length: nodeCount }, (_, i) => `G${i}`),
    group: Uint16Array.from(Array.from({ length: nodeCount }, (_, i) => i)),
  });
  const c = colouringOf(makeInput(meta, "group", null, []));

  const overflowSlot = c.colours[512] ?? -1;
  assert.equal(overflowSlot, 512);
  assert.equal(c.colours[513], overflowSlot);
  assert.equal(c.names(overflowSlot), "(other groups)");
  assert.deepEqual(c.palette[overflowSlot], MUTED);
});

test("first-seen order: palette order is first-seen in single pass over node indices", () => {
  const meta = makeMeta({
    nodeCount: 4,
    tags: [["b"], ["a"], ["b"], ["a"]], // first-seen: "b", "a"
  });
  const input = makeInput(meta, "tag", null, []);
  const c = colouringOf(input);

  assert.equal(c.names(0), "b");
  assert.equal(c.names(1), "a");
  assert.equal(c.colours[0], 0); // b
  assert.equal(c.colours[1], 1); // a
  assert.equal(c.colours[2], 0); // b
  assert.equal(c.colours[3], 1); // a
});

test("two groups: node matching both takes first group's colour", () => {
  const meta = makeMeta({ nodeCount: 2, kinds: ["record", "record"] });
  const groups: readonly Group[] = [
    { name: "First", query: "kind:record", colour: "#111111" },
    { name: "Second", query: "kind:record", colour: "#222222" },
  ];
  const input = makeInput(meta, "kind", null, groups);
  const c = colouringOf(input);

  // Both nodes match both queries, but first group wins
  assert.deepEqual(c.colours, new Uint16Array([0, 0]));
  assert.deepEqual(c.palette[0], "#111111");
  assert.equal(c.names(0), "First");
  // Second group should still be in palette at slot 1
  assert.deepEqual(c.palette[1], "#222222");
  assert.equal(c.names(1), "Second");
});

test("groups non-empty but no node matches: base colouring unchanged", () => {
  const meta = makeMeta({ nodeCount: 2, kinds: ["record", "note"] });
  const groups: readonly Group[] = [
    { name: "G1", query: "kind:database", colour: "#ff0000" }, // matches nothing
  ];
  const input = makeInput(meta, "kind", null, groups);
  const c = colouringOf(input);

  // No matches, so base kind colouring should be unchanged (just with group palette prepended but unused)
  // record -> slot 1 (0 is G1), note -> slot 2
  assert.equal(c.colours[0], 1);
  assert.equal(c.colours[1], 2);
  assert.equal(c.names(1), "record");
  assert.equal(c.names(2), "note");
});

test("by=none: all MUTED", () => {
  const meta = makeMeta({ nodeCount: 3 });
  const input = makeInput(meta, "none", null, []);
  const c = colouringOf(input);

  assert.deepEqual(c.colours, new Uint16Array([0, 0, 0]));
  assert.deepEqual(c.palette, [MUTED]);
  assert.equal(c.names(0), "nodes");
});