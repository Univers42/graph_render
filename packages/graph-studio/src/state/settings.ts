/**
 * Everything that decides what is on screen, as one frozen document: the graph's source,
 * the stages run over it, and how the result is drawn. A change is a new document, so a
 * reader holding the old one still holds what was true when it read it.
 *
 * Not in here: the camera, the selection, what is open. Those are the viewer's, not the
 * drawing's, and a recipe that carried them would replay someone else's scrolling.
 */
import { MAX_DEGREE, MAX_NODES, SHAPES, type SyntheticShape } from "../source/synthetic.ts";
import { type Fields, SettingsRefusal, fieldsOf, numberOf, oneOf, textOf, textOrNull, textsOf } from "./read.ts";

export { SettingsRefusal };

export type Source =
  | { readonly kind: "synthetic"; readonly seed: number; readonly nodes: number; readonly degree: number; readonly shape: SyntheticShape }
  | { readonly kind: "fixture"; readonly path: string }
  | { readonly kind: "document"; readonly name: string; readonly text: string };

export const THEMES = ["dark", "light"] as const;
export const COLOUR_BY = ["group", "kind", "analysis", "none"] as const;
export const SIZE_BY = ["weight", "degree", "analysis", "uniform"] as const;
export const LABEL_MODES = ["auto", "more", "none"] as const;
export const NODE_SCALE = { min: 0.25, max: 4, whole: false } as const;

export interface Appearance {
  readonly theme: (typeof THEMES)[number];
  readonly colourBy: (typeof COLOUR_BY)[number];
  readonly sizeBy: (typeof SIZE_BY)[number];
  /** Multiplies every node's radius. */
  readonly nodeScale: number;
  readonly labels: (typeof LABEL_MODES)[number];
}

/** Filters hide nodes in the drawing. The layout still ran over the whole graph. */
export interface Filter {
  /** Case-insensitive; a node is kept when its label contains it. */
  readonly text: string;
  readonly hiddenGroups: readonly string[];
  readonly minDegree: number;
}

export interface Settings {
  readonly source: Source;
  readonly layout: string;
  /** A POST pass over the layout's edges, or `null` for the edges as the layout drew them. */
  readonly edges: string | null;
  readonly analysis: string | null;
  readonly appearance: Appearance;
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
  });
}

function filterOf(filter: Filter): Filter {
  return Object.freeze({
    text: filter.text, hiddenGroups: Object.freeze([...filter.hiddenGroups]), minDegree: filter.minDegree,
  });
}

/** Members in one fixed order, so two equal documents are equal as text. */
function settingsOf(settings: Settings): Settings {
  return Object.freeze({
    source: Object.isFrozen(settings.source) ? settings.source : sourceOf(settings.source),
    layout: settings.layout,
    edges: settings.edges,
    analysis: settings.analysis,
    appearance: Object.isFrozen(settings.appearance) ? settings.appearance : appearanceOf(settings.appearance),
    filter: Object.isFrozen(settings.filter) ? settings.filter : filterOf(settings.filter),
  });
}

export const DEFAULT_SETTINGS: Settings = settingsOf({
  source: { kind: "synthetic", seed: 1, nodes: 400, degree: 2, shape: "vault" },
  layout: "layout.forceatlas2",
  edges: null,
  analysis: null,
  appearance: { theme: "dark", colourBy: "group", sizeBy: "weight", nodeScale: 1, labels: "auto" },
  filter: { text: "", hiddenGroups: [], minDegree: 0 },
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
  const fields = fieldsOf(value, at, ["theme", "colourBy", "sizeBy", "nodeScale", "labels"]);
  return {
    theme: oneOf(fields, at, "theme", THEMES),
    colourBy: oneOf(fields, at, "colourBy", COLOUR_BY),
    sizeBy: oneOf(fields, at, "sizeBy", SIZE_BY),
    nodeScale: numberOf(fields, at, "nodeScale", NODE_SCALE),
    labels: oneOf(fields, at, "labels", LABEL_MODES),
  };
}

function readFilter(value: unknown, at: string): Filter {
  const fields = fieldsOf(value, at, ["text", "hiddenGroups", "minDegree"]);
  return {
    text: textOf(fields, at, "text"),
    hiddenGroups: textsOf(fields, at, "hiddenGroups"),
    minDegree: numberOf(fields, at, "minDegree", { min: 0, max: 4294967295, whole: true }),
  };
}

/** Settings from outside the studio, or a refusal naming the member that was wrong. */
export function readSettings(value: unknown, at = "settings"): Settings {
  const fields: Fields = fieldsOf(value, at, ["source", "layout", "edges", "analysis", "appearance", "filter"]);
  return settingsOf({
    source: readSource(fields["source"], `${at}.source`),
    layout: textOf(fields, at, "layout"),
    edges: textOrNull(fields, at, "edges"),
    analysis: textOrNull(fields, at, "analysis"),
    appearance: readAppearance(fields["appearance"], `${at}.appearance`),
    filter: readFilter(fields["filter"], `${at}.filter`),
  });
}
