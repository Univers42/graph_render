// H9 for the oracle differential (docs/decisions/h9-group-width.md).
//
// layoutGroups is not a function the oracle exports: it is the body of
// LayoutController.rebuild (src/core/layout/layoutBridge.ts:76-87), transcribed here and
// run over the model as rebuild sees it — indexModel's de-duplicated nodes, with idList
// their ids. The transcription is refused unless its lines are still one contiguous block
// of that file, verbatim.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { canonical } from "./oracle-wire.mjs";

const H9_SOURCE = [
  "const nodeGroups = new Uint8Array(this.idList.length);",
  "const sourceIndex = new Map<string, number>();",
  "for (let i = 0; i < model.nodes.length; i += 1) {",
  "const source = model.nodes[i].source;",
  "if (source == null) continue;",
  "let g = sourceIndex.get(source);",
  "if (g === undefined) {",
  "g = sourceIndex.size;",
  "sourceIndex.set(source, g);",
  "}",
  "nodeGroups[i] = g & 0xff;",
  "}",
];

/** Throws unless H9_SOURCE is one run of consecutive lines of layoutBridge.ts. */
export function checkTranscription(root) {
  const path = join(root, "src", "core", "layout", "layoutBridge.ts");
  const lines = readFileSync(path, "utf8")
    .split("\n")
    .map((line) => line.trim());
  const found = lines.some((_, at) => H9_SOURCE.every((line, k) => lines[at + k] === line));
  if (!found) throw new Error("H9 transcription: layoutBridge.ts no longer holds its lines as one block");
}

/** The model rebuild receives, from fixture nodes that carry only an id and a source. */
export function groupModel(oracle, args) {
  const nodes = args.nodes.map(({ id, source }) => ({
    id,
    kind: "record",
    databaseId: null,
    source,
    label: "",
    group: null,
    weight: 0.5,
    version: 0,
    hasNote: false,
  }));
  return oracle.indexModel(nodes, []);
}

/** layoutBridge.ts:76-87, verbatim but for `this.idList`, which rebuild sets to the ids of `model.nodes`. */
export function layoutGroups(model) {
  const idList = model.nodes.map((node) => node.id);
  const nodeGroups = new Uint8Array(idList.length);
  const sourceIndex = new Map();
  for (let i = 0; i < model.nodes.length; i += 1) {
    const source = model.nodes[i].source;
    if (source == null) continue;
    let g = sourceIndex.get(source);
    if (g === undefined) {
      g = sourceIndex.size;
      sourceIndex.set(source, g);
    }
    nodeGroups[i] = g & 0xff;
  }
  return [...nodeGroups];
}

/** The same loop with neither the byte store nor the mask: what the H9 decision says graph-core stores. */
export function widenedGroups(model) {
  const nodeGroups = new Array(model.nodes.length).fill(0);
  const sourceIndex = new Map();
  for (let i = 0; i < model.nodes.length; i += 1) {
    const source = model.nodes[i].source;
    if (source == null) continue;
    let g = sourceIndex.get(source);
    if (g === undefined) {
      g = sourceIndex.size;
      sourceIndex.set(source, g);
    }
    nodeGroups[i] = g;
  }
  return nodeGroups;
}

/**
 * A layoutGroups mismatch is H9 only if graph-core's line is exactly the widened groups
 * and the oracle's exactly those & 0xff. A wrong group that happens to agree in its low
 * byte (300 where 44 is right) is not H9.
 */
export function h9Explains(wide, got, want) {
  return want === canonical(wide) && got === canonical(wide.map((g) => g & 0xff));
}
