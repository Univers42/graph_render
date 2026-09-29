/**
 * Everything the studio's chrome reads, as one value in one store. Replaced, never
 * edited: a reader holding the old value still holds what was true when it read it.
 */
import type { AnalysisReport, Catalog, GraphSummary } from "../motor/protocol.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { ShownError } from "./errors.ts";
import { DEFAULT_SETTINGS, type Settings } from "./settings.ts";

export interface RunSummary {
  readonly layoutId: string;
  readonly postId: string | null;
  /** Why the edge pass that was asked for did not run. */
  readonly postError: ShownError | null;
  readonly digest: string | null;
  readonly byteLength: number;
  readonly nodeKind: string;
  readonly edgeKind: string;
  readonly layoutMs: number;
  readonly postMs: number;
  /** What the motor degraded, by name and count. */
  readonly notes: readonly string[];
}

export interface LogEntry {
  readonly seq: number;
  /** The line that, typed in the console, asks for the same thing. */
  readonly command: string;
  readonly ok: boolean;
  readonly ms: number;
  readonly message: string;
  readonly digest: string | null;
  readonly notes: readonly string[];
  readonly error: ShownError | null;
}

export interface Running {
  readonly seq: number;
  readonly command: string;
}

export interface StudioState {
  /** What is on screen: changed only by what succeeded. */
  readonly settings: Settings;
  readonly catalog: Catalog | null;
  readonly graph: GraphSummary | null;
  readonly meta: GraphMeta | null;
  readonly run: RunSummary | null;
  readonly analysis: AnalysisReport | null;
  readonly busy: readonly Running[];
  /** The last failure, until something succeeds or it is dismissed. */
  readonly error: ShownError | null;
  readonly log: readonly LogEntry[];
  /** Dense index of the selected node, or -1. */
  readonly selected: number;
  /** Motor layout calls since the studio opened. A filter that does not re-layout leaves it alone. */
  readonly layoutCalls: number;
  /** The filter in force when the last layout ran, as JSON; "" before the first one. */
  readonly runFilter: string;
}

/** The console keeps this many entries; older ones are dropped, oldest first. */
export const LOG_LIMIT = 500;

export function initialState(settings: Settings = DEFAULT_SETTINGS): StudioState {
  return {
    settings, catalog: null, graph: null, meta: null, run: null, analysis: null,
    busy: [], error: null, log: [], selected: -1, layoutCalls: 0, runFilter: "",
  };
}

export function withEntry(state: StudioState, entry: LogEntry): StudioState {
  return { ...state, log: [...state.log, entry].slice(-LOG_LIMIT) };
}
