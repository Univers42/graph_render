// The universal-contract promise, proved: a snapshot's JSON face is readable with **no
// SDK at all**, in any language, because it is documented JSON with a committed schema.
//
// This file is that proof. It imports nothing from `crates/graph-sdk-js` — no wasm, no
// column views, no `Motor` — and reads a real snapshot with `JSON.parse` and nothing
// else. Everything it knows about the format it learned from
// `docs/contract/snapshot-schema.json` and from `docs/contract/binary-layout.md`
// §"The JSON face". If a change ever makes the JSON face unreadable without the SDK,
// this file stops working, and that is the point.
//
// It checks the schema against the snapshot it reads, too: every `$ref` resolves inside
// the committed schema, every object in it — the branches under a `oneOf` included —
// refuses an unknown member, and the snapshot carries every member the schema requires.
// Those are the three ways a committed schema rots without anybody noticing.
//
//   node harness/read-snapshot-raw.mjs <snapshot.json>     a file graph-cli wrote
//   node harness/read-snapshot-raw.mjs --selftest         the worked example below
//   node harness/read-snapshot-raw.mjs --schema <path> <snapshot.json>   another schema
//
// A snapshot path is required unless `--selftest` is given; with neither, and with no
// way to guess which was meant, the file refuses with exit 2 and this usage. Falling
// back to the example silently is how a real snapshot stops being checked at all, so
// the example is reachable only when it is asked for by name. The gate row
// `snapshot-raw` passes the artifact `graph-cli snapshot` actually writes, so drift on
// the producer's side of the JSON face lands here; the example is a stranger's-eye
// sample of the same bytes, not a substitute for the thing itself.
//
// `--schema` reads a different schema document; the default is the committed one, and
// no argument makes the check weaker.
//
// Exit codes follow graph-cli: 0 pass, 1 ran and failed, 2 could not run.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");
const COMMITTED_SCHEMA = join(ROOT, "docs", "contract", "snapshot-schema.json");

/// A real snapshot, verbatim: four nodes, five edges, laid out by `layout.grid`, the
/// same bytes `graph-cli snapshot --seed 0 --nodes 4 --layout layout.grid --out-json`
/// writes. `--selftest` only.
const WORKED_EXAMPLE = `{"edges":{"id":["bench-e-0","bench-e-1","bench-e-2","bench-e-3","bench-e-4"],"source":["bench:db-1:1","bench:db-2:2","bench:db-2:2","bench:db-3:3","bench:db-3:3"],"target":["bench:db-0:0","bench:db-0:0","bench:db-0:0","bench:db-2:2","bench:db-0:0"]},"geometry":{"edges":{"kind":"Line"},"nodes":{"kind":"Point","x":[-0.5,0.5,-0.5,0.5],"y":[-0.5,-0.5,0.5,0.5]}},"nodes":{"id":["bench:db-0:0","bench:db-1:1","bench:db-2:2","bench:db-3:3"]},"notes":{"code":[],"index":[]},"version":{"major":0,"minor":3}}`;

const USAGE = [
  "usage: read-snapshot-raw.mjs <snapshot.json>",
  "       read-snapshot-raw.mjs --selftest",
  "       read-snapshot-raw.mjs [--schema <schema.json>] <snapshot.json>",
].join("\n");

/** Refuse to run: the reason, then the usage. Exit 2, as the header promises. */
function refuse(reason) {
  process.stderr.write(`read-snapshot-raw: ${reason}\n${USAGE}\n`);
  process.exit(2);
}

/** A JSON document, or exit 2 with the reason. A missing, unreadable or unparseable
 *  input is a refusal to run, never a stack trace and never a silent default. */
async function readJson(path, what) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (err) {
    process.stderr.write(`read-snapshot-raw: could not read ${what} ${path}: ${err.message}\n`);
    process.exit(2);
  }
}

