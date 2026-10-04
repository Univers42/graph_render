/**
 * From settings to a drawing. `apply` does the least the change needs: a new source is
 * loaded and laid out, a new layout only laid out, a new look only restyled. Each part
 * writes its own members of the settings once it has succeeded, so the settings are what
 * is on screen and a recipe taken at any moment replays.
 */
import { frameFrom } from "../../../graph-render/src/frame.ts";
import { DEFAULT_POLICY, type LabelPolicy } from "../../../graph-render/src/labels.ts";
import { decodeSnapshot } from "../../../graph-render/src/snapshot/decode.ts";
import { styleFrom } from "../../../graph-render/src/style.ts";
import { backdropTheme } from "../../../graph-render/src/look/backdrop.ts";
import { isLightTheme, themeNamed } from "../../../graph-render/src/look/themes.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Outcome } from "../actions/registry.ts";
import { styleInputOf } from "../look/styleOf.ts";
import { CancelledError, type MotorClient } from "../motor/client.ts";
import type { AnalysisReport, GraphSummary, RunReport } from "../motor/protocol.ts";
import { MetaMismatch } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import { type Appearance, type ParamValues, type Settings, type Source, withSettings } from "../state/settings.ts";
import type { Store } from "../state/store.ts";
import { neighboursOf } from "./adjacency.ts";
import { fitResults } from "./fitResults.ts";
import { type Before, type Held, beforeOf, clear } from "./pipeline/clear.ts";
import { planOf } from "./plan.ts";
import { summaryOf } from "./runSummary.ts";
import { schemaOf } from "./schema.ts";

export type ViewFace = Pick<
  View,
  | "setFrame" | "setStyle" | "setTheme" | "setLabels"
  | "fit" | "reset" | "zoomBy" | "panBy" | "limits"
  | "focus" | "select" | "local" | "showAll" | "on" | "toPNG" | "setCamera" | "frame" | "viewport"
  | "hide" | "togglePin" | "pinned" | "selectMany"
  | "orbit" | "setOrbit" | "resetOrbit" | "projected"
>;

export interface Pipeline {
  /** Brings the drawing to `next`, asking the motor for what changed. */
  apply(next: Settings): Promise<Outcome>;
  /** The look and the filters of `next`; never asks the motor. */
  look(next: Settings): Outcome;
  /** Frames the camera on what the search highlighted. */
  fitResults(): Outcome;
  /** The snapshot that is drawn. */
  bytes(): Uint8Array | null;
  neighbours(node: number): readonly number[];
  /** Shows the first `count` nodes in ingest order, or all of them for null; not part of the settings. */
  reveal(count: number | null): void;
}

export interface PipelineDeps {
  readonly client: MotorClient;
  readonly view: ViewFace;
  readonly store: Store<StudioState>;
}

interface Rig extends PipelineDeps {
  held: Held | null;
  /** The look the view was last given; `null` before the first. */
  shown: Appearance | null;
  /**
   * The token of the newest `apply` call. A call that comes back from the motor with an older
   * token has been superseded, and says so rather than writing over the newer drawing.
   */
  generation: number;
  /** How many `apply` calls are between here and their outcome; what a `cancel` is for. */
  running: number;
}

interface Part {
  readonly message: string;
  readonly notes: readonly string[];
}

export const LABEL_POLICIES: Readonly<Record<Appearance["labels"], LabelPolicy>> = {
  auto: DEFAULT_POLICY,
  more: { threshold: 0.45, budget: 400 },
  none: { threshold: DEFAULT_POLICY.threshold, budget: 0 },
};

function policyOf(appearance: Appearance): LabelPolicy {
  return { ...LABEL_POLICIES[appearance.labels], fade: appearance.textFade };
}

const NOTES_SHOWN = 5;

/** The two ends of the motor round trip of a layout switch, for `deploy/perf/transition.py`. */
const REQUEST_MARK = "gm:transition:request";
const BYTES_MARK = "gm:transition:bytes";

function patch(rig: Rig, change: (state: StudioState) => Partial<StudioState>): void {
  rig.store.update((state) => ({ ...state, ...change(state) }));
}

function ms(value: number): string {
  return `${Math.round(value)} ms`;
}

export function firstOf(notes: readonly string[]): readonly string[] {
  if (notes.length <= NOTES_SHOWN) return notes;
  return [...notes.slice(0, NOTES_SHOWN), `… and ${notes.length - NOTES_SHOWN} more while reading the document`];
}

