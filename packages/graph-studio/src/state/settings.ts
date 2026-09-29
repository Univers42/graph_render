/**
 * Everything that decides what is on screen, as one frozen document: the graph's source,
 * the stages run over it, and how the result is drawn. A change is a new document, so a
 * reader holding the old one still holds what was true when it read it.
 *
 * Not in here: the camera, the selection, what is open. Those are the viewer's, not the
 * drawing's, and a recipe that carried them would replay someone else's scrolling.
 */
import { THEME_NAMES } from "../../../graph-render/src/look/themes.ts";
import { MAX_DEGREE, MAX_NODES, SHAPES, type SyntheticShape } from "../source/synthetic.ts";
import { type Fields, SettingsRefusal, fieldsOf, flagOf, numberOf, oneOf, textOf, textOrNull, textsOf } from "./read.ts";

export { SettingsRefusal };

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
    glow: look.glow, glowStrength: look.glowStrength,
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
function settingsOf(settings: Settings): Settings {
  return Object.freeze({
    source: Object.isFrozen(settings.source) ? settings.source : sourceOf(settings.source),
    layout: settings.layout,
    edges: settings.edges,
    analysis: settings.analysis,
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
  layout: "layout.forceatlas2",
  edges: null,
  analysis: null,
  appearance: {
    theme: "dark", colourBy: "group", sizeBy: "weight", nodeScale: 1, labels: "auto",
    arrows: false, textFade: 0, linkThickness: 1, edgeStyle: "straight", glow: false, glowStrength: 1,
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

export function sameSettings(a: Settings, b: Settings): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function readSource(value: unknown, at: string): Source {
  const kind = oneOf(fieldsOf(value, at, ["kind", "seed", "nodes", "degree", "shape", "path", "name", "text"]), at, "kind", ["synthetic", "fixture", "document"]);
  if (kind === "fixture") return { kind, path: textOf(fieldsOf(value, at, ["kind", "path"]), at, "path") };
  if (kind === "document") {
    const fields = fieldsOf(value, at, ["kind", "name", "text"]);
    return { kind, name: textOf(fields, at, "name"), text: textOf(fields, at, "text") };
  }
  const fields = fieldsOf(value, at, ["kind", "seed", "nodes", "degree", "shape"]);
  return {
    kind,
    seed: numberOf(fields, at, "seed", { min: 0, max: 4294967295, whole: true }),
    nodes: numberOf(fields, at, "nodes", { min: 2, max: MAX_NODES, whole: true }),
    degree: numberOf(fields, at, "degree", { min: 0, max: MAX_DEGREE, whole: true }),
    shape: oneOf(fields, at, "shape", SHAPES),
  };
}

function readAppearance(value: unknown, at: string): Appearance {
  const fields = fieldsOf(value, at, [
    "theme", "colourBy", "sizeBy", "nodeScale", "labels", "arrows", "textFade", "linkThickness", "edgeStyle", "glow", "glowStrength",
    "background", "minRadius", "maxRadius",
  ]);
  return {
    theme: oneOf(fields, at, "theme", THEMES),
    colourBy: oneOf(fields, at, "colourBy", COLOUR_BY),
    sizeBy: oneOf(fields, at, "sizeBy", SIZE_BY),
    nodeScale: numberOf(fields, at, "nodeScale", NODE_SCALE),
    labels: oneOf(fields, at, "labels", LABEL_MODES),
    arrows: flagOf(fields, at, "arrows"),
    textFade: numberOf(fields, at, "textFade", TEXT_FADE),
    linkThickness: numberOf(fields, at, "linkThickness", LINK_THICKNESS),
    edgeStyle: oneOf(fields, at, "edgeStyle", EDGE_STYLES),
    glow: flagOf(fields, at, "glow"),
    glowStrength: numberOf(fields, at, "glowStrength", GLOW_STRENGTH),
    background: oneOf(fields, at, "background", BACKGROUNDS),
    ...readRadii(fields, at),
  };
}

function readRadii(fields: Fields, at: string): Pick<Appearance, "minRadius" | "maxRadius"> {
  const minRadius = numberOf(fields, at, "minRadius", NODE_PX);
  const maxRadius = numberOf(fields, at, "maxRadius", NODE_PX);
  if (minRadius > maxRadius) throw new SettingsRefusal(`${at}.minRadius`, `above maxRadius (${maxRadius})`);
  return { minRadius, maxRadius };
}

function readFilter(value: unknown, at: string): Filter {
  const fields = fieldsOf(value, at, [
    "query", "text", "hiddenKinds", "hiddenGroups", "orphans", "existingOnly", "minDegree", "relayout",
  ]);
  return {
    query: textOf(fields, at, "query"),
    text: textOf(fields, at, "text"),
    hiddenKinds: textsOf(fields, at, "hiddenKinds"),
    hiddenGroups: textsOf(fields, at, "hiddenGroups"),
    orphans: flagOf(fields, at, "orphans"),
    existingOnly: flagOf(fields, at, "existingOnly"),
    minDegree: numberOf(fields, at, "minDegree", { min: 0, max: 4294967295, whole: true }),
    relayout: flagOf(fields, at, "relayout"),
  };
}

function readGroup(value: unknown, at: string): Group {
  const fields = fieldsOf(value, at, ["name", "query", "colour"]);
  return { name: textOf(fields, at, "name"), query: textOf(fields, at, "query"), colour: textOf(fields, at, "colour") };
}

/** The document's own list of groups, in the order the document gave them. */
export function readGroups(value: unknown, at = "settings.groups"): readonly Group[] {
  if (!Array.isArray(value)) throw new SettingsRefusal(at, "not a list");
  return groupsOf(value.map((group, i) => readGroup(group, `${at}[${i}]`)));
}

/** Settings from outside the studio, or a refusal naming the member that was wrong. */
export function readSettings(value: unknown, at = "settings"): Settings {
  const fields: Fields = fieldsOf(value, at, ["source", "layout", "edges", "analysis", "appearance", "groups", "filter"]);
  return settingsOf({
    source: readSource(fields["source"], `${at}.source`),
    layout: textOf(fields, at, "layout"),
    edges: textOrNull(fields, at, "edges"),
    analysis: textOrNull(fields, at, "analysis"),
    appearance: readAppearance(fields["appearance"], `${at}.appearance`),
    groups: readGroups(fields["groups"], `${at}.groups`),
    filter: readFilter(fields["filter"], `${at}.filter`),
  });
}
