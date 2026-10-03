/**
 * Everything that decides what is on screen, as one frozen document: the graph's source,
 * the stages run over it, and how the result is drawn. A change is a new document, so a
 * reader holding the old one still holds what was true when it read it.
 *
 * Not in here: the camera, the selection, what is open. Those are the viewer's, not the
 * drawing's, and a recipe that carried them would replay someone else's scrolling.
 */
import { THEME_NAMES } from "../../../graph-render/src/look/themes.ts";
import type { SyntheticShape } from "../source/synthetic.ts";
import { type ParamValue, type ParamsByLayout, type ParamValues, paramsOf, valuesOf } from "./paramValues.ts";
import { SettingsRefusal } from "./read.ts";

export { SettingsRefusal };
export { readGroups, readSettings } from "./settingsRead.ts";
export { type ParamValue, type ParamsByLayout, type ParamValues };

export type Source =
  | { readonly kind: "synthetic"; readonly seed: number; readonly nodes: number; readonly degree: number; readonly shape: SyntheticShape }
  | { readonly kind: "fixture"; readonly path: string }
  | { readonly kind: "document"; readonly name: string; readonly text: string };

export const THEMES: readonly string[] = THEME_NAMES;
/**
 * `tag` and `db` colour by the two columns the provisional document gives every node;
 * `analysis` is the metric colouring — a number the motor measured, not a name. It keeps
 * the name `analysis` because the analysis is what puts a value in that slot.
 */
export const COLOUR_BY = ["group", "kind", "tag", "db", "analysis", "none"] as const;
export const SIZE_BY = ["weight", "degree", "analysis", "uniform"] as const;
export const LABEL_MODES = ["auto", "more", "none"] as const;
export const EDGE_STYLES = ["straight", "curve"] as const;
/**
 * How an edge takes its colour: `flat` is one stroke in the theme's colour, `gradient` runs
 * from the source node's colour to the target's, as the SciGraphs edge tubes do.
 */
export const EDGE_COLOURS = ["flat", "gradient"] as const;
export const NODE_SCALE = { min: 0.2, max: 5, whole: false } as const;
export const LINK_THICKNESS = { min: 0.1, max: 5, whole: false } as const;
export const TEXT_FADE = { min: -3, max: 3, whole: false } as const;
export const GLOW_STRENGTH = { min: 0, max: 3, whole: false } as const;
export const BACKGROUNDS = ["theme", "flat", "aurora"] as const;
/** Bounds of the node-size pixel range; the defaults are the radii the painter always clamped to. */
export const NODE_PX = { min: 0.5, max: 120, whole: false } as const;

export interface Appearance {
  readonly theme: string;
  readonly colourBy: (typeof COLOUR_BY)[number];
  readonly sizeBy: (typeof SIZE_BY)[number];
  /** Multiplies every node's radius. */
  readonly nodeScale: number;
  readonly labels: (typeof LABEL_MODES)[number];
  /** Draw a head on every directed edge; its size follows `linkThickness`. */
  readonly arrows: boolean;
  /** Where labels start to appear by zoom, -3 (early) to 3 (late). */
  readonly textFade: number;
  /** Multiplies every edge's stroke width. */
  readonly linkThickness: number;
  readonly edgeStyle: (typeof EDGE_STYLES)[number];
  readonly edgeColour: (typeof EDGE_COLOURS)[number];
  readonly glow: boolean;
  readonly glowStrength: number;
  /** `theme` paints the theme's own ground, `flat` one solid colour, `aurora` a gradient. */
  readonly background: (typeof BACKGROUNDS)[number];
  /** Smallest and largest drawn node radius in pixels; `minRadius <= maxRadius` always. */
  readonly minRadius: number;
  readonly maxRadius: number;
}

/** Filters hide nodes in the drawing. The layout still ran over the whole graph. */
export interface Filter {
  /** The query grammar; `""` matches every node. */
  readonly query: string;
  /** Free text over the label, case-insensitive substring. */
  readonly text: string;
  readonly hiddenKinds: readonly string[];
  readonly hiddenGroups: readonly string[];
  /** True hides every node with no link. */
  readonly orphans: boolean;
  /** True keeps only the nodes that touch a link. */
  readonly existingOnly: boolean;
  readonly minDegree: number;
  /** Ask the motor for a new layout when the filter changes. */
  readonly relayout: boolean;
}

/** A named query over the document, drawn in its own colour. The first match wins. */
export interface Group {
  readonly name: string;
  readonly query: string;
  readonly colour: string;
}

export interface Settings {
  readonly source: Source;
  readonly layout: string;
  /** A POST pass over the layout's edges, or `null` for the edges as the layout drew them. */
  readonly edges: string | null;
  readonly analysis: string | null;
  /** What each layout is run at. Not the schema: that is the motor's, read from the motor. */
  readonly params: ParamsByLayout;
  readonly appearance: Appearance;
  /** Ordered; the first group a node matches is the group it is drawn in. */
  readonly groups: readonly Group[];
  readonly filter: Filter;
}

function sourceOf(source: Source): Source {
  if (source.kind === "fixture") return Object.freeze({ kind: source.kind, path: source.path });
  if (source.kind === "document") return Object.freeze({ kind: source.kind, name: source.name, text: source.text });
  return Object.freeze({
    kind: source.kind, seed: source.seed, nodes: source.nodes, degree: source.degree, shape: source.shape,
  });
}

