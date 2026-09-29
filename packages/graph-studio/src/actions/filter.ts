/**
 * What is hidden in the drawing. The layout ran over the whole graph and is not re-run: every
 * action here goes through `filter`, which asks the look and never the motor. The one member
 * that would change the layout is `relayout`, and it is a setting the studio reads, not a stage
 * this file runs.
 */
import { QueryRefusal, parseQuery } from "../console/parse.ts";
import { hiddenOf } from "../look/visibleOf.ts";
import { NODE_KINDS } from "../source/ingest.ts";
import type { StudioState } from "../state/model.ts";
import { type Filter, withFilter } from "../state/settings.ts";
import { type StudioAction, type StudioContext, flagArg, numberArg, textArg } from "./context.ts";
import { ActionRefusal, type Outcome } from "./registry.ts";

const DEGREE_MAX = 4294967295;
const NOTHING: Filter = {
  query: "", text: "", hiddenKinds: [], hiddenGroups: [],
  orphans: false, existingOnly: false, minDegree: 0, relayout: false,
};

function described(state: StudioState): string | null {
  return state.meta === null ? "nothing is drawn" : null;
}

// Ponytail: with `relayout` on the motor lays the whole graph out again, not only the nodes the
// filter keeps, because the motor is handed no mask; the drawing then shows the hidden ones'
// places as gaps. A filter that keeps most of the graph moves little; one that keeps a few does not.
async function filter(context: StudioContext, patch: Partial<Filter>): Promise<Outcome> {
  const next = withFilter(context.state().settings, patch);
  if (next.filter.relayout) await context.apply(next);
  else context.look(next);
  const { meta } = context.state();
  const hidden = meta === null ? null : hiddenOf(meta, next.filter);
  const count = hidden === null ? 0 : hidden.reduce((sum, flag) => sum + flag, 0);
  return { message: `${count} of ${meta?.nodeCount ?? 0} nodes hidden` };
}

function withGroup(groups: readonly string[], name: string, hide: boolean): readonly string[] {
  const others = groups.filter((group) => group !== name);
  return hide ? [...others, name] : others;
}

/** The list in NODE_KINDS order, not the order the kinds were switched off in. */
function withKind(kinds: readonly string[], name: string, hide: boolean): readonly string[] {
  const shown = new Set(kinds);
  if (hide) shown.add(name);
  else shown.delete(name);
  return NODE_KINDS.filter((kind) => shown.has(kind));
}

/** A query is read before it is stored: a bad one never reaches the document. */
function queryOf(text: string): string {
  try {
    parseQuery(text);
  } catch (error) {
    if (error instanceof QueryRefusal) throw new ActionRefusal("bad-value", `\`query\` ${error.message}`);
    throw error;
  }
  return text;
}

/** The label the search box holds, and what `filter` matches on. They are one member. */
const text: StudioAction = {
  id: "filter.text", alias: "filter", title: "Name contains", section: "Filters",
  params: [{ name: "text", kind: "text", title: "Name contains", control: "text", value: (state) => state.settings.filter.text }],
  available: described,
  run: (context, args) => filter(context, { text: textArg(args, "text") }),
};

const group: StudioAction = {
  id: "filter.group", alias: "group", title: "Hide a group", section: "Filters",
  params: [
    {
      name: "name", kind: "choice", title: "Group", control: "select",
      choices: (state) => state.meta?.groups ?? [], value: (state) => state.meta?.groups[0] ?? "",
    },
    { name: "hide", kind: "flag", title: "Hidden", control: "toggle", value: () => true },
  ],
  available: described,
  run: (context, args) => filter(context, {
    hiddenGroups: withGroup(context.state().settings.filter.hiddenGroups, textArg(args, "name"), flagArg(args, "hide")),
  }),
};

const degree: StudioAction = {
  id: "filter.degree", alias: "mindegree", title: "Fewest links", section: "Filters",
  params: [{
    name: "min", kind: "int", title: "Fewest links", control: "number", min: 0, max: DEGREE_MAX,
    value: (state) => state.settings.filter.minDegree,
  }],
  available: described,
  run: (context, args) => filter(context, { minDegree: numberArg(args, "min") }),
};

const clear: StudioAction = {
  id: "filter.clear", alias: "unfilter", title: "Show everything", section: "Filters",
  params: [],
  available: described,
  run: (context) => filter(context, NOTHING),
};

const query: StudioAction = {
  id: "filter.query", alias: "query", title: "Query", section: "Filters",
  params: [{ name: "query", kind: "text", title: "Query", control: "text", value: (state) => state.settings.filter.query }],
  available: described,
  run: (context, args) => filter(context, { query: queryOf(textArg(args, "query")) }),
};

const orphans: StudioAction = {
  id: "filter.orphans", alias: "orphans", title: "Hide the orphans", section: "Filters",
  params: [{ name: "hide", kind: "flag", title: "Hidden", control: "toggle", value: () => true }],
  available: described,
  run: (context, args) => filter(context, { orphans: flagArg(args, "hide") }),
};

const kind: StudioAction = {
  id: "filter.kind", alias: "kind", title: "Hide a kind", section: "Filters",
  params: [
    { name: "name", kind: "choice", title: "Kind", control: "select", choices: () => NODE_KINDS, value: () => NODE_KINDS[0] ?? "record" },
    { name: "hide", kind: "flag", title: "Hidden", control: "toggle", value: () => true },
  ],
  available: described,
  run: (context, args) => filter(context, {
    hiddenKinds: withKind(context.state().settings.filter.hiddenKinds, textArg(args, "name"), flagArg(args, "hide")),
  }),
};

const existing: StudioAction = {
  id: "filter.existing", alias: "existing", title: "Only what is linked", section: "Filters",
  params: [{ name: "hide", kind: "flag", title: "Hidden", control: "toggle", value: () => true }],
  available: described,
  run: (context, args) => filter(context, { existingOnly: flagArg(args, "hide") }),
};

const relayout: StudioAction = {
  id: "filter.relayout", alias: "relayout", title: "Lay the rest out again", section: "Filters",
  params: [{ name: "on", kind: "flag", title: "Relayout", control: "toggle", value: () => true }],
  available: described,
  run: (context, args) => filter(context, { relayout: flagArg(args, "on") }),
};

/** The search box's own text; `filter` is the same member read as a filter's own value. */
const search: StudioAction = {
  id: "filter.search", alias: "search", title: "Search", section: "Filters",
  params: [{ name: "text", kind: "text", title: "Search", control: "text", value: (state) => state.settings.filter.text }],
  available: described,
  run: (context, args) => filter(context, { text: textArg(args, "text") }),
};

export const FILTER_ACTIONS: readonly StudioAction[] = [
  text, group, degree, clear, query, orphans, kind, existing, relayout, search,
];
