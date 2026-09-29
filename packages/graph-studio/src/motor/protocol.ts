/**
 * What crosses between the studio and the motor's worker. The motor runs where it cannot
 * freeze the page, so everything it is asked and everything it answers is one of these.
 */
import type { GraphMeta } from "../source/meta.ts";
import type { ShownError } from "../state/errors.ts";
import type { Source } from "../state/settings.ts";

export interface Assets {
  readonly wasmUrl: string;
  /** Ends in a slash; a fixture's path is appended. */
  readonly fixturesUrl: string;
}

export interface Catalog {
  readonly layouts: readonly string[];
  readonly posts: readonly string[];
  readonly analyses: readonly string[];
}

export interface GraphSummary {
  readonly name: string;
  readonly nodeCount: number;
  readonly edgeCount: number;
  /** Every default filled and annotation dropped while reading the document. */
  readonly notes: readonly string[];
  readonly buildMs: number;
}

export interface RunReport {
  readonly layoutId: string;
  /** The edge pass that ran: `null` when none was asked for, or the one asked for refused. */
  readonly postId: string | null;
  readonly postError: ShownError | null;
  /** The snapshot's binary face. */
  readonly bytes: Uint8Array;
  /** sha256 of `bytes`, or `null` where the platform offers no digest. */
  readonly digest: string | null;
  readonly layoutMs: number;
  readonly postMs: number;
  /** With the first run over a graph, and again if the motor's node order ever changes. */
  readonly meta: GraphMeta | null;
}

export interface AnalysisReport {
  readonly id: string;
  readonly kind: "f64" | "u32";
  readonly values: Float64Array | Uint32Array;
  readonly converged: boolean | null;
  readonly modularity: number | null;
  readonly max: number | null;
  readonly ms: number;
}

export type Request =
  | { readonly type: "open"; readonly wasmUrl: string }
  | { readonly type: "load"; readonly source: Source; readonly fixturesUrl: string }
  | { readonly type: "layout"; readonly layoutId: string; readonly postId: string | null }
  | { readonly type: "analysis"; readonly analysisId: string };

export type Result =
  | { readonly type: "opened"; readonly catalog: Catalog }
  | { readonly type: "loaded"; readonly graph: GraphSummary }
  | { readonly type: "laid-out"; readonly run: RunReport }
  | { readonly type: "analysed"; readonly analysis: AnalysisReport }
  | { readonly type: "failed"; readonly error: ShownError };

export interface Envelope<Body> {
  readonly seq: number;
  readonly body: Body;
}

export interface Port {
  readonly send: (message: Envelope<Request>) => void;
  /** One listener: a second call replaces the first. */
  readonly listen: (handler: (message: Envelope<Result>) => void) => void;
  /** Stops the motor, whatever it is doing. */
  readonly close: () => void;
}

export type Spawn = () => Port;

const REQUESTS: readonly string[] = ["open", "load", "layout", "analysis"];
const RESULTS: readonly string[] = ["opened", "loaded", "laid-out", "analysed", "failed"];

/** Both ends are this package's own code, so the tag is checked and the members trusted. */
function typeOf(value: unknown): string | null {
  if (typeof value !== "object" || value === null || !("seq" in value) || !("body" in value)) return null;
  const { seq, body } = value;
  if (typeof seq !== "number" || typeof body !== "object" || body === null || !("type" in body)) return null;
  return typeof body.type === "string" ? body.type : null;
}

export function isRequest(value: unknown): value is Envelope<Request> {
  return REQUESTS.includes(typeOf(value) ?? "");
}

export function isResult(value: unknown): value is Envelope<Result> {
  return RESULTS.includes(typeOf(value) ?? "");
}
