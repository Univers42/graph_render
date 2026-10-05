// synthetic.mjs <n> <seed> <out>: a commit history in `map.mjs`'s FORMAT, children first, for a
// bench whose input is otherwise whatever the host happens to have checked out.
//
// What it models: a working set of live branch tips. Each commit picks one of three moves —
// branch (5%), merge (5%), or a commit on an existing branch (90%) — so the log has the fan-out
// and re-convergence a real history has, and the lanes layout has something to lay out. Commit i's
// hash is its index in hex, its time is 1600000000 + i, its author is `a<i % 50>`; the model is
// exact and seeded, so the same seed writes the same log.
//
// Caveat: a real history has what this does not model — octopus merges (more than two parents)
// are all two-parent merges here, so nothing exercises the multi-parent row ordering; a
// cherry-pick duplicates a change under a second hash rather than reusing one; commit times come
// from the clock, so they are skewed, out of order and occasionally identical, while here `%at` is
// strictly increasing by one second and never tied.

import { createWriteStream } from "node:fs";

const US = String.fromCharCode(0x1f);
const EPOCH = 1_600_000_000;
const BRANCH_AT = 0.05;
const MERGE_AT = 0.1;
const MAX_LIVE = 64;
const TIP_BIAS = 0.5;

/** mulberry32: the seeded generator the whole log is drawn from. No other source of randomness. */
function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const hash = (i) => i.toString(16).padStart(40, "0");

/** The parent list of commit `i`, and the live tips it leaves behind. */
function step(live, i, next) {
  const r = next();
  if (r < BRANCH_AT && live.length < MAX_LIVE) {
    const j = Math.floor(next() * live.length);
    const parent = live[j];
    live.push(i);
    return [parent];
  }
  if (r < MERGE_AT && live.length > 1) {
    const a = Math.floor(next() * live.length);
    let b = Math.floor(next() * (live.length - 1));
    if (b >= a) b += 1;
    const [lo, hi] = a < b ? [a, b] : [b, a];
    const parents = [live[lo], live[hi]];
    live[lo] = i;
    live.splice(hi, 1);
    return parents;
  }
  const j = next() < TIP_BIAS ? 0 : Math.floor(next() * live.length);
  const parent = live[j];
  live[j] = i;
  return [parent];
}

/** Commit i's record, without its newline. */
function record(i, parents) {
  return [hash(i), parents.map(hash).join(" "), `a${i % 50}`, `${EPOCH + i}`, "", `c${i}`].join(US);
}

/** Every record, children first: commit 0 is the root, every other commit names an older parent. */
export function* history(n, seed) {
  const next = mulberry32(seed);
  const live = [0];
  yield record(0, []);
  for (let i = 1; i < n; i += 1) yield record(i, step(live, i, next));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [count, seed, out] = process.argv.slice(2);
  if (!count || !seed || !out) {
    process.stderr.write("usage: synthetic.mjs <n> <seed> <out>\n");
    process.exit(2);
  }
  const stream = createWriteStream(out);
  // Back-pressure is awaited: a 1M-record log is ~90 MB and a drained-less write stream buffers
  // it in memory instead of on disk.
  for (const line of history(Number(count), Number(seed))) {
    if (!stream.write(`${line}\n`)) await new Promise((resolve) => stream.once("drain", resolve));
  }
  await new Promise((resolve, reject) => { stream.end(resolve); stream.on("error", reject); });
}