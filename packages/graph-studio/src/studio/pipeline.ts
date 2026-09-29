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
import { DARK_THEME, LIGHT_THEME, type Theme } from "../../../graph-render/src/theme.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Outcome } from "../actions/registry.ts";
import { styleInputOf } from "../look/styleOf.ts";
import type { MotorClient } from "../motor/client.ts";
import type { AnalysisReport, GraphSummary, RunReport } from "../motor/protocol.ts";
import { type Ends, MetaMismatch } from "../source/meta.ts";
import type { RunSummary, StudioState } from "../state/model.ts";
import { type Appearance, type Settings, type Source, withSettings } from "../state/settings.ts";
import type { Store } from "../state/store.ts";

export type ViewFace = Pick<
  View,
  "setFrame" | "setStyle" | "setTheme" | "setLabels" | "fit" | "zoomBy" | "focus" | "select" | "on" | "toPNG"
>;

export interface Pipeline {
  /** Brings the drawing to `next`, asking the motor for what changed. */
  apply(next: Settings): Promise<Outcome>;
  /** The look and the filters of `next`; never asks the motor. */
  look(next: Settings): Outcome;
  /** The snapshot that is drawn. */
  bytes(): Uint8Array | null;
  neighbours(node: number): readonly number[];
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

const THEMES: Readonly<Record<Appearance["theme"], Theme>> = { dark: DARK_THEME, light: LIGHT_THEME };

export const LABEL_POLICIES: Readonly<Record<Appearance["labels"], LabelPolicy>> = {
  auto: DEFAULT_POLICY,
  more: { threshold: 0.45, budget: 400 },
  none: { threshold: DEFAULT_POLICY.threshold, budget: 0 },
};

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
  const { meta, analysis } = rig.store.get();
  if (meta === null) return;
  rig.view.setStyle(styleFrom(styleInputOf({ meta, appearance: look.appearance, filter: look.filter, analysis })));
}

function showLook(rig: Rig, look: Settings): void {
  const { appearance, filter } = look;
  if (rig.shown?.theme !== appearance.theme) rig.view.setTheme(THEMES[appearance.theme]);
  if (rig.shown?.labels !== appearance.labels) rig.view.setLabels(LABEL_POLICIES[appearance.labels]);
  rig.shown = appearance;
  patch(rig, (state) => ({ settings: withSettings(state.settings, { appearance, filter }) }));
}

function clear(rig: Rig): void {
  rig.held = null;
  rig.view.setFrame(EMPTY_FRAME);
  patch(rig, () => ({ meta: null, run: null, selected: -1 }));
}

async function load(rig: Rig, source: Source): Promise<Part> {
  const graph: GraphSummary = await rig.client.load(source);
  patch(rig, (state) => ({ graph, analysis: null, settings: withSettings(state.settings, { source }) }));
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
    meta, run: summary, selected: shown.fresh ? -1 : state.selected,
    settings: withSettings(state.settings, { layout: run.layoutId, edges: run.postId }),
  }));
  restyle(rig, shown.look);
  const pass = run.postId === null ? "" : ` + ${run.postId} ${ms(run.postMs)}`;
  return { message: `${run.layoutId} ${ms(run.layoutMs)}${pass}`, notes: summary.notes };
}

async function arrange(rig: Rig, next: Settings, fresh: boolean): Promise<Part> {
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
  const layout = load || run === null || run.layoutId !== next.layout || run.postId !== next.edges;
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

function neighboursOf(ends: Ends, node: number): readonly number[] {
  const found = new Set<number>();
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    if (s === node && t !== node) found.add(t);
    if (t === node && s !== node) found.add(s);
  }
  return [...found];
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
    bytes: () => rig.held?.bytes ?? null,
    neighbours: (node) => (rig.held === null ? [] : neighboursOf(rig.held.ends, node)),
  };
}
