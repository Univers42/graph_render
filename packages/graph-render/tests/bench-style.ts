/**
 * What `styleFrom` costs at a million nodes, and which of its parts the time is in: the rank,
 * the radius loop and the colour buckets. `docs/measurements/perf-p6-data-path.md` §4 put
 * 115 ms of a 117 ms restyle in `styleFrom`; this splits that figure.
 *
 * Run it (Docker only, like every other command here):
 *
 *   scripts/orch/node-slim.sh node --experimental-strip-types packages/graph-render/tests/bench-style.ts
 *
 * It prints, it asserts nothing, and it is not a `.test.ts`, so `scripts/studio.sh` never runs it
 * as a gate. `docs/measurements/perf-style-rank.md` quotes what it prints.
 *
 * Every arm that needs a "new" array alternates between two arrays with the same contents, built
 * before the clock starts, so a call never times the copy that made its input.
 */
import { readFileSync } from "node:fs";
import { bucketsOf, DEFAULT_SIZING, radiiOf, rankOf, styleFrom, type Style } from "../src/style.ts";
import { mulberry32 } from "./support.ts";

const NODES = 1_000_000;
const PALETTE = ["#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2", "#7f7f7f"];
const ROUNDS = 3;
const WARMUP = 2;
const CALLS = 5;

type Pair<T> = readonly [T, T];

interface Fixture {
  readonly weights: Pair<Float32Array>;
  readonly colours: Pair<Uint16Array>;
  readonly masks: Pair<Uint8Array>;
}

interface Arm {
  readonly name: string;
  readonly call: (turn: number) => unknown;
}

/** Weights in 0..1, every other one quantised to 1/256 so the rank has long runs of ties. */
function fixture(seed: number): Fixture {
  const next = mulberry32(seed);
  const weights = new Float32Array(NODES);
  const colours = new Uint16Array(NODES);
  const mask = new Uint8Array(NODES);
  for (let i = 0; i < NODES; i += 1) {
    const value = next();
    weights[i] = i % 2 === 0 ? Math.round(value * 256) / 256 : value;
    colours[i] = Math.floor(next() * PALETTE.length);
    mask[i] = next() < 0.5 ? 1 : 0;
  }
  return { weights: [weights, weights.slice()], colours: [colours, colours.slice()], masks: [mask, mask.slice()] };
}

function pick<T>(pair: Pair<T>, turn: number): T {
  return turn % 2 === 0 ? pair[0] : pair[1];
}

function styled(weights: Float32Array, colours: Uint16Array, hidden: Uint8Array | null): Style {
  return styleFrom({ labels: [], weights, colours, palette: PALETTE, hidden });
}

function armsOf({ weights, colours, masks }: Fixture): readonly Arm[] {
  return [
    { name: "styleFrom, fresh weights", call: (turn) => styled(pick(weights, turn), colours[0], null) },
    { name: "styleFrom, fresh weights and colours", call: (turn) => styled(pick(weights, turn), pick(colours, turn), null) },
    { name: "styleFrom, reveal step (new mask only)", call: (turn) => styled(weights[0], colours[0], pick(masks, turn)) },
    { name: "rank alone", call: () => rankOf(weights[0]) },
    { name: "radius loop alone", call: () => radiiOf(weights[0], DEFAULT_SIZING) },
    { name: "bucketsOf alone", call: () => bucketsOf(colours[0], PALETTE.length) },
  ];
}

function median(values: readonly number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
}

/** One round's figure for an arm: the median of CALLS timed calls after WARMUP untimed ones. */
function timeArm(arm: Arm): number {
  const times: number[] = [];
  for (let turn = 0; turn < WARMUP + CALLS; turn += 1) {
    const started = performance.now();
    arm.call(turn);
    if (turn >= WARMUP) times.push(performance.now() - started);
  }
  return median(times);
}

function loadavg(): string {
  try {
    return readFileSync("/proc/loadavg", "utf8").trim();
  } catch {
    return "unavailable";
  }
}

/** ROUNDS rounds, the arm order reversed on every other one, so no arm always runs first. */
function run(arms: readonly Arm[]): Map<string, number[]> {
  const rounds = new Map<string, number[]>(arms.map((arm) => [arm.name, []]));
  for (let round = 0; round < ROUNDS; round += 1) {
    const order = round % 2 === 0 ? arms : [...arms].reverse();
    for (const arm of order) rounds.get(arm.name)?.push(timeArm(arm));
  }
  return rounds;
}

function main(): void {
  const arms = armsOf(fixture(20261003));
  console.log(`nodes ${NODES}, ${ROUNDS} rounds, each the median of ${CALLS} calls after ${WARMUP} warm-up`);
  console.log(`loadavg before: ${loadavg()}`);
  for (const [name, times] of run(arms)) {
    const each = times.map((time) => time.toFixed(3)).join(", ");
    console.log(`${name.padEnd(40)} median ${median(times).toFixed(3).padStart(9)} ms  rounds ${each}`);
  }
  console.log(`loadavg after:  ${loadavg()}`);
}

main();
