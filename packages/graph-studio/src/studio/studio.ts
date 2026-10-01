/**
 * The studio without its chrome: a store, the actions, and one way to run them. The dock,
 * the console, a shortcut and a host all call `dispatch`, which never throws: what went
 * wrong is in the entry it returns and in the log.
 */
import { studioActions } from "../actions/all.ts";
import type { ForceLink } from "../actions/forces.ts";
import type { Save, StudioContext } from "../actions/context.ts";
import { type Args, type Outcome, type RawArgs, type Registry, type Resolved, createRegistry } from "../actions/registry.ts";
import { formatCommand, parseCommand } from "../console/parse.ts";
import { type MotorClient, MotorFailure } from "../motor/client.ts";
import { type ShownError, describeError } from "../state/errors.ts";
import { type LogEntry, type StudioState, initialState, withEntry } from "../state/model.ts";
import type { Settings, Source } from "../state/settings.ts";
import { type SettingsStorage, keepSettings, recall } from "../state/persist.ts";
import { type Store, createStore } from "../state/store.ts";
import { type ViewFace, createPipeline } from "./pipeline.ts";
import { createReveal } from "./reveal.ts";

export interface StudioDeps {
  readonly client: MotorClient;
  readonly view: ViewFace;
  readonly save: Save;
  readonly now: () => number;
  /** The live simulation behind the forces actions; nothing live when left out. */
  readonly forces?: ForceLink;
  readonly settings?: Settings;
  /** Where settings are kept per source; absent means nothing is remembered. */
  readonly storage?: SettingsStorage;
}

export interface Studio {
  readonly store: Store<StudioState>;
  readonly registry: Registry<StudioState, StudioContext>;
  dispatch(idOrAlias: string, raw?: RawArgs): Promise<LogEntry>;
  /** A line as typed in the console. */
  run(text: string): Promise<LogEntry>;
  /** Opens the motor and draws the settings' source. */
  start(): Promise<LogEntry>;
  neighbours(node: number): readonly number[];
  /** Keeps the text in the state, and offers it to the system clipboard where that is allowed. */
  copy(text: string): void;
  dismiss(): void;
  destroy(): void;
}

interface Desk {
  readonly deps: StudioDeps;
  readonly store: Store<StudioState>;
  readonly registry: Registry<StudioState, StudioContext>;
  readonly context: StudioContext;
  seq: number;
}

interface Started {
  readonly seq: number;
  readonly command: string;
  readonly at: number;
}

const LONG_VALUE = 96;

/** A document pasted as a value would bury the log: the line keeps how it starts. */
function shortened(args: Args): Args {
  return Object.fromEntries(Object.entries(args).map(([name, value]) => {
    const long = typeof value === "string" && value.length > LONG_VALUE;
    return [name, long ? `${value.slice(0, 40)}… (${value.length} characters)` : value];
  }));
}

function sourceCommand(source: Source): readonly [string, RawArgs] {
  if (source.kind === "fixture") return ["source.fixture", { path: source.path }];
  if (source.kind === "document") return ["source.document", { name: source.name, text: source.text }];
  return ["source.synthetic", { nodes: source.nodes, degree: source.degree, seed: source.seed, shape: source.shape }];
}

/** What the worker described is kept as it was: the wire code does not survive as a member. */
function shownOf(error: Error): ShownError {
  return error instanceof MotorFailure ? error.shown : describeError(error);
}

function finish(desk: Desk, started: Started, outcome: Outcome | Error): LogEntry {
  const failed = outcome instanceof Error;
  const error = failed ? shownOf(outcome) : null;
  const entry: LogEntry = {
    seq: started.seq, command: started.command, ok: !failed, ms: desk.deps.now() - started.at,
    message: outcome.message,
    digest: failed ? null : (outcome.digest ?? null),
    notes: failed ? [] : (outcome.notes ?? []),
    error,
  };
  // A run that was stopped was stopped on purpose: logged, and not raised as a failure.
  const raised = error !== null && error.title !== "CancelledError" ? error : null;
  desk.store.update((state) => withEntry({
    ...state, error: raised ?? (failed ? state.error : null), busy: state.busy.filter((running) => running.seq !== started.seq),
  }, entry));
  return entry;
}

