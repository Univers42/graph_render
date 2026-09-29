/**
 * The document's own ordered list of groups: a name, the query that finds it, and the colour
 * it is drawn in. The list is a position as much as a membership — the first group a node
 * matches is the one it is drawn in — so add, drop, move and recolour each rewrite one list
 * and never touch anything else in the settings.
 */
import { QueryRefusal, parseQuery } from "../console/parse.ts";
import { GROUP_PALETTE } from "../look/palette.ts";
import { highlightOf } from "../look/visibleOf.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import { type Filter, type Group, withGroups } from "../state/settings.ts";
import { type StudioAction, type StudioContext, numberArg, textArg } from "./context.ts";
import { ActionRefusal, type Outcome } from "./registry.ts";

const HEX = /^#[0-9a-f]{6}$/i;
/** Both separators: `rgb(1,2,3)` and the CSS 4 `rgb(1 2 3)`. */
const RGB = /^rgba?\(\s*\d{1,3}\s*[,\s]\s*\d{1,3}\s*[,\s]\s*\d{1,3}\s*([,\s]\s*(0|1|0?\.\d+)\s*)?\)$/i;
const MAX_POSITIONS = 255;
const FALLBACK_COLOUR = "#8b8d98";

function described(state: StudioState): string | null {
  return state.meta === null ? "nothing is drawn" : null;
}

function grouped(state: StudioState): string | null {
  return described(state) ?? (state.settings.groups.length === 0 ? "no group has been added" : null);
}

function named(groups: readonly Group[]): readonly string[] {
  return groups.map((group) => group.name);
}

function firstNamed(groups: readonly Group[]): string {
  return groups[0]?.name ?? "";
}

function said(what: string, groups: readonly Group[]): Outcome {
  return { message: `${what}; ${groups.length} groups` };
}

/** Read before it is stored: a query the grammar refuses never reaches the document. */
function queryOf(text: string): string {
  try {
    parseQuery(text);
  } catch (error) {
    if (error instanceof QueryRefusal) throw new ActionRefusal("bad-value", `\`query\` ${error.message}`);
    throw error;
  }
  return text;
}

/**
 * Ponytail: a colour is read as `#rrggbb` or `rgb()`/`rgba()` and nothing else, because one
 * the studio cannot read would reach the canvas and be dropped there: the user would see a
 * group in nobody's colour and no refusal anywhere. Fails: a named colour (`red`) or `#abc`,
 * both of which a browser accepts. Escape hatch: `addgroup` with an empty colour.
 */
function checkedColour(colour: string): string {
  if (HEX.test(colour) || RGB.test(colour)) return colour;
  throw new ActionRefusal(
    "bad-value",
    `\`colour\` must be #rrggbb, rgb(r,g,b) or rgba(r,g,b,a), not ${JSON.stringify(colour)}`,
  );
}

/**
 * Ponytail: a group with no colour of its own takes the first the palette has left, and once
 * they are all taken the list cycles. Fails: eleven groups, the eleventh drawn in the first
 * colour again and told apart by nothing but its place. Escape hatch: `recolour` names one.
 */
function nextColour(groups: readonly Group[]): string {
  const used = new Set(groups.map((group) => group.colour));
  const free = GROUP_PALETTE.find((colour) => !used.has(colour));
  return free ?? GROUP_PALETTE[groups.length % GROUP_PALETTE.length] ?? FALLBACK_COLOUR;
}

/** A name already in the list is replaced where it stands: the position is the membership. */
function put(groups: readonly Group[], group: Group): readonly Group[] {
  const at = groups.findIndex((held) => held.name === group.name);
  if (at < 0) return [...groups, group];
  return groups.map((held, i) => (i === at ? group : held));
}

const add: StudioAction = {
  id: "groups.add", alias: "addgroup", title: "Add a group", section: "Filters",
  params: [
    { name: "name", kind: "text", title: "Name", control: "text", value: () => "" },
    { name: "query", kind: "text", title: "Query", control: "text", value: () => "" },
    { name: "colour", kind: "text", title: "Colour", control: "text", value: () => "" },
  ],
  available: described,
  run: (context, args) => {
    const name = textArg(args, "name");
    const query = queryOf(textArg(args, "query"));
    const asked = textArg(args, "colour");
    const groups = context.state().settings.groups;
    const colour = asked === "" ? nextColour(groups) : checkedColour(asked);
    const next = withGroups(context.state().settings, put(groups, { name, query, colour }));
    context.look(next);
    return said(`\`${name}\` in ${colour}`, next.groups);
  },
};