function restyle(rig: Rig, look: Settings): void {
  const { meta, analysis, reveal } = rig.store.get();
  if (meta === null) return;
  const { appearance, filter, groups } = look;
  rig.view.setStyle(styleFrom(styleInputOf({ meta, appearance, filter, groups, analysis, reveal })));
}

function showLook(rig: Rig, look: Settings): void {
  const { appearance, filter, groups } = look;
  if (rig.shown?.theme !== appearance.theme || rig.shown.background !== appearance.background) {
    rig.view.setTheme(backdropTheme(themeNamed(appearance.theme), appearance.background, isLightTheme(appearance.theme)));
  }
  if (rig.shown?.labels !== appearance.labels || rig.shown.textFade !== appearance.textFade) {
    rig.view.setLabels(policyOf(appearance));
  }
  rig.shown = appearance;
  patch(rig, (state) => ({ settings: withSettings(state.settings, { appearance, filter, groups }) }));
}

/**
 * WHY a token and not the motor's own `busy()`: `busy()` is false between a request going out
 * and the reply coming back — while the worker starts, and while this module patches and draws —
 * so a call made inside a `graph-load` handler re-entered here with a graph already on screen and
 * nothing to cancel. The token is this call's own: newer or older, on every path.
 */
function guard(rig: Rig, token: number): void {
  if (rig.generation !== token) throw new CancelledError();
}

async function load(rig: Rig, token: number, source: Source): Promise<Part> {
  const graph: GraphSummary = await rig.client.load(source);
  guard(rig, token);
  patch(rig, (state) => ({ graph, analysis: null, reveal: null, settings: withSettings(state.settings, { source }) }));
  return { message: `${graph.name}: ${graph.nodeCount} nodes, ${graph.edgeCount} links`, notes: firstOf(graph.notes) };
}

function draw(rig: Rig, token: number, run: RunReport, shown: { readonly look: Settings; readonly fresh: boolean }): Part {
  guard(rig, token);
  const snapshot = decodeSnapshot(run.bytes);
  const frame = frameFrom(snapshot);
  const meta = run.meta ?? rig.store.get().meta;
  if (meta === null || meta.nodeCount !== frame.nodeCount) {
    throw new MetaMismatch(`the snapshot holds ${frame.nodeCount} nodes and came with no description of them`);
  }
  const summary = summaryOf(run, snapshot);
  rig.held = { bytes: run.bytes, ends: frame };
  rig.view.setFrame(frame, { animate: !shown.fresh });
  if (shown.fresh) rig.view.select(-1);
  patch(rig, (state) => ({
    meta, run: summary, selected: shown.fresh ? -1 : state.selected, selection: shown.fresh ? [] : state.selection,
    // The values are written here, with the layout: they are what this run was made at, and
    // nothing else in the pipeline writes a member of the settings that a run settles.
    settings: withSettings(state.settings, { layout: run.layoutId, edges: run.postId, params: shown.look.params }),
    // The filter the drawing was made under, and the only place it is written: the count
    // below is what a `relayout` filter is compared against to know it has already run.
    runFilter: JSON.stringify(shown.look.filter),
    // The values, the same way: what the motor was actually run at, which is not what was asked
    // for when a force layout on a large graph ran as a scatter instead.
    runParams: JSON.stringify(run.params),
  }));
  restyle(rig, shown.look);
  const pass = run.postId === null ? "" : ` + ${run.postId} ${ms(run.postMs)}`;
  return { message: `${run.layoutId} ${ms(run.layoutMs)}${pass}`, notes: summary.notes };
}

async function arrange(rig: Rig, token: number, next: Settings, shown: { readonly fresh: boolean; readonly before: Before }): Promise<Part> {
  const { fresh } = shown;
  // Counted before the await: a run that is cancelled while it waits was still asked for,
  // and a count that only moved on success would hide that from the studio's own tests.
  patch(rig, (state) => ({ layoutCalls: state.layoutCalls + 1 }));
  // A switch between two layouts of the same graph is the one this measures; the marks the
  // render side puts down are `gm:transition:moved` and `gm:transition:settled`.
  if (!fresh) performance.mark(REQUEST_MARK);
  const asked: ParamValues = next.params[next.layout] ?? {};
  try {
    const run = await rig.client.layout(next.layout, next.edges, asked);
    if (!fresh) performance.mark(BYTES_MARK);
    const part = draw(rig, token, run, { look: next, fresh });
    return { message: part.message, notes: [...part.notes, ...await schemaOf(rig.client, rig.store, run.layoutId)] };
  } catch (error) {
    // After a load the old drawing is of another graph; after a refused layout it still holds.
    // A superseded call clears nothing: the drawing on screen is the newer call's.
    if (fresh && rig.generation === token) clear(rig, shown.before);
    throw error;
  }
}

