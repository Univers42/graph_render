// H9 for the oracle differential (docs/decisions/h9-group-width.md).
//
// layoutGroups is not a function the oracle exports: it is the body of
// LayoutController.rebuild (src/core/layout/layoutBridge.ts:76-87), transcribed here and
// run over the model as rebuild sees it — indexModel's de-duplicated nodes, with idList
// their ids. The transcription is refused unless those lines are still one contiguous block
// *of rebuild itself*, verbatim: searching the whole file proved only that the text exists
// somewhere, so moving the block into a dead method, re-indented, left the guard passing
// while the code that actually runs no longer contained it.

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

/**
 * The lines of `rebuild` in layoutBridge.ts: from its signature line to the line that closes
 * the brace it opened. Throws unless the method is found and closed, so a rename or a
 * reformat that moves the body is a refusal and not an empty search space.
 */
function rebuildLines(lines) {
  const open = lines.findIndex((line) => /^\s*rebuild\s*\(/.test(line));
  if (open < 0) throw new Error("H9 transcription: layoutBridge.ts has no rebuild method");
  let depth = 0;
  for (let at = open; at < lines.length; at += 1) {
    depth += (lines[at].match(/\{/g) ?? []).length - (lines[at].match(/\}/g) ?? []).length;
    if (depth === 0 && at > open) return lines.slice(open, at + 1);
  }
  throw new Error(`H9 transcription: rebuild at line ${open + 1} is never closed`);
}

/**
 * Throws unless H9_SOURCE is one run of consecutive lines inside `rebuild` itself — anchored
 * to the method and to the brace-balanced range it spans, not to the file.
 */
export function checkTranscription(root) {
  const path = join(root, "src", "core", "layout", "layoutBridge.ts");
  const body = rebuildLines(readFileSync(path, "utf8").split("\n")).map((line) => line.trim());
  const found = body.some((_, at) => H9_SOURCE.every((line, k) => body[at + k] === line));
  if (!found) throw new Error("H9 transcription: rebuild no longer holds the twelve lines as one block");
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

/**
 * The transcribed loop, once. `wide` is the H9 decision's own width: the oracle stores the
 * group in a `Uint8Array` and masks (`g & 0xff`), graph-core stores the unmasked index. One
 * body, two widths — a second copy of the loop would drift from the twelve verified lines
 * silently, which is what `assertAgree` below exists to catch.
 */
function groupLoop(model, wide) {
  const idList = model.nodes.map((node) => node.id);
  const nodeGroups = wide ? new Array(idList.length).fill(0) : new Uint8Array(idList.length);
  const sourceIndex = new Map();
  for (let i = 0; i < model.nodes.length; i += 1) {
    const source = model.nodes[i].source;
    if (source == null) continue;
    let g = sourceIndex.get(source);
    if (g === undefined) {
      g = sourceIndex.size;
      sourceIndex.set(source, g);
    }
    nodeGroups[i] = wide ? g : g & 0xff;
  }
  return [...nodeGroups];
}

/** layoutBridge.ts:76-87, verbatim but for `this.idList`, which rebuild sets to the ids of `model.nodes`. */
export const layoutGroups = (model) => groupLoop(model, false);

/** The same loop with neither the byte store nor the mask: what the H9 decision says graph-core stores. */
export const widenedGroups = (model) => groupLoop(model, true);

/**
 * A layoutGroups mismatch is H9 only if graph-core's line is exactly the widened groups
 * and the oracle's exactly those & 0xff. A wrong group that happens to agree in its low
 * byte (300 where 44 is right) is not H9.
 */
export function h9Explains(wide, got, want) {
  return want === canonical(wide) && got === canonical(wide.map((g) => g & 0xff));
}
