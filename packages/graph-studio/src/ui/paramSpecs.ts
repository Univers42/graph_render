/**
 * The motor's own schema, as the dock's own knobs: one `ParamSpec` per published parameter, in
 * the order the motor published them.
 *
 * Nothing here names a layout or a parameter. The kinds are the studio's, so `controlOf` picks
 * the control and the controls that already exist draw the row: a bounded float is a slider, an
 * int is a number field, a bool is a switch.
 */
import type { StudioParam } from "../actions/context.ts";
import type { LayoutParamSpec } from "../motor/protocol.ts";
import type { StudioState } from "../state/model.ts";
import { type ParamValue, type ParamValues } from "../state/settings.ts";

const KINDS: Readonly<Record<LayoutParamSpec["kind"], StudioParam["kind"]>> = {
  int: "int", float: "number", bool: "flag",
};

/** What the current layout is run at: what the caller set, and the motor's default elsewhere. */
export function valuesOf(state: StudioState, specs: readonly LayoutParamSpec[]): ParamValues {
  const held = state.settings.params[state.settings.layout] ?? {};
  return Object.fromEntries(specs.map((spec) => [spec.name, held[spec.name] ?? spec.default]));
}

export function heldValue(values: ParamValues, spec: LayoutParamSpec): ParamValue {
  return values[spec.name] ?? spec.default;
}

/**
 * One published parameter as a knob of the studio's own. `float` asks for a slider and the
 * other two take the plain control for their kind (`controlOf`).
 */
export function knobOf(spec: LayoutParamSpec): StudioParam {
  return {
    name: spec.name,
    kind: KINDS[spec.kind],
    title: spec.name,
    min: spec.min,
    max: spec.max,
    step: spec.step,
    value: () => spec.default,
    ...(spec.kind === "float" ? { control: "slider" as const } : {}),
  };
}

/** One published parameter and the knob that shows it. */
export interface ParamRow {
  readonly spec: LayoutParamSpec;
  readonly knob: StudioParam;
}

/** One row per published parameter, in the order the motor published them. */
export function rowsOf(specs: readonly LayoutParamSpec[]): readonly ParamRow[] {
  return specs.map((spec) => ({ spec, knob: knobOf(spec) }));
}

/**
 * What the panel is drawn from: the layout, and every value in force for it. The dock uses it
 * as the panel's remount key, so a value changed from the console is what the controls show.
 */
export function paramsKey(state: StudioState, specs: readonly LayoutParamSpec[]): string {
  return `${state.settings.layout}\n${JSON.stringify(valuesOf(state, specs))}`;
}
