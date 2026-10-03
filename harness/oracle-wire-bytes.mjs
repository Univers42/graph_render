// The binary face of the contract, read from docs/contract/binary-layout.md and checked
// against the bytes that document pins. Split out of harness/oracle-wire.mjs so that file
// stays under the 300-line house limit; oracle-diff.mjs calls it once per run.
//
// **A check on the contract, not a second writer.** Nothing here re-implements
// graph-contract's encoder: the document's own pinned byte table is the input, and what is
// asserted is that the document is internally consistent — that each section starts where
// the framing rules say it does, that the header fields sit at the offsets and carry the
// values the header table states, and that the node columns decode, in the order the
// contract's column table names, to the values the byte table prints. Every offset below is
// derived by decoding, not written down, so the framing rules are what is under test.
//
// So a divergence in the binary face — a magic that moved, a dim byte that stopped being
// the dimension, a column reordered, a padding rule that no longer pads to 4 — is caught
// here even though no writer is called. The spec-derived third-party reader
// (packages/graph-render/src/snapshot/decode.ts) is the independent implementation this
// stands in for.

import { readFileSync } from "node:fs";
import { join } from "node:path";

const CONTRACT = join("docs", "contract", "binary-layout.md");
/** `padding(len) = (4 - len % 4) % 4`, the contract's own formula (`binary.rs:195-197`). */
const paddingOf = (len) => (4 - len % 4) % 4;

/** Refuses unless every byte of the run is 0, the padding rule's only legal content. */
function checkZeroRun(bytes, at, len, label) {
  for (let i = 0; i < len; i += 1) {
    if (bytes[at + i] !== 0) throw new Error(`${label} byte ${at + i} is 0x${bytes[at + i].toString(16)}, padding must be 0`);
  }
}

/**
 * The CSR string table at `at`: `count + 1` u32 offsets, the UTF-8 run, then the pad.
 * Every rule the contract states about it is enforced, and the offset the next section
 * starts at is returned — derived by decoding, not written down.
 */
function decodeTable(bytes, at, count, label) {
  const offsets = [];
  for (let i = 0; i <= count; i += 1) offsets.push(bytes.readUInt32LE(at + i * 4));
  if (offsets[0] !== 0) throw new Error(`${label} offsets[0] is ${offsets[0]}, the contract says 0`);
  for (let i = 1; i <= count; i += 1) {
    if (offsets[i] < offsets[i - 1]) throw new Error(`${label} offsets[${i}] = ${offsets[i]} decreases from ${offsets[i - 1]}`);
  }
  const text = at + (count + 1) * 4;
  const pad = paddingOf(offsets[count]);
  if (offsets[count] + text + pad > bytes.length) throw new Error(`${label} claims ${offsets[count]} text bytes from offset ${text}, past the end`);
  checkZeroRun(bytes, text + offsets[count], pad, `${label} padding`);
  const strings = [];
  for (let i = 0; i < count; i += 1) strings.push(bytes.subarray(text + offsets[i], text + offsets[i + 1]).toString("utf8"));
  return { strings, textAt: text, next: text + offsets[count] + pad };
}

/** The 28-byte header, at the offsets and types the contract's header table states. */
function decodeHeader(bytes) {
  const magic = bytes.subarray(0, 4).toString("latin1");
  if (magic !== "GMSN") throw new Error(`magic is ${JSON.stringify(magic)}, the contract says "GMSN"`);
  const header = {
    major: bytes.readUInt32LE(4),
    minor: bytes.readUInt32LE(8),
    nodeTag: bytes.readUInt8(12),
    edgeTag: bytes.readUInt8(13),
    dim: bytes.readUInt8(14),
    stages: bytes.readUInt32LE(16),
    nodes: bytes.readUInt32LE(20),
    edges: bytes.readUInt32LE(24),
  };
  if (header.dim > 1) throw new Error(`dim byte 14 is ${header.dim}; the contract refuses 2..=255 as ReservedDim`);
  if (bytes.readUInt8(15) !== 0) throw new Error(`padding byte 15 is ${bytes.readUInt8(15)}, must be 0`);
  if (header.stages !== 1) throw new Error(`stage count is ${header.stages}, only 1 is accepted`);
  if (header.nodeTag > 2) throw new Error(`node geometry tag ${header.nodeTag} is Unknown`);
  if (header.edgeTag > 4) throw new Error(`edge geometry tag ${header.edgeTag} is Unknown`);
  return header;
}

/**
 * The node column order the contract's own table gives for this snapshot: `x`, `y` for a
 * Point at `dim = 0`, with a `z` between them at `dim = 1`. Read out of the table rather
 * than assumed, so a reordered column table changes what is decoded.
 */