function appearanceOf(look: Appearance): Appearance {
  return Object.freeze({
    theme: look.theme, colourBy: look.colourBy, sizeBy: look.sizeBy, nodeScale: look.nodeScale, labels: look.labels,
    arrows: look.arrows, textFade: look.textFade, linkThickness: look.linkThickness, edgeStyle: look.edgeStyle,
    edgeColour: look.edgeColour, glow: look.glow, glowStrength: look.glowStrength,
    background: look.background, minRadius: look.minRadius, maxRadius: look.maxRadius,
  });
}

function filterOf(filter: Filter): Filter {
  return Object.freeze({
    query: filter.query,
    text: filter.text,
    hiddenKinds: Object.freeze([...filter.hiddenKinds]),
    hiddenGroups: Object.freeze([...filter.hiddenGroups]),
    orphans: filter.orphans,
    existingOnly: filter.existingOnly,
    minDegree: filter.minDegree,
    relayout: filter.relayout,
  });
}

/** A copy, so a list the caller still holds cannot change the document behind its back. */
export function groupsOf(groups: readonly Group[]): readonly Group[] {
  return Object.freeze(groups.map((group) => Object.freeze({
    name: group.name, query: group.query, colour: group.colour,
  })));
}

/** Members in one fixed order, so two equal documents are equal as text. */
export function settingsOf(settings: Settings): Settings {
  return Object.freeze({
    source: Object.isFrozen(settings.source) ? settings.source : sourceOf(settings.source),
    layout: settings.layout,
    edges: settings.edges,
    analysis: settings.analysis,
    params: Object.isFrozen(settings.params) ? settings.params : paramsOf(settings.params),
    appearance: Object.isFrozen(settings.appearance) ? settings.appearance : appearanceOf(settings.appearance),
    groups: Object.isFrozen(settings.groups) ? settings.groups : groupsOf(settings.groups),
    filter: Object.isFrozen(settings.filter) ? settings.filter : filterOf(settings.filter),
  });
}

export const OPENING_SOURCE: Extract<Source, { kind: "synthetic" }> = Object.freeze({
  kind: "synthetic", seed: 1, nodes: 400, degree: 2, shape: "vault",
});

export const DEFAULT_SETTINGS: Settings = settingsOf({
  source: OPENING_SOURCE,
  // Barnes-Hut, not the exact sum: same layout, repulsion over a quadtree, O(n log n) and a
  // 250 000-node ceiling against 14 000 (`docs/measurements/perf-fa2bh.md`). The exact
  // layout stays in the catalog and stays the escape hatch.
  layout: "layout.forceatlas2.barnes_hut",
  edges: null,
  analysis: null,
  // Every layout at the motor's own defaults: the published defaults reproduce the registered
  // run byte for byte, so an empty map and a map of defaults draw the same picture
  // (docs/decisions/layout-params.md).
  params: {},
  appearance: {
    theme: "dark", colourBy: "group", sizeBy: "weight", nodeScale: 1, labels: "auto",
    arrows: false, textFade: 0, linkThickness: 1, edgeStyle: "straight", edgeColour: "flat",
    glow: false, glowStrength: 1,
    background: "theme", minRadius: NODE_PX.min, maxRadius: NODE_PX.max,
  },
  groups: [],
  filter: {
    query: "", text: "", hiddenKinds: [], hiddenGroups: [],
    orphans: false, existingOnly: false, minDegree: 0, relayout: false,
  },
});

export function withSettings(settings: Settings, patch: Partial<Settings>): Settings {
  return settingsOf({ ...settings, ...patch });
}

export function withAppearance(settings: Settings, patch: Partial<Appearance>): Settings {
  return settingsOf({ ...settings, appearance: appearanceOf({ ...settings.appearance, ...patch }) });
}

export function withFilter(settings: Settings, patch: Partial<Filter>): Settings {
  return settingsOf({ ...settings, filter: filterOf({ ...settings.filter, ...patch }) });
}

export function withGroups(settings: Settings, groups: readonly Group[]): Settings {
  return settingsOf({ ...settings, groups: groupsOf(groups) });
}

/**
 * What one layout is run at, with `values` merged into what it is already run at. A layout
 * left with no values at all loses its entry, so a document says nothing about a layout that
 * is run at the defaults and two equal drawings have equal bytes.
 */
export function withParams(settings: Settings, layoutId: string, values: ParamValues): Settings {
  const held = valuesOf({ ...settings.params[layoutId], ...values });
  const params = Object.keys(held).length === 0
    ? withoutLayout(settings.params, layoutId)
    : paramsOf({ ...settings.params, [layoutId]: held });
  return settingsOf({ ...settings, params });
}

export function withoutParams(settings: Settings, layoutId: string): Settings {
  return settingsOf({ ...settings, params: withoutLayout(settings.params, layoutId) });
}

/**
 * What one layout is run at, with nothing of its own: the motor's defaults again. A fresh copy
 * without it, because a document that names a layout at the defaults says nothing the reader
 * needs, and this repository does not `delete` a computed key.
 */
function withoutLayout(params: ParamsByLayout, layoutId: string): ParamsByLayout {
  return paramsOf(Object.fromEntries(Object.entries(params).filter(([held]) => held !== layoutId)));
}

export function sameSettings(a: Settings, b: Settings): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
