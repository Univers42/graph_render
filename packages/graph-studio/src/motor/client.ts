/**
 * The studio's side of the conversation: promises over a port.
 *
 * A layout cannot be interrupted from inside, so stopping one closes the worker. The next
 * call starts a new one and gives it the graph that was last loaded, which costs one
 * module start and one build; the studio's state never sees the gap.
 */
import type { ShownError } from "../state/errors.ts";
import type { ParamValues, Source } from "../state/settings.ts";
import type {
  AnalysisReport, Assets, Catalog, Envelope, ForceRequest, GraphSummary, LayoutParamSpec, Port, Request, Result, RunReport, Spawn,
} from "./protocol.ts";

export class CancelledError extends Error {
  constructor() {
    super("the run was stopped");
    this.name = "CancelledError";
  }
}

/** The motor, or the worker around it, refused. Carries what the worker described. */
export class MotorFailure extends Error {
  readonly shown: ShownError;

  constructor(shown: ShownError) {
    super(shown.detail);
    this.name = shown.title;
    this.shown = shown;
  }
}

export interface MotorClient {
  catalog(): Promise<Catalog>;
  load(source: Source): Promise<GraphSummary>;
  /** `params` is what to run the layout at; absent means the motor's own defaults. */
  layout(layoutId: string, postId: string | null, params?: ParamValues): Promise<RunReport>;
  /** The schema the motor publishes for `layoutId`; `[]` when it publishes none. */
  params(layoutId: string): Promise<readonly LayoutParamSpec[]>;
  analysis(analysisId: string): Promise<AnalysisReport>;
  /** Stops what is running. False when nothing was. */
  cancel(): boolean;
  busy(): boolean;
  /**
   * Sends a force request without waiting for a reply; dropped when no motor is open. Optional
   * so a test double need not carry it.
   */
  force?(request: ForceRequest): void;
  /** Every frame the live loop pushes, and its state; returns the cancel. */
  onForce?(handler: (result: Result) => void): () => void;
  /**
   * The worker failing where the page can hear it, and nothing else; returns the cancel.
   * Optional so a test double need not carry it.
   */
  onFail?(handler: (detail: string) => void): () => void;
  close(): void;
}

interface Waiting {
  readonly resolve: (result: Result) => void;
  readonly reject: (error: Error) => void;
}

interface Link {
  readonly port: Port;
  /** Settles once the motor is open and holds the last loaded graph. */
  readonly ready: Promise<Catalog>;
}

interface State {
  link: Link | null;
  seq: number;
  loaded: Source | null;
  closed: boolean;
  readonly waiting: Map<number, Waiting>;
  /** Everything the motor pushes without being asked: live frames and the force state. */
  readonly pushed: Set<(result: Result) => void>;
  /** Every way the worker can fail in the page's hearing, and nothing else. */
  readonly failures: Set<(detail: string) => void>;
}

function mismatch(wanted: string, result: Result): Error {
  return new Error(`the motor answered ${result.type} to a request for ${wanted}`);
}

function exchange(state: State, port: Port, body: Request): Promise<Result> {
  state.seq += 1;
  const message: Envelope<Request> = { seq: state.seq, body };
  return new Promise<Result>((resolve, reject) => {
    state.waiting.set(message.seq, { resolve, reject });
    port.send(message);
  }).then((result) => {
    if (result.type === "failed") throw new MotorFailure(result.error);
    return result;
  });
}

async function openOn(state: State, port: Port, assets: Assets): Promise<Catalog> {
  const threads = assets.threads === undefined ? {} : { threads: assets.threads };
  const opened = await exchange(state, port, { type: "open", wasmUrl: assets.wasmUrl, ...threads });
  if (opened.type !== "opened") throw mismatch("open", opened);
  if (state.loaded !== null) {
    await exchange(state, port, { type: "load", source: state.loaded, fixturesUrl: assets.fixturesUrl });
  }
  return opened.catalog;
}

/** A live frame, and the loop's own state: neither answers a request, so neither is waited for. */
function isPushed(result: Result): boolean {
  return result.type === "force-frame" || result.type === "force-state";
}

