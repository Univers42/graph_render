/**
 * The motor's side of the conversation: one graph at a time, built from a source, run
 * through a layout and an optional edge pass, read back as snapshot bytes.
 *
 * Knows the motor only by the members it calls, so it runs the same in a worker, on the
 * page with the worker switched off, and under node with the real module.
 */
import { decodeSnapshot, idAt } from "../../../graph-render/src/snapshot/decode.ts";
import { FIXTURES } from "../source/fixtures.ts";
import { type IngestNode, IngestRefusal, normaliseIngest } from "../source/ingest.ts";
import { type GraphMeta, metaOf } from "../source/meta.ts";
import { syntheticRecords } from "../source/synthetic.ts";
import { type ShownError, describeError } from "../state/errors.ts";
import type { Source } from "../state/settings.ts";
import type { AnalysisReport, Catalog, GraphSummary, RunReport } from "./protocol.ts";

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
  layout(handle: Handle, layoutId: string): unknown;
  post(handle: Handle, postId: string): unknown;
  analysis(handle: Handle, analysisId: string): AnalysisFace;
  toBytes(handle: Handle): Uint8Array;
  release(handle: Handle): void;
}

export interface SessionDeps<Handle> {
  readonly motorFrom: (wasmUrl: string) => Promise<MotorLike<Handle>>;
  readonly fetchText: (url: string) => Promise<string>;
  readonly digest: (bytes: Uint8Array) => Promise<string | null>;
  readonly now: () => number;
}

export interface Session {
  open(wasmUrl: string): Promise<Catalog>;
  load(source: Source, fixturesUrl: string): Promise<GraphSummary>;
  layout(layoutId: string, postId: string | null): Promise<RunReport>;
  analysis(analysisId: string): AnalysisReport;
}

interface Document {
  readonly name: string;
  readonly json: string;
  readonly nodes: readonly IngestNode[];
  readonly edgeCount: number;
  readonly notes: readonly string[];
}

interface Built<Handle> {
  readonly handle: Handle;
  readonly nodes: readonly IngestNode[];
  /** The id table the description was last built against; `null` before the first run. */
  described: Uint8Array | null;
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
  if (typeof crypto === "undefined" || !("subtle" in crypto) || crypto.subtle === undefined) return null;
  const digest = await crypto.subtle.digest("SHA-256", bytes.slice());
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function generated(source: Extract<Source, { kind: "synthetic" }>): Document {
  const { nodes, edges } = syntheticRecords({
    seed: source.seed, nodeCount: source.nodes, degree: source.degree, shape: source.shape,
  });
  return {
    name: `${source.shape} seed ${source.seed}`,
    json: JSON.stringify({ version: 1, nodes, edges }),
    nodes, edgeCount: edges.length, notes: [],
  };
}

function normalised(text: string, name: string): Document {
  const { json, doc, notes } = normaliseIngest(text, name);
  return { name, json, nodes: doc.nodes, edgeCount: doc.edges.length, notes };
}

async function documentFor(source: Source, fixturesUrl: string, fetchText: (url: string) => Promise<string>): Promise<Document> {
  if (source.kind === "synthetic") return generated(source);
  if (source.kind === "document") return normalised(source.text, source.name);
  // The path comes from settings, and settings come from recipes: only the listed files.
  if (!FIXTURES.includes(source.path)) throw new IngestRefusal(source.path, "not a bundled fixture");
  return normalised(await fetchText(`${fixturesUrl}${source.path}`), source.path);
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

export function createSession<Handle>(deps: SessionDeps<Handle>): Session {
  let motor: MotorLike<Handle> | null = null;
  let built: Built<Handle> | null = null;
  const ready = (): { readonly motor: MotorLike<Handle>; readonly built: Built<Handle> } => {
    if (motor === null) throw new SessionRefusal("the motor is not open");
    if (built === null) throw new SessionRefusal("no graph is loaded");
    return { motor, built };
  };
  return {
    open: async (wasmUrl) => {
      motor = await deps.motorFrom(wasmUrl);
      return { layouts: motor.layouts(), posts: motor.posts(), analyses: motor.analyses() };
    },
    load: async (source, fixturesUrl) => {
      if (motor === null) throw new SessionRefusal("the motor is not open");
      const document = await documentFor(source, fixturesUrl, deps.fetchText);
      const started = deps.now();
      const handle = motor.build(document.json);
      if (built !== null) motor.release(built.handle);
      built = { handle, nodes: document.nodes, described: null };
      return {
        name: document.name, nodeCount: document.nodes.length, edgeCount: document.edgeCount,
        notes: document.notes, buildMs: deps.now() - started,
      };
    },
    layout: async (layoutId, postId) => {
      const live = ready();
      const started = deps.now();
      live.motor.layout(live.built.handle, layoutId);
      const layoutMs = deps.now() - started;
      const pass = runPass(live.motor, live.built.handle, postId, deps.now);
      const bytes = live.motor.toBytes(live.built.handle);
      const meta = describe(live.built, bytes);
      return { layoutId, ...pass, bytes, digest: await deps.digest(bytes), layoutMs, meta };
    },
    analysis: (analysisId) => {
      const live = ready();
      const started = deps.now();
      const face = live.motor.analysis(live.built.handle, analysisId);
      return reportOf(face, deps.now() - started);
    },
  };
}
