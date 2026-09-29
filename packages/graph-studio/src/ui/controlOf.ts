/** Which control shows a parameter: the one it asks for, or the plain one for its kind. */
import type { Control, ParamKind, ParamSpec } from "../actions/registry.ts";

const PLAIN: Readonly<Record<ParamKind, Control>> = {
  int: "number", number: "number", text: "text", choice: "select", flag: "toggle",
};

export function controlOf<State>(spec: ParamSpec<State>): Control {
  return spec.control ?? PLAIN[spec.kind];
}
