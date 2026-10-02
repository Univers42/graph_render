/**
 * From settings to a drawing. `apply` does the least the change needs: a new source is
 * loaded and laid out, a new layout only laid out, a new look only restyled. Each part
 * writes its own members of the settings once it has succeeded, so the settings are what
 * is on screen and a recipe taken at any moment replays.
 */
import { frameFrom } from "../../../graph-render/src/frame.ts";
import { DEFAULT_POLICY, type LabelPolicy } from "../../../graph-render/src/labels.ts";
import { EMPTY_FRAME } from "../../../graph-render/src/scene.ts";
import { type Snapshot, decodeSnapshot } from "../../../graph-render/src/snapshot/decode.ts";
import { styleFrom } from "../../../graph-render/src/style.ts";
import { backdropTheme } from "../../../graph-render/src/look/backdrop.ts";
import { isLightTheme, themeNamed } from "../../../graph-render/src/look/themes.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Outcome } from "../actions/registry.ts";
import { styleInputOf } from "../look/styleOf.ts";
import type { MotorClient } from "../motor/client.ts";
import type { AnalysisReport, GraphSummary, RunReport } from "../motor/protocol.ts";
import { type Ends, MetaMismatch } from "../source/meta.ts";
import type { RunSummary, StudioState } from "../state/model.ts";
import { type Appearance, type Settings, type Source, withSettings } from "../state/settings.ts";
import type { Store } from "../state/store.ts";
import { neighboursOf } from "./adjacency.ts";
import { fitResults } from "./fitResults.ts";

export type ViewFace = Pick<
  View,
  | "setFrame" | "setStyle" | "setTheme" | "setLabels"
  | "fit" | "reset" | "zoomBy" | "panBy" | "limits"
  | "focus" | "select" | "local" | "showAll" | "on" | "toPNG" | "setCamera" | "frame" | "viewport"
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
  held: { readonly bytes: Uint8Array; readonly ends: Ends } | null;
  /** The look the view was last given; `null` before the first. */
  shown: Appearance | null;
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

function patch(rig: Rig, change: (state: StudioState) => Partial<StudioState>): void {
  rig.store.update((state) => ({ ...state, ...change(state) }));
}

function ms(value: number): string {
  return `${Math.round(value)} ms`;
}

function firstOf(notes: readonly string[]): readonly string[] {
  if (notes.length <= NOTES_SHOWN) return notes;
  return [...notes.slice(0, NOTES_SHOWN), `… and ${notes.length - NOTES_SHOWN} more while reading the document`];
}

function sameSource(a: Source, b: Source): boolean {
  return a === b || (a.kind === b.kind && JSON.stringify(a) === JSON.stringify(b));
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

function clear(rig: Rig): void {
  rig.held = null;
  rig.view.setFrame(EMPTY_FRAME);
  patch(rig, () => ({ meta: null, run: null, selected: -1, selection: [], reveal: null }));
}

async function load(rig: Rig, source: Source): Promise<Part> {
  const graph: GraphSummary = await rig.client.load(source);
  patch(rig, (state) => ({ graph, analysis: null, reveal: null, settings: withSettings(state.settings, { source }) }));
  return { message: `${graph.name}: ${graph.nodeCount} nodes, ${graph.edgeCount} links`, notes: firstOf(graph.notes) };
}

function degradations(snapshot: Snapshot): string[] {
  const counts = new Map<string, number>();
  for (const note of snapshot.notes) {
    const name = note.name ?? `note ${note.code}`;
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return [...counts].map(([name, count]) => (count === 1 ? name : `${name} ×${count}`));
}

function summaryOf(run: RunReport, snapshot: Snapshot): RunSummary {
  const refused = run.postError === null ? [] : [`${run.postError.title}: ${run.postError.detail} — the layout's own edges are shown`];
  return {
    layoutId: run.layoutId, postId: run.postId, postError: run.postError, digest: run.digest,
    byteLength: run.bytes.byteLength, nodeKind: snapshot.nodeKind, edgeKind: snapshot.edgeKind,
    // The dim off the decoded snapshot, not off the layout id: the z column's presence is
    // what the painter branches on, so that is what the badge has to report.
    dim: snapshot.dim,
    layoutMs: run.layoutMs, postMs: run.postMs, notes: [...degradations(snapshot), ...refused],
  };
}

function draw(rig: Rig, run: RunReport, shown: { readonly look: Settings; readonly fresh: boolean }): Part {
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
    settings: withSettings(state.settings, { layout: run.layoutId, edges: run.postId }),
    // The filter the drawing was made under, and the only place it is written: the count
    // below is what a `relayout` filter is compared against to know it has already run.
    runFilter: JSON.stringify(shown.look.filter),
  }));
  restyle(rig, shown.look);
  const pass = run.postId === null ? "" : ` + ${run.postId} ${ms(run.postMs)}`;
  return { message: `${run.layoutId} ${ms(run.layoutMs)}${pass}`, notes: summary.notes };
}

