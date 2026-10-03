/**
 * Reading a settings document that came from outside the studio — a recipe, a stored copy, a
 * file the user picked — member by member, with every refusal naming the member that was wrong.
 *
 * Its own module so `settings.ts` is the document and this is what makes one out of untrusted
 * JSON. The types come from there and are erased, so the dependency runs one way only.
 */
import { MAX_DEGREE, MAX_NODES, SHAPES } from "../source/synthetic.ts";
import { readForces } from "./forces.ts";
import { readParams } from "./paramValues.ts";
import { type Fields, SettingsRefusal, fieldsOf, flagOf, numberOf, oneOf, textOf, textOrNull, textsOf } from "./read.ts";
import {
  BACKGROUNDS, COLOUR_BY, EDGE_COLOURS, EDGE_STYLES, GLOW_STRENGTH, LABEL_MODES, LINK_THICKNESS, NODE_PX,
  NODE_SCALE, SIZE_BY, THEMES, TEXT_FADE,
  type Appearance, type Filter, type Group, type Settings, type Source, groupsOf, settingsOf,
} from "./settings.ts";

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
    "theme", "colourBy", "sizeBy", "nodeScale", "labels", "arrows", "textFade", "linkThickness", "edgeStyle", "edgeColour",
    "glow", "glowStrength", "background", "minRadius", "maxRadius",
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
    edgeColour: oneOf(fields, at, "edgeColour", EDGE_COLOURS),
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
  const fields: Fields = fieldsOf(value, at, [
    "source", "layout", "edges", "analysis", "params", "appearance", "groups", "filter", "forces",
  ]);
  return settingsOf({
    source: readSource(fields["source"], `${at}.source`),
    layout: textOf(fields, at, "layout"),
    edges: textOrNull(fields, at, "edges"),
    analysis: textOrNull(fields, at, "analysis"),
    params: readParams(fields["params"], `${at}.params`),
    appearance: readAppearance(fields["appearance"], `${at}.appearance`),
    groups: readGroups(fields["groups"], `${at}.groups`),
    filter: readFilter(fields["filter"], `${at}.filter`),
    forces: readForces(fields["forces"], `${at}.forces`),
  });
}
