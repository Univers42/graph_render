/**
 * The settings' one member that is a map of maps: what each layout is run at, by layout id and
 * by the motor's own parameter names.
 *
 * Its own module because it is the only member whose shape nobody can write down: the names are
 * the motor's, so there is no list of them here, and a value's range is the motor's too and is
 * checked there (`docs/decisions/layout-params.md`).
 */
import { SettingsRefusal } from "./read.ts";

/** One value: a `number` for every kind but a bool, which is `true` or `false`. */
export type ParamValue = number | boolean;
/** What one layout is run at, by the motor's own parameter names and its own kinds. */
export type ParamValues = Readonly<Record<string, ParamValue>>;
/**
 * What each layout is run at, by layout id: the values a caller has set for it. A layout with
 * no entry is run at the motor's own defaults, which is what leaving it out means.
 */
export type ParamsByLayout = Readonly<Record<string, ParamValues>>;

export function valuesOf(values: ParamValues): ParamValues {
  return Object.freeze(Object.fromEntries(Object.entries(values).map(([name, value]) => [name, value])));
}

/** Frozen at both levels, in the order the layouts were first given. */
export function paramsOf(params: ParamsByLayout): ParamsByLayout {
  return Object.freeze(Object.fromEntries(
    Object.entries(params).map(([layoutId, values]) => [layoutId, valuesOf(values)]),
  ));
}

/**
 * One value as the document holds it: a finite number or a bool. No range is checked here,
 * because the range is the motor's and is not in the document: a value a hand-edited file
 * puts out of range is refused by the motor, by name, on the run.
 */
function readValues(value: unknown, at: string): ParamValues {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new SettingsRefusal(at, "not an object");
  const values: Record<string, ParamValue> = {};
  for (const [name, held] of Object.entries(value)) {
    if (typeof held === "boolean") values[name] = held;
    else if (typeof held === "number" && Number.isFinite(held)) values[name] = held;
    else throw new SettingsRefusal(`${at}.${name}`, "not a finite number or true/false");
  }
  return valuesOf(values);
}

/** What each layout is run at, from a document that came from outside the studio. */
export function readParams(value: unknown, at = "settings.params"): ParamsByLayout {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new SettingsRefusal(at, "not an object");
  const params: Record<string, ParamValues> = {};
  for (const [layoutId, held] of Object.entries(value)) {
    params[layoutId] = readValues(held, `${at}["${layoutId}"]`);
  }
  return paramsOf(params);
}
