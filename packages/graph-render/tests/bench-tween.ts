/**
 * Two numbers this job is built on, measured on the host that runs it:
 *
 *   1. one mid-tween pick at 200 000 nodes — the linear scan over the eased pose that
 *      `scene.pickEased` does, against the settled grid it replaces, so the cost of turning
 *      picking back on for the length of a tween is a number and not an adjective.
 *   2. `planLabels` at 20 000 nodes, which is what `canvas2d/layout-key.ts:54`'s `travelling ||`
 *      forces on every tween frame, against the 16.67 ms frame it runs inside.
 *
 * Run it (Docker only, like every other command here):
 *
 *   scripts/orch/node-slim.sh node --experimental-strip-types packages/graph-render/tests/bench-tween.ts
 *
 * It prints, it asserts nothing, and it is not a `.test.ts` so `scripts/studio.sh` never runs it
 * as a gate. The figures it prints are the ones `docs/measurements/perf-transition-pick.md` quotes.
 */
import { fitCamera } from "../src/camera.ts";
import type { Frame } from "../src/frame.ts";
import { DEFAULT_POLICY, newLabelPlan, occupancyFor, planLabels } from "../src/labels.ts";
import { type EasedPose, pickEased, pickIn, sceneOf } from "../src/scene.ts";
import { styleFrom } from "../src/style.ts";
import { mulberry32 } from "./support.ts";

const VIEWPORT = { width: 1920, height: 1080 };
const ROUNDS = 60;

function quantile(values: number[], at: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(at * sorted.length))] ?? 0;
}

/**
 * The milliseconds each of `rounds` calls of `each` took.
 *
 * `WARMUP` is 30 rather than a handful because the scan is bimodal across processes on this host:
 * a run that tiers the loop up lands near 4 ms and one that does not sits near 8, and a short
 * warm-up measures whichever tier the timed rounds happen to start in. Thirty calls of a
 * 200 000-node scan is about 0.25 s, which is the price of a number that means the same thing
 * twice. The five after it are dropped too, for the last inlining decisions.
 */
const WARMUP = 30;

function timed(rounds: number, each: () => void): number[] {
  const out: number[] = [];
  for (let round = 0; round < rounds + WARMUP + 5; round += 1) {
    const started = performance.now();
    each();
    if (round >= WARMUP + 5) out.push(performance.now() - started);
  }
  return out;
}

function report(what: string, times: number[]): void {
  console.log(`${what.padEnd(34)} p50 ${quantile(times, 0.5).toFixed(3)} ms  p95 ${quantile(times, 0.95).toFixed(3)} ms`);
}

/**
 * A graph on the studio's own shape: `degree 2`, one palette entry, a label per node, spread on
 * the unit square scaled to 1 000 world units. Built column-wise rather than through
 * `randomFrame`, whose spread operator cannot take 200 000 arguments.
 */
function dressed(nodes: number, seed: number) {
  const next = mulberry32(seed);
  const x = new Float32Array(nodes);
  const y = new Float32Array(nodes);
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let at = 0; at < nodes; at += 1) {
    x[at] = next() * 1000;
    y[at] = next() * 1000;
    minX = Math.min(minX, x[at] ?? 0);
    maxX = Math.max(maxX, x[at] ?? 0);
    minY = Math.min(minY, y[at] ?? 0);
    maxY = Math.max(maxY, y[at] ?? 0);
  }
  const edges = nodes * 2;
  const frame: Frame = {
    nodeKind: "Point", edgeKind: "Line", nodeCount: nodes, edgeCount: edges, x, y,
    z: null, r: null, w: null, h: null,
    source: Uint32Array.from({ length: edges }, (_, at) => at % nodes),
    target: Uint32Array.from({ length: edges }, (_, at) => (at * 7 + 1) % nodes),
    curveDegree: 0, offsets: null, pts: null, bounds: { minX, minY, maxX, maxY }, factor: 1,
  };
  const style = styleFrom({
    labels: Array.from({ length: nodes }, (_, at) => `node-${at}`),
    weights: Float32Array.from({ length: nodes }, (_, at) => ((at * 37) % 101) / 100),
    colours: new Uint16Array(nodes),
    palette: ["#9a9a9a"],
  });
  const scene = sceneOf(frame, style, null);
  return { frame, scene, camera: fitCamera(scene.bounds, VIEWPORT) };
}

/** One pick per hover frame, over the pose a tween at half its clock is drawing. */
function pickBench(): void {
  const nodes = 200_000;
  const { frame, scene, camera } = dressed(nodes, 7);
  const pose: EasedPose = {
    fromX: frame.x.slice(),
    fromY: frame.y.slice(),
    toX: Float32Array.from(frame.x, (x) => x * 0.5 + 7),
    toY: Float32Array.from(frame.y, (y) => y * 0.5 + 3),
    eased: 0.5,
  };
  const query = { x: 500, y: 500, tolerance: 4 / camera.scale, floor: 1.25 / camera.scale };
  report(`pick ${nodes} eased scan`, timed(ROUNDS, () => void pickEased(scene, query, pose)));
  report(`pick ${nodes} settled grid`, timed(ROUNDS, () => void pickIn(scene, query)));
}

/**
 * What `layout-key.ts`'s `travelling ||` costs a tween frame, on the 2D painter's own budget.
 *
 * The scale is swept, not fixed, because it decides the whole cost: `planLabels` walks the rank
 * order and breaks at the first node whose zoom alpha is invisible, so a camera far enough out
 * that nothing is labelled reads one node and stops. The expensive end is zoomed *in*, where every
 * ranked node has alpha 1, none of them break the walk, and the plan's own early x test is what
 * rejects them — which is the case `canvas2d/loop.ts` already quotes at about 7 ms a frame for a
 * million nodes. A fit alone would have reported the cheap end and called the line free.
 */
function labelBench(): void {
  const plan = newLabelPlan(DEFAULT_POLICY.budget);
  const occupancy = occupancyFor(VIEWPORT);
  for (const nodes of [20_000, 200_000, 1_000_000]) {
    const { scene, camera: fitted } = dressed(nodes, 11);
    const rows: string[] = [];
    // A focus is the second case, and the worse one: with `lit` set, `planLabels` has no
    // invisible node to break on, so it walks the whole rank order however few labels it places.
    // Hovering a moving node sets the focus, so this is a tween frame with the pointer on it.
    const lit = new Uint8Array(nodes);
    lit[0] = 1;
    for (const focus of [null, lit]) {
      const tag = focus === null ? "idle" : "hover";
      for (const scale of [fitted.scale, 8, 32]) {
        const camera = { ...fitted, scale };
        const input = {
          style: scene.style, x: scene.frame.x, y: scene.frame.y, extent: scene.extent, camera,
          viewport: VIEWPORT, lit: focus, policy: DEFAULT_POLICY, widthOf: (node: number) => 12 + (node % 7) * 2, height: 16,
        };
        const times = timed(ROUNDS, () => planLabels(input, plan, occupancy));
        rows.push(`${tag} x${scale.toFixed(1).padStart(5)} ${quantile(times, 0.5).toFixed(3)}`);
      }
    }
    console.log(`planLabels ${String(nodes).padStart(7)}  ${rows.join("  ")}  (ms)`);
  }
  console.log("\n3% of a 16.67 ms frame is 0.50 ms.");
}

pickBench();
labelBench();