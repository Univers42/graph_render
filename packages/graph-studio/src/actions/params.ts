/**
 * What one layout is run at, changed from the dock and from the console.
 *
 * Every name and every bound comes from the schema the motor published (`motor.protocol.ts`),
 * so nothing here names a layout or a parameter: the studio's source cannot fall out of step
 * with the registry because it never had a copy of it. A value is checked against that schema
 * once, in {@link checkedValue}, which `resolve` reaches through `accept` and which the run
 * then reads — one rule, not one per entry point.
 */
import type { LayoutParamSpec } from "../motor/protocol.ts";
import type { StudioState } from "../state/model.ts";
import { type ParamValue, type ParamValues, withParams, withoutParams } from "../state/settings.ts";
import { type StudioAction, type StudioContext, textArg } from "./context.ts";
import { FLAG_WORDS, type Outcome } from "./registry.ts";

const EMPTY: readonly LayoutParamSpec[] = [];

/** The dock section the motor's own parameters are shown in. */
export const PARAMS_SECTION = "Layout settings";

/** The ids the dock's own panel dispatches; each one's console word is its alias. */
export const SET_ID = "layout.set";
export const SET_MANY_ID = "layout.params";
export const RESET_ID = "layout.reset";

/** What the motor publishes for the layout now on screen, or nothing yet: it has not said. */
export function specsOf(state: StudioState): readonly LayoutParamSpec[] {
  return state.schemas[state.settings.layout] ?? EMPTY;
}

function specNamed(state: StudioState, name: string): LayoutParamSpec | undefined {
  return specsOf(state).find((spec) => spec.name === name);
}

/** Why the current layout's parameters cannot be set, or `null`. */
export function unavailable(state: StudioState): string | null {
  if (state.graph === null) return "no graph is loaded";
  if (state.schemas[state.settings.layout] === undefined) return `${state.settings.layout} has no schema yet`;
  if (specsOf(state).length === 0) return `${state.settings.layout} publishes no parameters`;
  return null;
}

export type Checked =
  | { readonly ok: true; readonly value: ParamValue }
  | { readonly ok: false; readonly reason: string };

function whole(spec: LayoutParamSpec, value: number): string | null {
  return spec.kind !== "int" || Number.isInteger(value) ? null : `must be a whole number, not ${value}`;
}

/** The value `raw` is for `spec`, or the reason it is not one. */
export function checkedValue(spec: LayoutParamSpec, raw: unknown): Checked {
  const text = typeof raw === "string" ? raw.trim() : String(raw);
  if (spec.kind === "bool") {
    const flag = typeof raw === "boolean" ? raw : FLAG_WORDS.get(text.toLowerCase());
    if (typeof flag !== "boolean") return { ok: false, reason: `\`${spec.name}\` must be on or off, not ${text}` };
    return { ok: true, value: flag };
  }
  const value = typeof raw === "number" ? raw : Number(text);
  if (!Number.isFinite(value)) return { ok: false, reason: `\`${spec.name}\` must be a number, not ${text}` };
  const notWhole = whole(spec, value);
  if (notWhole !== null) return { ok: false, reason: `\`${spec.name}\` ${notWhole}` };
  if (value < spec.min || value > spec.max) {
    return { ok: false, reason: `\`${spec.name}\` must be in ${spec.min}..${spec.max}, not ${value}` };
  }
  return { ok: true, value };
}

/** The value out of a checked one; the refusal has already been made by `accept`. */
function valueOf(spec: LayoutParamSpec, raw: unknown): ParamValue {
  const checked = checkedValue(spec, raw);
  if (!checked.ok) throw new Error(checked.reason);
  return checked.value;
}

/** `values` as the layout's own names and kinds, or the first one that is not. */
function checkedValues(state: StudioState, text: string): ParamValues {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("the values are not JSON");
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error(`the values must be a JSON object of name and value, not ${text}`);
  }
  const values: Record<string, ParamValue> = {};
  for (const [name, raw] of Object.entries(parsed)) {
    const spec = specNamed(state, name);
    if (spec === undefined) throw new Error(`\`${name}\` is not a parameter this layout publishes`);
    values[name] = valueOf(spec, raw);
  }
  return values;
}

/** Why `text` is not a set of values this layout could be run at, or `null`. */
function reasonForValues(state: StudioState, text: string): string | null {
  try {
    checkedValues(state, text);
    return null;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

const one: StudioAction = {
  id: SET_ID, alias: "layoutset", title: "Set a layout parameter", section: null,
  params: [
    {
      name: "param", kind: "choice", title: "Parameter",
      choices: (state) => specsOf(state).map((spec) => spec.name),
      value: (state) => specsOf(state)[0]?.name ?? "",
    },
    { name: "value", kind: "text", title: "Value", value: (state) => heldText(state, specsOf(state)[0]) },
  ],
  available: unavailable,
  accept: (args, state) => reasonForOne(state, args.param, args.value),
  run: (context, args) => setOne(context, textArg(args, "param"), textArg(args, "value")),
};

const many: StudioAction = {
  id: SET_MANY_ID, alias: "layoutparams", title: "Set several layout parameters", section: null,
  params: [{ name: "values", kind: "text", title: "Values as JSON", value: (state) => JSON.stringify(heldOf(state)) }],
  available: unavailable,
  accept: (args, state) => reasonForValues(state, String(args.values)),
  run: (context, args) => setMany(context, textArg(args, "values")),
};

const reset: StudioAction = {
  id: RESET_ID, alias: "layoutreset", title: "Reset this layout's parameters", section: null,
  params: [],
  available: (state) => unavailable(state) ?? nothingHeld(state),
  run: (context) => resetOne(context),
};

/** Why there is nothing to put back, or `null`. */
function nothingHeld(state: StudioState): string | null {
  const held = state.settings.params[state.settings.layout];
  return held === undefined || Object.keys(held).length === 0 ? "already at the motor's defaults" : null;
}

/** What the layout is run at now, each value as text: the console's face of a set of values. */
export function heldText(state: StudioState, spec: LayoutParamSpec | undefined): string {
  if (spec === undefined) return "";
  return String(heldOf(state)[spec.name] ?? spec.default);
}

function heldOf(state: StudioState): ParamValues {
  const held = state.settings.params[state.settings.layout] ?? {};
  return Object.fromEntries(specsOf(state).map((spec) => [spec.name, held[spec.name] ?? spec.default]));
}

function reasonForOne(state: StudioState, name: string, value: unknown): string | null {
  const spec = specNamed(state, String(name));
  if (spec === undefined) return `\`${name}\` is not a parameter this layout publishes`;
  const checked = checkedValue(spec, value);
  return checked.ok ? null : checked.reason;
}

function setOne(context: StudioContext, name: string, value: string): Promise<Outcome> {
  const state = context.state();
  const spec = specNamed(state, name);
  if (spec === undefined) return Promise.reject(new Error(`\`${name}\` is not a parameter this layout publishes`));
  return context.apply(withParams(state.settings, state.settings.layout, { [name]: valueOf(spec, value) }));
}

function setMany(context: StudioContext, text: string): Promise<Outcome> {
  const state = context.state();
  return context.apply(withParams(state.settings, state.settings.layout, checkedValues(state, text)));
}

function resetOne(context: StudioContext): Promise<Outcome> {
  const settings = context.state().settings;
  return context.apply(withoutParams(settings, settings.layout));
}

export const PARAMS_ACTIONS: readonly StudioAction[] = [one, many, reset];
