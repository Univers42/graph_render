/** What is hidden in the drawing. The layout ran over the whole graph and is not re-run. */
import { hiddenOf } from "../look/styleOf.ts";
import type { StudioState } from "../state/model.ts";
import { type Filter, withFilter } from "../state/settings.ts";
import { type StudioAction, type StudioContext, flagArg, numberArg, textArg } from "./context.ts";
import type { Outcome } from "./registry.ts";

const DEGREE_MAX = 4294967295;
const NOTHING: Filter = { text: "", hiddenGroups: [], minDegree: 0 };

function described(state: StudioState): string | null {
  return state.meta === null ? "nothing is drawn" : null;
}

function filter(context: StudioContext, patch: Partial<Filter>): Outcome {
  const next = withFilter(context.state().settings, patch);
  context.look(next);
  const { meta } = context.state();
  const hidden = meta === null ? null : hiddenOf(meta, next.filter);
  const count = hidden === null ? 0 : hidden.reduce((sum, flag) => sum + flag, 0);
  return { message: `${count} of ${meta?.nodeCount ?? 0} nodes hidden` };
}

function withGroup(groups: readonly string[], name: string, hide: boolean): readonly string[] {
  const others = groups.filter((group) => group !== name);
  return hide ? [...others, name] : others;
}

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

export const FILTER_ACTIONS: readonly StudioAction[] = [text, group, degree, clear];
