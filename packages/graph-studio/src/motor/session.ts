/**
 * The motor's side of the conversation: one graph at a time, built from a source, run
 * through a layout and an optional edge pass, read back as snapshot bytes.
 *
 * Knows the motor only by the members it calls, so it runs the same in a worker, on the
 * page with the worker switched off, and under node with the real module.
 */
import type { Built } from "./built.ts";
import { type Assembler, type Document, documentFor } from "./documents.ts";
import type { Source } from "../state/settings.ts";
import {
  DEFAULT_KNOBS, type ForceEngine, type ForceParams, type ForcePort, type ForceSeed, type Growable, type LiveForce,
} from "./live.ts";
import { createLiveForce } from "./liveSession.ts";
import type { AnalysisReport, Catalog, GraphBatch, GraphSummary, RunReport } from "./protocol.ts";
import { type Live, runAnalysis, snapshot } from "./run.ts";
import { type RunPlan, SCATTER, planRun } from "./settle.ts";

export interface AnalysisFace {
  readonly id: string;
  readonly kind: "f64" | "u32";
  readonly values: readonly number[];
  readonly converged?: boolean | undefined;
  readonly modularity?: number | undefined;
  readonly max?: number | undefined;
}

export interface MotorLike<Handle> {
  layouts(): readonly string[];
  posts(): readonly string[];
  analyses(): readonly string[];
  build(ingestJson: string): Handle;
  /**
   * Builds a graph from the **binary** columnar document (`docs/contract/ingest-columns.md`).
   * Separate from `build` because it is a separate export in the module and stays separate: a
   * caller holding columns must not be routed through a JSON text it would have to build
   * first, and a caller holding text must not be routed through an encoder it does not need.
   */
  buildColumns(bytes: Uint8Array): Handle;
  layout(handle: Handle, layoutId: string): unknown;
  post(handle: Handle, postId: string): unknown;
  analysis(handle: Handle, analysisId: string): AnalysisFace;
  toBytes(handle: Handle): Uint8Array;
  release(handle: Handle): void;
  /**
   * Appends a batch to a built graph (`Motor.extend`); whole or not at all. Optional so a motor
   * without it — and every test double — still satisfies it, and a batch is then refused.
   */
  extend?(handle: Handle, batch: GraphBatch): void;
  /** The live session over a graph's topology, or null on a motor without one. */
  forceSession?(handle: Handle, params?: Partial<ForceParams>, engine?: ForceEngine, seed?: ForceSeed): (ForcePort & Growable<Handle>) | null;
}

export interface SessionDeps<Handle> {
  readonly motorFrom: (wasmUrl: string, threads?: number) => Promise<MotorLike<Handle>>;
  readonly fetchText: (url: string) => Promise<string>;
  readonly digest: (bytes: Uint8Array) => Promise<string | null>;
  /**
   * Turns a generated graph's columns into the binary document. Injected for the same reason
   * `digest` is: this module may not import the motor's SDK (`app/eslint.config.js`), and the
   * SDK owns the encoder.
   */
  readonly assemble: Assembler;
  readonly now: () => number;
  /**
   * Told when a force session is released, so whatever is stepping it stops at once. Never
   * called to ask whether there is a session: that would make one as a side effect.
   */
  readonly onForget?: () => void;
  /** The gate's negative control: true drops the grow after an extend, so nothing moves. */
  readonly breakDeltas?: () => boolean;
  /**
   * Told when a re-layout releases the force session: the loop stops as for `onForget`, but
   * forces stay available, since the next request seeds a session at the new picture. Left
   * out, a re-layout tells `onForget`.
   */
  readonly onRenew?: () => void;
}

export interface Session {
  open(wasmUrl: string, threads?: number): Promise<Catalog>;
  load(source: Source, fixturesUrl: string): Promise<GraphSummary>;
  layout(layoutId: string, postId: string | null): Promise<RunReport>;
  analysis(analysisId: string): AnalysisReport;
  /** The scatter a delta batch needs drawn, over the graph as it now is; the force session stays. */
  structure(): Promise<RunReport>;
  /**
   * The live force port over the graph as it is now drawn, or null when there is none: the
   * motor behind this session has no live session, or no layout has run to order the rows.
   * A new load releases the last one, so a port is never a session of another graph.
   */
  forces(): LiveForce | null;
}

/** Nothing can run in the state the session is in. */
export class SessionRefusal extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SessionRefusal";
  }
}

