/** What an action is offered now, and when a form that shows those values is remade. */
import type { StudioAction } from "../actions/context.ts";
import type { ArgValue, Args, Control } from "../actions/registry.ts";
import type { StudioState } from "../state/model.ts";

/** A string this long or longer is described by its head, not carried in full. */
const LONG = 96;
const HEAD = 32;
const ON_CHANGE: readonly Control[] = ["list", "select", "segmented", "toggle", "file"];

export function valuesOf(action: StudioAction, state: StudioState): Args {
  return Object.fromEntries(action.params.map((spec) => [spec.name, spec.value(state)]));
}

/**
 * Ponytail: a text over LONG characters is signed by its length and its first HEAD
 * characters, so two documents that begin alike and are the same size share a signature and
 * the form keeps showing the older one until some other value changes. Escape hatch: change
 * any other parameter, or run the action from the console.
 */
function signed(value: ArgValue): string {
  if (typeof value === "string" && value.length > LONG) return `${value.length}:${value.slice(0, HEAD)}`;
  return JSON.stringify(value);
}

/** The values as one string, in name order: equal values always give equal signatures. */
export function signatureOf(args: Args): string {
  return Object.keys(args).sort().map((name) => `${name}=${signed(args[name] ?? "")}`).join("\n");
}

/**
 * Everything an action's form draws that comes out of the state: the reason it is refused,
 * the values it is offered, and the choices each parameter lists. One string, so a form can
 * be compared without being drawn and a change to the log, the selection or the search box
 * leaves it exactly as it was.
 *
 * Caveat: a value that is equal but not identical is signed by value, so this is the same
 * comparison the form's own remount key makes; a parameter whose value is a rebuilt object
 * of the same content changes the signature by its printed form, not its identity.
 */
export function drawnOf(action: StudioAction, state: StudioState): string {
  const reason = action.available?.(state) ?? "";
  const choices = action.params.map((spec) => (spec.choices?.(state) ?? []).join("|")).join("\n");
  return `${reason}\n${signatureOf(valuesOf(action, state))}\n${choices}`;
}

/** True when picking a value is the whole run: there is nothing to confirm. */
export function commitsOnChange(control: Control): boolean {
  return ON_CHANGE.includes(control);
}
