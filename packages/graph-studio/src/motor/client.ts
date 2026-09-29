/**
 * The studio's side of the conversation: promises over a port.
 *
 * A layout cannot be interrupted from inside, so stopping one closes the worker. The next
 * call starts a new one and gives it the graph that was last loaded, which costs one
 * module start and one build; the studio's state never sees the gap.
 */
import type { ShownError } from "../state/errors.ts";
import type { Source } from "../state/settings.ts";
import type {
  AnalysisReport, Assets, Catalog, Envelope, GraphSummary, Port, Request, Result, RunReport, Spawn,
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
  layout(layoutId: string, postId: string | null): Promise<RunReport>;
  analysis(analysisId: string): Promise<AnalysisReport>;
  /** Stops what is running. False when nothing was. */
  cancel(): boolean;
  busy(): boolean;
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
  const opened = await exchange(state, port, { type: "open", wasmUrl: assets.wasmUrl });
  if (opened.type !== "opened") throw mismatch("open", opened);
  if (state.loaded !== null) {
    await exchange(state, port, { type: "load", source: state.loaded, fixturesUrl: assets.fixturesUrl });
  }
  return opened.catalog;
}

function connect(state: State, spawn: Spawn, assets: Assets): Link {
  const port = spawn();
  port.listen((message) => {
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

export function createClient(spawn: Spawn, assets: Assets): MotorClient {
  const state: State = { link: null, seq: 0, loaded: null, closed: false, waiting: new Map() };
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
      const result = await call({ type: "load", source, fixturesUrl: assets.fixturesUrl });
      if (result.type !== "loaded") throw mismatch("load", result);
      state.loaded = source;
      return result.graph;
    },
    layout: async (layoutId, postId) => {
      const result = await call({ type: "layout", layoutId, postId });
      if (result.type !== "laid-out") throw mismatch("layout", result);
      return result.run;
    },
    analysis: async (analysisId) => {
      const result = await call({ type: "analysis", analysisId });
      if (result.type !== "analysed") throw mismatch("analysis", result);
      return result.analysis;
    },
    cancel: () => {
      if (state.waiting.size === 0) return false;
      drop(state);
      return true;
    },
    busy: () => state.waiting.size > 0,
    close: () => {
      state.closed = true;
      drop(state);
    },
  };
}
