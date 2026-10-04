/**
 * The motor's side of the conversation: one graph at a time, built from a source, run
 * through a layout and an optional edge pass, read back as snapshot bytes.
 *
 * Knows the motor only by the members it calls, so it runs the same in a worker, on the
 * page with the worker switched off, and under node with the real module.
 */
import { decodeSnapshot, idAt } from "../../../graph-render/src/snapshot/decode.ts";
import type { IngestNode } from "../source/ingest.ts";
import { type GraphMeta, metaOf } from "../source/meta.ts";
import { type Assembler, type Document, documentFor } from "./documents.ts";
import { type ShownError, describeError } from "../state/errors.ts";
import type { Source } from "../state/settings.ts";
import type { ForceEngine, ForceParams, ForcePort, LiveForce } from "./live.ts";
import { createLiveForce } from "./liveSession.ts";
import type { AnalysisReport, Catalog, GraphBatch, GraphSummary, RunReport } from "./protocol.ts";
import { planRun } from "./settle.ts";

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
   * Appends a batch to a built graph (`Motor.extend`); whole or not at all, and the graph is
   * as it was when it refuses. Optional so a motor built before the extend path — and every
   * test double — still satisfies this interface, and a delta batch is refused with a reason
   * rather than dropped.
   */
  extend?(handle: Handle, batch: GraphBatch): void;
  /** The live session over a graph's topology, or null on a motor without one. */
  forceSession?(handle: Handle, params?: Partial<ForceParams>, engine?: ForceEngine): ForcePort | null;
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
}

export interface Session {
  open(wasmUrl: string, threads?: number): Promise<Catalog>;
  load(source: Source, fixturesUrl: string): Promise<GraphSummary>;
  layout(layoutId: string, postId: string | null): Promise<RunReport>;
  analysis(analysisId: string): AnalysisReport;
  /**
   * The live force port over the graph as it is now drawn, or null when there is none: the
   * motor behind this session has no live session, or no layout has run to order the rows.
   * A new load releases the last one, so a port is never a session of another graph.
   */
  forces(): LiveForce | null;
}

interface Built<Handle> {
  readonly handle: Handle;
  readonly nodes: readonly IngestNode[];
  /** The id table the description was last built against; `null` before the first run. */
  described: Uint8Array | null;
  /** The motor's live session over this graph, made when one is first asked for. */
  forced: ForcePort | null;
  /** The port over it, cached so the loop sees one object for one session. */
  port: LiveForce | null;
  /** Node ids in the force session's dense row order; `null` until a layout has run. */
  order: readonly string[] | null;
  /** The tick the live session is made with; a change releases the one made before. */
  engine: ForceEngine;
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

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) if (a[i] !== b[i]) return false;
  return true;
}

/** The description of the graph in the order this snapshot uses, if that order is news. */
function describe<Handle>(built: Built<Handle>, bytes: Uint8Array): GraphMeta | null {
  const snapshot = decodeSnapshot(bytes);
  const table = snapshot.nodeIds.bytes;
  if (built.described !== null && sameBytes(built.described, table)) return null;
  const order = Array.from({ length: snapshot.nodeCount }, (_, i) => idAt(snapshot.nodeIds, i));
  built.described = table.slice();
  built.order = order;
  return metaOf(built.nodes, order, snapshot);
}

interface Pass {
  readonly postId: string | null;
  readonly postError: ShownError | null;
  readonly postMs: number;
}

function runPass<Handle>(motor: MotorLike<Handle>, handle: Handle, postId: string | null, now: () => number): Pass {
  if (postId === null) return { postId, postError: null, postMs: 0 };
  const started = now();
  try {
    motor.post(handle, postId);
    return { postId, postError: null, postMs: now() - started };
  } catch (error) {
    // A refused pass leaves the layout's own edges in place, so the run is still drawn.
    return { postId: null, postError: describeError(error), postMs: now() - started };
  }
}

function reportOf(face: AnalysisFace, ms: number): AnalysisReport {
  return {
    id: face.id,
    kind: face.kind,
    values: face.kind === "u32" ? Uint32Array.from(face.values) : Float64Array.from(face.values),
    converged: face.converged ?? null,
    modularity: face.modularity ?? null,
    max: face.max ?? null,
    ms,
  };
}

function summaryOf(document: Document, buildMs: number): GraphSummary {
  return {
    name: document.name, nodeCount: document.nodes.length, edgeCount: document.edgeCount,
    notes: document.notes, buildMs,
  };
}

/**
 * The live force port over the graph as it is now drawn, or null when there is none.
 *
 * Null rather than a refusal: a force request that arrives before a graph is loaded is "no
 * session yet", which is the same answer as a motor that has none.
 */
function forcesOf<Handle>(motor: MotorLike<Handle> | null, built: Built<Handle> | null): LiveForce | null {
  if (motor === null || built === null || built.order === null) return null;
  if (motor.forceSession === undefined) return null;
  built.forced ??= motor.forceSession(built.handle, undefined, built.engine);
  if (built.forced === null) return null;
  // The port is cached, not rebuilt: the loop compares ports by identity and replaces itself
  // when one changes, so a fresh object per request would stop the loop on every message. So
  // `restart` hands the new session to `built.forced`, which `forget` then releases.
  const session = built.forced;
  built.port ??= createLiveForce({
    session,
    ids: () => built.order,
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
    },
    grow: () => {
      if (session.grow === undefined) throw new SessionRefusal("this motor's live session cannot grow");
      session.grow();
    },
  });
  return built.port;
}

/**
 * One layout over the graph, with the edge pass and the digest the studio reports. A force
 * layout on a large graph runs as a scatter and reports the layout that settles it (`settle.ts`).
 */
async function runLayout<Handle>(
  live: { readonly motor: MotorLike<Handle>; readonly built: Built<Handle> },
  deps: SessionDeps<Handle>,
  layoutId: string,
  postId: string | null,
): Promise<RunReport> {
  const { motor, built } = live;
  const plan = planRun(layoutId, built.nodes.length, motor.forceSession !== undefined);
  if (plan.engine !== built.engine) {
    forget(built, deps.onForget);
    built.engine = plan.engine;
  }
  const started = deps.now();
  motor.layout(built.handle, plan.run);
  const layoutMs = deps.now() - started;
  const pass = runPass(motor, built.handle, postId, deps.now);
  const bytes = motor.toBytes(built.handle);
  const meta = describe(built, bytes);
  return { layoutId: plan.report, ...pass, bytes, digest: await deps.digest(bytes), layoutMs, meta };
}

/** One analysis over the graph, in the face the studio reports. */
function runAnalysis<Handle>(
  live: { readonly motor: MotorLike<Handle>; readonly built: Built<Handle> },
  deps: SessionDeps<Handle>,
  analysisId: string,
): AnalysisReport {
  const started = deps.now();
  const face = live.motor.analysis(live.built.handle, analysisId);
  return reportOf(face, deps.now() - started);
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
  const ready = (): { readonly motor: MotorLike<Handle>; readonly built: Built<Handle> } => {
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
      engine: "barnes_hut" };
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
    forces: () => forcesOf(motor, built),
  };
}
