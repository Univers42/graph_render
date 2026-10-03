// Seed 66 before and after the fix: which edges' drawn points moved, and that nothing
// else in the snapshot did. The 12 that move must be exactly the edges inside arc
// (20, 28)'s contaminated member "range" whose own arc sorts earlier.
import { readFileSync } from "node:fs";

const before = JSON.parse(readFileSync("scratch/dagrt/s66.prefix.json", "utf8"));
const after = JSON.parse(readFileSync("scratch/dagrt/s66.fixed.json", "utf8"));
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const rows = [];
for (const key of ["nodes", "edges", "version"]) {
  rows.push(`${eq(before[key], after[key]) ? "same" : "MOVED"} ${key}`);
}
rows.push(`${eq(before.notes, after.notes) ? "same" : "MOVED"} notes`);
const bn = before.geometry.nodes;
const an = after.geometry.nodes;
rows.push(`${eq(bn.x, an.x) ? "same" : "MOVED"} node x`);
rows.push(`${eq(bn.y, an.y) ? "same" : "MOVED"} node y`);
const be = before.geometry.edges;
const ae = after.geometry.edges;
rows.push(`${eq(be.pts, ae.pts) ? "same" : "MOVED"} edge pts`);
// Which edges' own point rows differ, and whether each now has |span| - 1 dummies.
const idx = new Map(after.nodes.id.map((id, i) => [id, i]));
const y = an.y;
let same = 0;
const moved = [];
for (let e = 0; e < be.offsets.length - 1; e += 1) {
  const a = be.pts.slice(be.offsets[e] * 2, be.offsets[e + 1] * 2);
  const b = ae.pts.slice(ae.offsets[e] * 2, ae.offsets[e + 1] * 2);
  if (eq(a, b)) { same += 1; continue; }
  const s = idx.get(after.edges.source[e]);
  const t = idx.get(after.edges.target[e]);
  const span = Math.abs(y[t] - y[s]);
  const want = span - 1;
  const got = ae.offsets[e + 1] - ae.offsets[e];
  moved.push({ e, s, t, span, dummiesBefore: be.offsets[e + 1] - be.offsets[e], dummiesAfter: got, ok: got === want });
}
console.log(rows.join("\n"));
console.log(`edges identical: ${same}, moved: ${moved.length}`);
for (const m of moved) {
  console.log(`  edge ${m.e} (${m.s} -> ${m.t}): ${m.dummiesBefore} -> ${m.dummiesAfter} dummies over ${m.span} layers ${m.ok ? "ok" : "STILL WRONG"}`);
}
process.exit(moved.every((m) => m.ok) ? 0 : 1);