let failures = 0;
function check(name, condition, detail = "") {
  if (condition) {
    process.stdout.write(`ok - ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`not ok - ${name}${detail ? `\n#   ${detail}` : ""}\n`);
  }
}

/** Every `$ref` a JSON Schema uses. A `$ref` that does not resolve makes the schema
 *  unable to validate anything, and that is invisible until something tries. */
function refsIn(value, out = []) {
  if (Array.isArray(value)) {
    value.forEach((v) => refsIn(v, out));
  } else if (value !== null && typeof value === "object") {
    for (const [key, inner] of Object.entries(value)) {
      if (key === "$ref" && typeof inner === "string") out.push(inner);
      else refsIn(inner, out);
    }
  }
  return out;
}

/** The object shape the schema states for a member, following its `$ref`. */
function shapeOf(schema, ref) {
  const def = ref.startsWith("#/$defs/") ? schema.$defs?.[ref.slice(8)] : undefined;
  if (def === undefined || (def.type !== "object" && def.properties === undefined)) return null;
  return { required: def.required ?? [], named: Object.keys(def.properties ?? {}), closed: def.additionalProperties === false };
}

/** The path that reaches a subschema, named by the `kind` it declares where it has one:
 *  index 0 of `EdgeGeometry.oneOf` reads as `Line`, which is what a reader knows. */
function subschemaPath(node, path) {
  const kind = Array.isArray(node) ? undefined : node?.properties?.kind?.const;
  return typeof kind === "string" ? `${path}/${kind}` : path;
}

/** Every object-shaped subschema that does not refuse an unknown member, by the path
 *  that reaches it. `NodeGeometry` and `EdgeGeometry` carry no `type` at all — they are
 *  `oneOf` — and their six branches are where the objects are, so the walk steps into
 *  `oneOf`, `anyOf` and `allOf` (and everything else) rather than filtering on `type`. */
function openObjects(node, path = "#", out = []) {
  if (Array.isArray(node)) {
    node.forEach((v, i) => openObjects(v, subschemaPath(v, `${path}[${i}]`), out));
  } else if (node !== null && typeof node === "object") {
    const shaped = node.type === "object" || node.properties !== undefined;
    if (shaped && node.additionalProperties !== false) out.push(path);
    for (const [key, inner] of Object.entries(node)) {
      openObjects(inner, subschemaPath(inner, `${path}/${key}`), out);
    }
  }
  return out;
}

function checkObject(schema, ref, value, path, problems) {
  const shape = shapeOf(schema, ref);
  if (shape === null) {
    // A def the walk meant to read and cannot is a hole, not a pass: say so.
    problems.push(`${path}: the schema states no object shape for ${ref}`);
    return;
  }
  // An absent member is every required member missing, not a TypeError from `in`.
  const present = value !== null && typeof value === "object";
  for (const member of shape.required) {
    if (!present || !(member in value)) problems.push(`${path}: missing ${member}`);
  }
  if (shape.closed && present) {
    for (const key of Object.keys(value)) {
      if (!shape.named.includes(key)) problems.push(`${path}: unknown member ${key}`);
    }
  }
}

/** `lo, hi` of a column by iteration. Never `Math.min(...values)`: a snapshot has more
 *  values in a column than a call takes arguments, and the RangeError that follows
 *  replaces the summary with a stack trace. */
function extent(values) {
  let lo = Infinity;
  let hi = -Infinity;
  for (const value of values) {
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  return `${lo}, ${hi}`;
}

// --- what to read, from the arguments alone --------------------------------------------

const argv = process.argv.slice(2);
let selftest = false;
let schemaArg = null;
let snapshotArg = null;
for (let i = 0; i < argv.length; i += 1) {
  const arg = argv[i];
  if (arg === "--selftest") selftest = true;
  else if (arg === "--schema") {
    if (argv[i + 1] === undefined) refuse("--schema needs a path");
    schemaArg = argv[i + 1];
    i += 1;
  } else if (arg.startsWith("-")) refuse(`unknown argument '${arg}'`);
  else if (snapshotArg === null) snapshotArg = arg;
  else refuse(`more than one snapshot given ('${snapshotArg}' and '${arg}')`);
}
if (selftest && snapshotArg !== null) refuse("--selftest reads the worked example, not a file");
if (!selftest && snapshotArg === null) refuse("no snapshot given");
const snapshot = selftest ? JSON.parse(WORKED_EXAMPLE) : await readJson(snapshotArg, "snapshot");
const schema = await readJson(schemaArg ?? COMMITTED_SCHEMA, "schema");

// --- the schema, before the snapshot is read through it -------------------------------

const dangling = refsIn(schema).filter(
  (ref) => !ref.startsWith("#/$defs/") || schema.$defs?.[ref.slice(8)] === undefined,
);
check("every $ref in the committed schema resolves inside it", dangling.length === 0, dangling.join(", "));
const open = openObjects(schema);
check("every object in the committed schema refuses an unknown member", open.length === 0, open.join(", "));

// --- the snapshot, read with nothing but JSON.parse ----------------------------------

const problems = [];
for (const member of ["nodes", "edges", "geometry"]) {
  checkObject(schema, `#/$defs/${member[0].toUpperCase()}${member.slice(1)}`, snapshot[member], `snapshot.${member}`, problems);
}
check("the snapshot carries every member the schema requires", problems.length === 0, problems.join("; "));

// The one rule a versioned contract owes its readers: a reader that meets a *newer
// major* must refuse rather than guess. The schema deliberately does **not** cap `major`
// — it describes the type, not this build — so "the newest major I understand" is the
// reader's own knowledge, stated here as a constant. That is the honest arrangement: a
// stranger reads the schema for the shape and brings their own cut-off.
const READS_MAJOR = 0;
const version = snapshot.version ?? { major: 0, minor: 0 };
const major = schema.$defs.FormatVersion.properties.major;
check(
  "the format version is a non-negative integer pair, as the schema states",
  Number.isInteger(version.major) &&
    version.major >= major.minimum &&
    Number.isInteger(version.minor) &&
    version.minor >= schema.$defs.FormatVersion.properties.minor.minimum,
  JSON.stringify(version),
);
check(
  "the snapshot's major is one this reader understands, so it did not have to refuse",
  version.major <= READS_MAJOR,
  `this reader understands major <= ${READS_MAJOR}`,
);
check(
  "the schema does not cap the major, so a reader is never told to accept one it cannot",
  major.maximum === undefined,
  "the schema caps `major`, which would make a new format inexpressible",
);

const nodeIds = snapshot.nodes?.id ?? [];
const edgeIds = snapshot.edges?.id ?? [];
const sources = snapshot.edges?.source ?? [];
const targets = snapshot.edges?.target ?? [];
const nodeSet = new Set(nodeIds);
const named = (id) => nodeSet.has(id);
check("the snapshot names at least one node and one edge", nodeIds.length > 0 && edgeIds.length > 0);
check("node ids are unique", nodeSet.size === nodeIds.length);
check("edge ids are unique", new Set(edgeIds).size === edgeIds.length);
check("every edge carries both of its endpoints", sources.length === edgeIds.length && targets.length === edgeIds.length);
check("every edge endpoint names a node that exists", sources.every(named) && targets.every(named));

// A kind this reader cannot place is a failure it records, not one it throws on: the
// column walk and the per-node line below both read `wanted`, and a bare `undefined`
// there turned "1 failed" into a TypeError.
const NODE_COLUMNS = { Point: ["x", "y"], Circle: ["x", "y", "r"], Box: ["x", "y", "w", "h"] };
const kind = snapshot.geometry?.nodes?.kind;
const columns = snapshot.geometry?.nodes ?? {};
const wanted = NODE_COLUMNS[kind] ?? [];
check(`the node geometry is a kind this reader can place`, wanted.length > 0, String(kind));
for (const name of wanted) {
  const values = columns[name];
  check(
    `every node has an ${name}`,
    Array.isArray(values) && values.length === nodeIds.length && values.every(Number.isFinite),
  );
}

// --- what a stranger actually gets ---------------------------------------------------

const span = (name) => `${name}[${extent(columns[name] ?? [])}]`;
process.stdout.write(
  `# ${nodeIds.length} nodes, ${edgeIds.length} edges, ${kind} nodes / ${snapshot.geometry?.edges?.kind} edges, ` +
    `bounds ${["x", "y"].map(span).join(" ")}\n`,
);
nodeIds.forEach((id, i) => {
  const at = (columns.x ?? [])[i];
  const across = (columns.y ?? [])[i];
  const extra = wanted
    .filter((name) => !["x", "y"].includes(name))
    .map((name) => `${name}=${(columns[name] ?? [])[i]}`);
  process.stdout.write(`#   ${id} at (${at}, ${across})${extra.length > 0 ? ` ${extra.join(" ")}` : ""}\n`);
});

process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
// `exitCode`, not `exit()`: stdout to a pipe is asynchronous past the pipe's buffer,
// and a 200k-node snapshot's summary is far past it. `exit()` would drop the verdict.
process.exitCode = failures === 0 ? 0 : 1;