async function execute(desk: Desk, label: string, resolve: () => Resolved<StudioState, StudioContext>): Promise<LogEntry> {
  desk.seq += 1;
  const started = { seq: desk.seq, command: label, at: desk.deps.now() };
  try {
    const { action, args } = resolve();
    const named = { ...started, command: formatCommand(action, shortened(args)) };
    const running = action.run(desk.context, args);
    if (!(running instanceof Promise)) return finish(desk, named, running);
    desk.store.update((state) => ({ ...state, busy: [...state.busy, { seq: named.seq, command: named.command }] }));
    return finish(desk, named, await running.catch((error: unknown) => errorOf(error)));
  } catch (error) {
    return finish(desk, started, errorOf(error));
  }
}

function errorOf(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

async function start(desk: Desk): Promise<LogEntry> {
  try {
    const catalog = await desk.deps.client.catalog();
    desk.store.update((state) => ({ ...state, catalog }));
  } catch (error) {
    desk.seq += 1;
    return finish(desk, { seq: desk.seq, command: "open", at: desk.deps.now() }, errorOf(error));
  }
  const [id, raw] = sourceCommand(desk.store.get().settings.source);
  return execute(desk, id, () => desk.registry.resolve(id, raw, desk.store.get()));
}

/**
 * Ponytail: the system clipboard is written without waiting and without reporting a refusal
 * (no permission, an insecure page, headless): the state copy is what a caller can rely on.
 */
function copyText(store: Store<StudioState>, text: string): void {
  store.update((state) => ({ ...state, clipboard: text }));
  try {
    void globalThis.navigator.clipboard.writeText(text).catch(() => undefined);
  } catch {
    // The state holds the text; there is nothing else to do.
  }
}

function contextOf(deps: StudioDeps, store: Store<StudioState>, registry: () => Registry<StudioState, StudioContext>): StudioContext {
  const pipeline = createPipeline({ client: deps.client, view: deps.view, store });
  return {
    ...pipeline,
    animation: createReveal({
      total: () => store.get().meta?.nodeCount ?? 0, show: (count) => pipeline.reveal(count), now: deps.now,
      schedule: (step, ms) => {
        const timer = setTimeout(step, ms);
        return () => clearTimeout(timer);
      },
    }),
    state: store.get,
    view: deps.view,
    stop: () => deps.client.cancel(),
    save: deps.save,
    clearLog: () => store.update((state) => ({ ...state, log: [] })),
    actions: () => registry().actions,
    recall: (source) => (deps.storage === undefined ? null : recall(deps.storage, source)),
  };
}

export function createStudio(deps: StudioDeps): Studio {
  const store = createStore(initialState(deps.settings));
  const registry = createRegistry<StudioState, StudioContext>(studioActions(deps.forces));
  const context = contextOf(deps, store, () => registry);
  const desk: Desk = { deps, store, registry, context, seq: 0 };
  const unkeep = deps.storage === undefined ? () => undefined : keepSettings(store, deps.storage);
  const unselect = deps.view.on("select", (selected) => store.update((state) => ({ ...state, selected })));
  const unselectMany = deps.view.on("selection", (selection) => store.update((state) => ({ ...state, selection })));
  return {
    store,
    registry,
    dispatch: (idOrAlias, raw = {}) => execute(desk, idOrAlias, () => registry.resolve(idOrAlias, raw, store.get())),
    run: (text) => execute(desk, text.trim(), () => {
      const command = parseCommand(text, registry);
      return registry.resolve(command.id, command.raw, store.get());
    }),
    start: () => start(desk),
    neighbours: (node) => context.neighbours(node),
    copy: (text) => copyText(store, text),
    dismiss: () => store.update((state) => ({ ...state, error: null })),
    destroy: () => {
      unselect();
      unkeep();
      unselectMany();
      deps.client.close();
    },
  };
}
