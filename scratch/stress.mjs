// Stress comparison for phase06 (P56_SPEC.md decisions 6-7, devil C10).
// Reads our Rust dumps (topology + our positions, from `force_dump`), runs d3-force@3.0.0
// on the SAME topology with the frozen force set and our own golden-spiral seed, and
// prints each case's stress correlation for d3, our Barnes-Hut, and our FA2 — plus the
// margin `ours_bh - d3` (spec: must be >= -0.05).
//
// Usage: node scratch/stress.mjs <dumps-dir> <case1> <case2> ...
// Each <case> names a pair of files: <dumps-dir>/<case>-barnes_hut.txt and
// <dumps-dir>/<case>-fa2.txt (both hold the same topology; that is asserted).

import { readFileSync } from "node:fs";
import { forceSimulation, forceLink, forceManyBody, forceCenter, forceCollide } from "d3-force";

const GOLDEN_ANGLE = 2.399963229728653;

function parseDump(text) {
  const lines = text.split("\n");
  let i = 0;
  const n = Number(lines[i++].split(" ")[1]);
  const m = Number(lines[i++].split(" ")[1]);
  const source = new Array(m), target = new Array(m);
  for (let e = 0; e < m; e++) {
    const [s, t] = lines[i++].split(" ").map(Number);
    source[e] = s; target[e] = t;
  }
  if (lines[i++] !== "POSITIONS") throw new Error("expected POSITIONS");
  const x = new Array(n), y = new Array(n);
  for (let v = 0; v < n; v++) {
    const [px, py] = lines[i++].split(" ").map(Number);
    x[v] = px; y[v] = py;
  }
  return { n, m, source, target, x, y };
}

function goldenSpiral(n) {
  const x = new Array(n), y = new Array(n);
  for (let i = 0; i < n; i++) {
    const r = 12 * Math.sqrt(i + 1);
    x[i] = Math.cos(i * GOLDEN_ANGLE) * r;
    y[i] = Math.sin(i * GOLDEN_ANGLE) * r;
  }
  return { x, y };
}

// The frozen force set (P56_SPEC.md decision 7; params.rs's own citations).
function runD3(topo) {
  const nodes = Array.from({ length: topo.n }, (_, i) => ({ index: i }));
  const seed = goldenSpiral(topo.n);
  for (let i = 0; i < topo.n; i++) { nodes[i].x = seed.x[i]; nodes[i].y = seed.y[i]; }
  const links = topo.source.map((s, e) => ({ source: s, target: topo.target[e], strength: 0.5 }));
  const link = forceLink(links).id((d) => d.index)
    .distance((l) => 60 / Math.max(0.4, l.strength))
    .strength((l) => Math.min(0.7, 0.15 * l.strength));
  const sim = forceSimulation(nodes)
    .force("link", link)
    .force("charge", forceManyBody().strength(-90).distanceMax(520))
    .force("center", forceCenter(0, 0))
    .force("collide", forceCollide().radius(16).iterations(1))
    .alpha(1).alphaDecay(0.06).velocityDecay(0.42)
    .stop();
  sim.tick(112);
  return { x: nodes.map((d) => d.x), y: nodes.map((d) => d.y) };
}

function bfsHops(n, adj, pivot) {
  const dist = new Array(n).fill(-1);
  dist[pivot] = 0;
  const queue = [pivot];
  let head = 0;
  while (head < queue.length) {
    const u = queue[head++];
    for (const v of adj[u]) {
      if (dist[v] === -1) { dist[v] = dist[u] + 1; queue.push(v); }
    }
  }
  return dist;
}

function pearson(xs, ys) {
  const n = xs.length;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let sxy = 0, sxx = 0, syy = 0;
  for (let i = 0; i < n; i++) {
    const dx = xs[i] - mx, dy = ys[i] - my;
    sxy += dx * dy; sxx += dx * dx; syy += dy * dy;
  }
  return sxy / Math.sqrt(sxx * syy);
}

// Max-min (farthest-point) pivot selection (P56_SPEC.md decision 6): start at dense
// index 0, then repeatedly add the node whose distance to its NEAREST already-chosen
// pivot is largest (ties -> lowest index), until `count` pivots or every node is one.
// An unreachable node's distance from a pivot outside its component is treated as
// infinite, so a disconnected fixture picks up a pivot in each component before
// refining within one (that component's own internal spread only matters once every
// component already has a representative).
function selectPivots(n, adj, count) {
  const pivots = [0];
  const chosen = new Set([0]);
  const minDist = bfsHops(n, adj, 0).map((d) => (d === -1 ? Infinity : d));
  while (pivots.length < count && pivots.length < n) {
    let best = -1;
    let bestDist = -Infinity;
    for (let v = 0; v < n; v++) {
      if (chosen.has(v)) continue;
      if (minDist[v] > bestDist) {
        bestDist = minDist[v];
        best = v;
      }
    }
    if (best === -1) break;
    pivots.push(best);
    chosen.add(best);
    const distFromBest = bfsHops(n, adj, best);
    for (let v = 0; v < n; v++) {
      const dv = distFromBest[v] === -1 ? Infinity : distFromBest[v];
      if (dv < minDist[v]) minDist[v] = dv;
    }
  }
  return pivots;
}

function stress(topo, x, y) {
  const adj = Array.from({ length: topo.n }, () => []);
  for (let e = 0; e < topo.m; e++) {
    adj[topo.source[e]].push(topo.target[e]);
    adj[topo.target[e]].push(topo.source[e]);
  }
  const pivotCount = Math.min(32, topo.n);
  const pivots = selectPivots(topo.n, adj, pivotCount);
  const hops = [], euclid = [];
  for (const p of pivots) {
    const dist = bfsHops(topo.n, adj, p);
    for (let v = 0; v < topo.n; v++) {
      if (v === p || dist[v] === -1) continue;
      hops.push(dist[v]);
      euclid.push(Math.hypot(x[v] - x[p], y[v] - y[p]));
    }
  }
  if (hops.length < 2) return null;
  return pearson(hops, euclid);
}

function loadCase(dumpsDir, name) {
  const bh = parseDump(readFileSync(`${dumpsDir}/${name}-barnes_hut.txt`, "utf8"));
  const fa2 = parseDump(readFileSync(`${dumpsDir}/${name}-fa2.txt`, "utf8"));
  if (bh.n !== fa2.n || bh.m !== fa2.m) throw new Error(`${name}: topology mismatch between dumps`);
  return { bh, fa2 };
}

const [, , dumpsDir, ...cases] = process.argv;
const rows = [];
for (const name of cases) {
  const { bh, fa2 } = loadCase(dumpsDir, name);
  const d3pos = runD3(bh);
  const rD3 = stress(bh, d3pos.x, d3pos.y);
  const rBh = stress(bh, bh.x, bh.y);
  const rFa2 = stress(fa2, fa2.x, fa2.y);
  const margin = rBh === null || rD3 === null ? null : rBh - rD3;
  rows.push({ name, n: bh.n, m: bh.m, rD3, rBh, rFa2, margin });
}
console.log(JSON.stringify(rows, null, 2));