async function arrange(rig: Rig, next: Settings, fresh: boolean): Promise<Part> {
  // Counted before the await: a run that is cancelled while it waits was still asked for,
  // and a count that only moved on success would hide that from the studio's own tests.
  patch(rig, (state) => ({ layoutCalls: state.layoutCalls + 1 }));
  try {
    const run = await rig.client.layout(next.layout, next.edges);
    return draw(rig, run, { look: next, fresh });
  } catch (error) {
    // After a load the old drawing is of another graph; after a refused layout it still holds.
    if (fresh) clear(rig);
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

async function measure(rig: Rig, next: Settings): Promise<Part> {
  if (next.analysis === null) {
    forget(rig);
    restyle(rig, next);
    return { message: "analysis off", notes: [] };
  }
  try {
    const analysis = await rig.client.analysis(next.analysis);
    patch(rig, (state) => ({ analysis, settings: withSettings(state.settings, { analysis: analysis.id }) }));
    restyle(rig, next);
    return { message: `${analysis.id} ${ms(analysis.ms)}`, notes: measured(analysis) };
  } catch (error) {
    forget(rig);
    restyle(rig, rig.store.get().settings);
    throw error;
  }
}

interface Plan {
  readonly load: boolean;
  readonly layout: boolean;
  readonly analysis: boolean;
}

function planOf(state: StudioState, next: Settings): Plan {
  const load = state.graph === null || !sameSource(state.settings.source, next.source);
  const { run } = state;
  // A filter that asks to be laid out again is one the last run was not made under, or the
  // layout would repeat the drawing already on screen for a filter nobody changed.
  const relayout = next.filter.relayout && JSON.stringify(next.filter) !== state.runFilter;
  const layout = load || run === null || run.layoutId !== next.layout || run.postId !== next.edges || relayout;
  const asked = next.analysis !== null && (load || state.analysis?.id !== next.analysis);
  return { load, layout, analysis: asked || (next.analysis === null && state.analysis !== null) };
}

async function apply(rig: Rig, next: Settings): Promise<Outcome> {
  const plan = planOf(rig.store.get(), next);
  const parts: Part[] = [];
  // The latest request wins: what the motor is doing is for a drawing nobody waits for now.
  if ((plan.load || plan.layout || plan.analysis) && rig.client.busy()) rig.client.cancel();
  if (plan.load) parts.push(await load(rig, next.source));
  if (plan.layout) parts.push(await arrange(rig, next, plan.load));
  if (plan.analysis) parts.push(await measure(rig, next));
  showLook(rig, next);
  if (parts.length === 0) restyle(rig, next);
  return {
    message: parts.length === 0 ? "already drawn" : parts.map((part) => part.message).join(" · "),
    digest: rig.store.get().run?.digest ?? null,
    notes: parts.flatMap((part) => part.notes),
  };
}

export function createPipeline(deps: PipelineDeps): Pipeline {
  const rig: Rig = { ...deps, held: null, shown: null };
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