function connect(state: State, spawn: Spawn, assets: Assets): Link {
  const port = spawn();
  port.onFail?.((detail) => {
    for (const fail of state.failures) fail(detail);
  });
  port.listen((message) => {
    // Asked first: a pushed message carries UNSOLICITED, and there is no waiter under it.
    if (isPushed(message.body)) {
      for (const push of state.pushed) push(message.body);
      return;
    }
    const waiting = state.waiting.get(message.seq);
    state.waiting.delete(message.seq);
    waiting?.resolve(message.body);
  });
  const link: Link = { port, ready: openOn(state, port, assets) };
  // A failed start is reported by the call that waited for it; here it must not go unhandled.
  link.ready.catch(() => undefined);
  state.link = link;
  return link;
}

function drop(state: State): void {
  state.link?.port.close();
  state.link = null;
  const stopped = [...state.waiting.values()];
  state.waiting.clear();
  for (const waiting of stopped) waiting.reject(new CancelledError());
}

function cancelWaiting(state: State): boolean {
  if (state.waiting.size === 0) return false;
  drop(state);
  return true;
}

function fireAndForget(state: State, body: Request): void {
  if (state.link === null || state.closed) return;
  state.seq += 1;
  state.link.port.send({ seq: state.seq, body });
}

/** The one answer of a wanted kind, or a refusal naming what came back instead. */
function loaded(result: Result): GraphSummary {
  if (result.type !== "loaded") throw mismatch("load", result);
  return result.graph;
}

/** The schema of the layout asked about; a motor that names another layout is not the answer. */
function published(result: Result, layoutId: string): readonly LayoutParamSpec[] {
  if (result.type !== "params") throw mismatch("params", result);
  if (result.layoutId !== layoutId) throw new Error(`the motor answered the schema of ${result.layoutId} to a request for ${layoutId}`);
  return result.specs;
}

function laidOut(result: Result): RunReport {
  if (result.type !== "laid-out") throw mismatch("layout", result);
  return result.run;
}

function analysed(result: Result): AnalysisReport {
  if (result.type !== "analysed") throw mismatch("analysis", result);
  return result.analysis;
}

/**
 * A run's values, as the request carries them: nothing at all where there are none, so a run
 * with no values of its own sends no `params` member and the motor takes its own defaults.
 */
function asked(params: ParamValues | undefined): { readonly params?: ParamValues } {
  return params === undefined || Object.keys(params).length === 0 ? {} : { params };
}

/** A handler on a set of them, and the call that takes it off again. */
function watch<Payload>(set: Set<(payload: Payload) => void>, handler: (payload: Payload) => void): () => void {
  set.add(handler);
  return () => void set.delete(handler);
}

export function createClient(spawn: Spawn, assets: Assets): MotorClient {
  const state: State = {
    link: null, seq: 0, loaded: null, closed: false, waiting: new Map(), pushed: new Set(), failures: new Set(),
  };
  const linked = async (): Promise<Link> => {
    if (state.closed) throw new Error("the motor client is closed");
    const link = state.link ?? connect(state, spawn, assets);
    await link.ready;
    if (state.link !== link) throw new CancelledError();
    return link;
  };
  const call = async (body: Request): Promise<Result> => exchange(state, (await linked()).port, body);
  return {
    catalog: async () => (await linked()).ready,
    load: async (source) => {
      const graph = loaded(await call({ type: "load", source, fixturesUrl: assets.fixturesUrl }));
      state.loaded = source;
      return graph;
    },
    layout: async (layoutId, postId, params) => laidOut(await call({ type: "layout", layoutId, postId, ...asked(params) })),
    params: async (layoutId) => published(await call({ type: "params", layoutId }), layoutId),
    analysis: async (analysisId) => analysed(await call({ type: "analysis", analysisId })),
    cancel: () => cancelWaiting(state),
    busy: () => state.waiting.size > 0,
    force: (body) => fireAndForget(state, body),
    onForce: (handler) => watch(state.pushed, handler),
    onFail: (handler) => watch(state.failures, handler),
    close: () => {
      state.closed = true;
      drop(state);
    },
  };
}