const drop: StudioAction = {
  id: "groups.remove", alias: "dropgroup", title: "Drop a group", section: "Filters",
  params: [{
    name: "name", kind: "choice", title: "Group", control: "select",
    choices: (state) => named(state.settings.groups), value: (state) => firstNamed(state.settings.groups),
  }],
  available: grouped,
  run: (context, args) => {
    const name = textArg(args, "name");
    const groups = context.state().settings.groups;
    const next = withGroups(context.state().settings, groups.filter((held) => held.name !== name));
    context.look(next);
    return said(`\`${name}\` dropped`, next.groups);
  },
};

function move(context: StudioContext, name: string, to: number): Outcome {
  const groups = context.state().settings.groups;
  const from = groups.findIndex((group) => group.name === name);
  if (from < 0) throw new ActionRefusal("bad-value", `no group is named \`${name}\``);
  // Refused, not clamped: a position the list has no place for is a mistake worth naming, and
  // a clamp would move the group somewhere nobody asked for without saying so.
  if (to < 0 || to >= groups.length) {
    throw new ActionRefusal("bad-value", `\`to\` must be 0..${groups.length - 1}; \`${name}\` is at ${from}`);
  }
  const rest = [...groups];
  const [moved] = rest.splice(from, 1);
  if (moved !== undefined) rest.splice(to, 0, moved);
  const next = withGroups(context.state().settings, rest);
  context.look(next);
  return said(`\`${name}\` moved to ${to}`, next.groups);
}

const reorder: StudioAction = {
  id: "groups.reorder", alias: "movegroup", title: "Move a group", section: "Filters",
  params: [
    {
      name: "name", kind: "choice", title: "Group", control: "select",
      choices: (state) => named(state.settings.groups), value: (state) => firstNamed(state.settings.groups),
    },
    { name: "to", kind: "int", title: "Position", control: "number", min: 0, max: MAX_POSITIONS, value: () => 0 },
  ],
  available: grouped,
  run: (context, args) => move(context, textArg(args, "name"), numberArg(args, "to")),
};

const recolour: StudioAction = {
  id: "groups.recolour", alias: "recolour", title: "Recolour a group", section: "Filters",
  params: [
    {
      name: "name", kind: "choice", title: "Group", control: "select",
      choices: (state) => named(state.settings.groups), value: (state) => firstNamed(state.settings.groups),
    },
    { name: "colour", kind: "text", title: "Colour", control: "text", value: () => "" },
  ],
  available: grouped,
  run: (context, args) => {
    const name = textArg(args, "name");
    const colour = checkedColour(textArg(args, "colour"));
    const groups = context.state().settings.groups;
    const held = groups.find((group) => group.name === name);
    if (held === undefined) throw new ActionRefusal("bad-value", `no group is named \`${name}\``);
    const next = withGroups(context.state().settings, put(groups, { ...held, colour }));
    context.look(next);
    return said(`\`${name}\` is ${colour}`, next.groups);
  },
};

/** How many nodes the search lit up, so the camera can be told what to fit. */
function results(meta: GraphMeta, filter: Filter): number {
  const lit = highlightOf(meta, filter);
  return lit === null ? 0 : lit.reduce((sum, flag) => sum + flag, 0);
}

const fit: StudioAction = {
  id: "search.fit", alias: "fitresults", title: "Fit the view to the results", section: "Filters",
  params: [],
  available: described,
  run: (context) => {
    const { meta, settings } = context.state();
    if (meta === null) throw new ActionRefusal("unavailable", "nothing is drawn");
    context.look(settings);
    return { message: `fit to ${results(meta, settings.filter)} results` };
  },
};

export const GROUP_ACTIONS: readonly StudioAction[] = [add, drop, reorder, recolour, fit];