function columnOrder(text, nodeTag, dim) {
  const row = text.split("\n").find((line) => line.startsWith(`| Point (\`${nodeTag}\`)`) || line.startsWith(`| Circle (\`${nodeTag}\`)`) || line.startsWith(`| Box (\`${nodeTag}\`)`));
  if (!row) throw new Error(`no row for node kind ${nodeTag} in the contract's node geometry table`);
  const cells = row.split("|").slice(1, -1).map((cell) => cell.trim());
  return [...cells[dim === 1 ? 2 : 1].matchAll(/`([a-z])`/g)].map(([, name]) => name);
}

/**
 * Checks the contract's own pinned bytes against the rules the same document states, and
 * returns what it decoded, for the gate record. Throws on the first inconsistency.
 *
 * @param root the repository root; the contract is read from `root/docs/contract/…`.
 */
export function checkBinaryContract(root) {
  const text = readFileSync(join(root, CONTRACT), "utf8");
  const start = text.indexOf("## The pinned 84-byte example");
  if (start < 0) throw new Error(`${CONTRACT}: no '## The pinned 84-byte example' section`);
  // The first table only: the section's second table (the Curve tail) numbers its offsets
  // "from 80" and is about a different snapshot.
  const section = text.slice(start);
  const firstTable = section.slice(section.indexOf("| offset |"), section.indexOf("\n\n", section.indexOf("| offset |")));
  const rows = [];
  for (const line of firstTable.split("\n")) {
    const cells = line.split("|").slice(1, -1).map((cell) => cell.trim().replace(/^`|`$/g, ""));
    if (cells.length < 4 || !/^\d+$/.test(cells[0]) || !/^[0-9A-F]{2}( [0-9A-F]{2})*$/.test(cells[1])) continue;
    const listed = cells[1].split(" ").map((b) => Number.parseInt(b, 16));
    if (String(listed.length) !== cells[2]) throw new Error(`the byte table lists ${cells[2]} bytes for ${cells[3]} but shows ${listed.length}`);
    const at = Number(cells[0]);
    for (const [i, byte] of listed.entries()) {
      if (rows[at + i] !== undefined && rows[at + i] !== byte) throw new Error(`the byte table states two different bytes at offset ${at + i}`);
      rows[at + i] = byte;
    }
  }
  if (rows[0] === undefined) throw new Error("the pinned example has no row at offset 0");
  for (const [at, byte] of rows.entries()) {
    if (byte === undefined) throw new Error(`the pinned example has a gap at byte ${at}`);
  }
  const bytes = Buffer.from(rows);
  const header = decodeHeader(bytes);
  const nodes = decodeTable(bytes, 28, header.nodes, "node.id table");
  const edges = decodeTable(bytes, nodes.next, header.edges, "edge.id table");
  const endpoints = Array.from({ length: header.edges * 2 }, (_, i) => bytes.readUInt32LE(edges.next + i * 4));
  for (const [i, position] of endpoints.entries()) {
    if (position >= header.nodes) throw new Error(`edge.${i % 2 === 0 ? "source" : "target"}[${Math.floor(i / 2)}] = ${position} is not a position in the ${header.nodes}-entry node id table`);
  }
  const columnsAt = edges.next + header.edges * 8;
  const names = columnOrder(text, header.nodeTag, header.dim);
  const columns = Object.fromEntries(names.map((name, i) => [name, Array.from({ length: header.nodes }, (_, k) => bytes.readFloatLE(columnsAt + (i * header.nodes + k) * 4))]));
  if (JSON.stringify(columns.x) !== "[1,-2.5]" || JSON.stringify(columns.y) !== "[0,0.5]") {
    throw new Error(`the node columns decode to x=${JSON.stringify(columns.x)} y=${JSON.stringify(columns.y)}; the byte table pins node.x=[1,-2.5] node.y=[0,0.5]`);
  }
  const notesAt = columnsAt + names.length * header.nodes * 4;
  const notes = bytes.readUInt32LE(notesAt);
  if (notes !== 0) throw new Error(`note.count is ${notes}, the pinned example carries none`);
  if (notesAt + 4 !== bytes.length) throw new Error(`the pinned example is ${bytes.length} bytes; the header and its sections account for ${notesAt + 4}`);
  return {
    snapshot: `${bytes.length}-byte pinned example`,
    magic: "GMSN",
    format: `${header.major}.${header.minor}`,
    dim: header.dim,
    nodes: header.nodes,
    edges: header.edges,
    nodeIds: nodes.strings,
    edgeIds: edges.strings,
    columns: names,
    source: endpoints.filter((_, i) => i % 2 === 0),
    target: endpoints.filter((_, i) => i % 2 === 1),
    x: columns.x,
    y: columns.y,
    notes,
    bytes: bytes.length,
  };
}