function measured(analysis: AnalysisReport): string[] {
  const notes: string[] = [];
  if (analysis.converged === false) notes.push("did not converge: the values are the last iterate, not the fixed point");
  if (analysis.modularity !== null) notes.push(`modularity ${analysis.modularity.toFixed(4)}`);
  if (analysis.max !== null) notes.push(`largest value ${analysis.max}`);
  return notes;
}

function forget(rig: Rig): void {
  patch(rig, (state) => ({ analysis: null, settings: withSettings(state.settings, { analysis: null }) }));
}

async function measure(rig: Rig, token: number, next: Settings): Promise<Part> {
  if (next.analysis === null) {
    forget(rig);
    restyle(rig, next);
    return { message: "analysis off", notes: [] };
  }
  try {
    const analysis = await rig.client.analysis(next.analysis);
    guard(rig, token);
    patch(rig, (state) => ({ analysis, settings: withSettings(state.settings, { analysis: analysis.id }) }));
    restyle(rig, next);
    return { message: `${analysis.id} ${ms(analysis.ms)}`, notes: measured(analysis) };
  } catch (error) {
    // A superseded call leaves the newer call's analysis on the state; its own is forgotten.
    if (rig.generation === token) {
      forget(rig);
      restyle(rig, rig.store.get().settings);
    }
    throw error;
  }
}

/**
 * The call that takes the drawing. Every commit below carries the token this call took, so a
 * newer call — from a second `loadGraph`, or from inside a `graph-load` handler — owns the frame
 * and this one rejects with a `CancelledError` instead of writing over it.
 */
async function apply(rig: Rig, next: Settings): Promise<Outcome> {
  const token = rig.generation + 1;
  rig.generation = token;
  rig.running += 1;
  try {
    return await drawOut(rig, token, next);
  } finally { rig.running -= 1; }
}

async function drawOut(rig: Rig, token: number, next: Settings): Promise<Outcome> {
  const plan = planOf(rig.store.get(), next);
  // Taken before the load is asked for, and used only if it fails: what the store carried then is
  // what a failed fresh load is rolled back to (`pipeline/clear.ts`).
  const before = beforeOf(rig.store.get());
  const parts: Part[] = [];
  // The latest request wins: what the motor is doing is for a drawing nobody waits for now.
  // Cancelled only when this studio's own older call is still in flight, so a newer call is
  // never rejected by an older one's cancel.
  if ((plan.load || plan.layout || plan.analysis) && rig.running > 0) rig.client.cancel();
  if (plan.load) parts.push(await load(rig, token, next.source));
  if (plan.layout) parts.push(await arrange(rig, token, next, { fresh: plan.load, before }));
  if (plan.analysis) parts.push(await measure(rig, token, next));
  // One turn of the queue before the answer. A host that calls `loadGraph` from inside the
  // `graph-load` handler it was just given re-enters on the next turn, not inside this frame, and
  // the token is only raised once that call reaches `apply`: without the turn this call would
  // report the counts of a frame the host had already taken back.
  await Promise.resolve();
  guard(rig, token);
  showLook(rig, next);
  if (parts.length === 0) restyle(rig, next);
  return {
    message: parts.length === 0 ? "already drawn" : parts.map((part) => part.message).join(" · "),
    digest: rig.store.get().run?.digest ?? null,
    notes: parts.flatMap((part) => part.notes),
  };
}

export function createPipeline(deps: PipelineDeps): Pipeline {
  const rig: Rig = { ...deps, held: null, shown: null, generation: 0, running: 0 };
  return {
    apply: (next) => apply(rig, next),
    look: (next) => {
      showLook(rig, next);
      restyle(rig, next);
      return { message: "restyled" };
    },
    fitResults: () => fitResults(rig.view, rig.store.get()),
    reveal: (count) => {
      patch(rig, () => ({ reveal: count }));
      restyle(rig, rig.store.get().settings);
    },
    bytes: () => rig.held?.bytes ?? null,
    neighbours: (node) => (rig.held === null ? [] : neighboursOf(rig.held.ends, node)),
  };
}