export async function sha256Hex(bytes: Uint8Array): Promise<string | null> {
  // `crypto.subtle` exists only in a secure context: absent on a page served over plain
  // http from anything but localhost.
  if (typeof crypto === "undefined" || !("subtle" in crypto)) return null;
  const digest = await crypto.subtle.digest("SHA-256", bytes.slice());
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function summaryOf(document: Document, buildMs: number): GraphSummary {
  return {
    name: document.name, nodeCount: document.nodes.length, edgeCount: document.edgeCount,
    notes: document.notes, buildMs,
  };
}

/**
 * The live force port over the graph as it is now drawn, or null when there is none: a force
 * request before a graph is loaded is "no session yet", the same answer as a motor with none.
 */
function forcesOf<Handle>(motor: MotorLike<Handle> | null, built: Built<Handle> | null, deps: SessionDeps<Handle>): LiveForce | null {
  if (motor === null || built === null || built.order === null) return null;
  if (motor.forceSession === undefined) return null;
  built.forced ??= startSession(motor, built);
  if (built.forced === null) return null;
  // The port is cached, not rebuilt: the loop compares ports by identity and replaces itself
  // when one changes, so a fresh object per request would stop the loop on every message. So
  // `restart` hands the new session to `built.forced`, which `forget` then releases.
  built.port ??= createLiveForce({
    session: built.forced,
    ids: () => built.order,
    knobs: built.knobs,
    // "Animate" settles from the motor's own spiral, hot, whatever the last run drew.
    restart: () => {
      built.forced?.release();
      built.forced = motor.forceSession?.(built.handle, undefined, built.engine) ?? null;
      if (built.forced === null) throw new SessionRefusal("the motor made no force session");
      return built.forced;
    },
    // Both refusals are the queue's to answer: `force.deltas` turns a throw here into a
    // `failed` result carrying the message, and the graph is untouched either way.
    extend: (batch) => {
      if (motor.extend === undefined) throw new SessionRefusal("this motor cannot add to a built graph");
      motor.extend(built.handle, batch);
      // After the motor: a refusal leaves the graph and this list as they were.
      built.nodes = [...built.nodes, ...batch.nodes];
    },
    grow: () => {
      if (deps.breakDeltas?.() === true) return;
      // Read late: "Animate" restarts the session, so a captured binding is a released one.
      const running = built.forced;
      if (running === null || running.grow === undefined) throw new SessionRefusal("this motor's live session cannot grow");
      running.grow(built.handle);
    },
  });
  return built.port;
}

/**
 * The session a force request finds: seeded at the picture the last run drew and born cold,
 * so its first frame repaints that picture instead of replacing it, and a drag or a knob wakes
 * it from there. A scatter (`settle.ts`) is no picture to keep: that session starts hot from
 * the motor's spiral and settles on screen.
 *
 * Measured before this (2026-10-03): every force layout was replaced on the first frame by one
 * settle from the spiral, so ForceAtlas2 and DrL drew identical bounds.
 */
function startSession<Handle>(motor: MotorLike<Handle>, built: Built<Handle>): (ForcePort & Growable<Handle>) | null {
  if (!built.warm) return motor.forceSession?.(built.handle, undefined, built.engine) ?? null;
  const session = motor.forceSession?.(built.handle, undefined, built.engine, "layout") ?? null;
  session?.reheat(0);
  return session;
}

/** A run is a new picture, so the session over the last one goes. The knobs stay, the pins go. */
function renew<Handle>(built: Built<Handle>, plan: RunPlan, deps: SessionDeps<Handle>): void {
  built.knobs = built.port?.knobs?.() ?? built.knobs;
  forget(built, deps.onRenew ?? deps.onForget);
  built.engine = plan.engine;
  built.warm = plan.run === plan.report;
}

/**
 * One layout over the graph, with the edge pass and the digest the studio reports. A force
 * layout on a large graph runs as a scatter and reports the layout that settles it (`settle.ts`).
 */
async function runLayout<Handle>(live: Live<Handle>, deps: SessionDeps<Handle>, layoutId: string, postId: string | null): Promise<RunReport> {
  const plan = planRun(layoutId, live.built.nodes.length, live.motor.forceSession !== undefined);
  renew(live.built, plan, deps);
  return snapshot(live, deps, plan, postId);
}

/**
 * Lets a built graph go: the motor's session, the port over it, and the loop stepping it.
 *
 * WHY the port is marked before the release: the stepping loop holds it, its next frame is
 * already scheduled, and every call on a released session throws. WHY the notice goes last:
 * the loop is told once the port says dead, so a frame in between reads the mark and stops
 * on its own.
 */
function forget<Handle>(built: Built<Handle> | null, onForget?: () => void): void {
  const forced = built?.forced ?? null;
  if (built !== null && built.port !== null) built.port.dead = true;
  forced?.release();
  if (built === null) return;
  built.forced = null;
  built.port = null;
  if (forced !== null) onForget?.();
}

export function createSession<Handle>(deps: SessionDeps<Handle>): Session {
  let motor: MotorLike<Handle> | null = null;
  let built: Built<Handle> | null = null;
  const ready = (): Live<Handle> => {
    if (motor === null) throw new SessionRefusal("the motor is not open");
    if (built === null) throw new SessionRefusal("no graph is loaded");
    return { motor, built };
  };
  /** Builds the next graph and lets the last one go, force session and all. Which build is
   *  called follows the document: a generated graph is already columns, a document or a
   *  fixture is already text, and neither is ever converted to the other's form. */
  const replace = (document: Document, started: number): GraphSummary => {
    const open = motor;
    if (open === null) throw new SessionRefusal("the motor is not open");
    const payload = document.payload;
    const handle = payload.kind === "columns" ? open.buildColumns(payload.bytes) : open.build(payload.text);
    if (built !== null) open.release(built.handle);
    forget(built, deps.onForget);
    built = { handle, nodes: document.nodes, described: null, forced: null, port: null, order: null,
      engine: "barnes_hut", warm: false, knobs: DEFAULT_KNOBS };
    return summaryOf(document, deps.now() - started);
  };
  return {
    open: async (wasmUrl, threads) => {
      motor = await deps.motorFrom(wasmUrl, threads);
      return { layouts: motor.layouts(), posts: motor.posts(), analyses: motor.analyses() };
    },
    load: async (source, fixturesUrl) => {
      const document = await documentFor(source, fixturesUrl, deps.fetchText, deps.assemble);
      return replace(document, deps.now());
    },
    layout: async (layoutId, postId) => runLayout(ready(), deps, layoutId, postId),
    analysis: (analysisId) => runAnalysis(ready(), deps, analysisId),
    // Not `layout`: a run renews the force session, and a delta batch's snapshot must not.
    structure: async () => snapshot(ready(), deps, { run: SCATTER, report: SCATTER }, null),
    forces: () => forcesOf(motor, built, deps),
  };
}
