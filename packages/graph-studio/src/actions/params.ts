/**
 * `set <param> <value>` and `get <param>`, over every parameter the studio's actions
 * already hold. The list is built from those actions, so a parameter that exists is
 * settable, one that is renamed is renamed, and a new action's parameters arrive here
 * without anyone writing them down twice.
 *
 * `set` changes a value the way the console already changes it: by running that action,
 * with the new value and the rest of its parameters as they stand. What the log prints
 * is then the line that was typed, `set theme.name light`, and the panel control that
 * does the same prints `theme light` — the two lines for the one change, both readable.
 *
 * Ponytail: not every parameter is a value that is held. `zoom.factor` is a factor one
 * call is by, not a zoom level, and `group.hide` is a toggle's default, so `get` reads
 * what the action would take if the parameter were left out — the only value the
 * registry holds. It is not what the camera or the drawing is doing now, and the
 * message does not claim to be: `zoom.factor = 1.25` is the factor a bare `zoom` uses.
 *
 * And the value after the parameter's name is not completed: which values a parameter
 * takes is a function of the state *and* of the parameter in front of it, and Tab is
 * offered the words before that one is read. `get theme.name` and `help theme` are what
 * name the values.
 *
 * What is not here is the layout's own arithmetic. ForceAtlas2's gravity, scaling ratio,
 * iteration count and seed live in Rust, and the wire carries two strings, a layout id
 * and an edge-pass id: there is no parameter map to address, so `set gravity` is refused
 * like any other word that names nothing. Until the protocol carries one, `layout` names
 * an algorithm and not its numbers, and this list says so by its absence.
 */
import { textArg, type StudioAction, type StudioContext, type StudioParam } from "./context.ts";
import { ActionRefusal, readParam, type Args, type ArgValue, type Outcome } from "./registry.ts";
import type { StudioState } from "../state/model.ts";

/** A parameter of an action, named `<alias>.<name>`. */
interface Address {
  readonly action: StudioAction;
  readonly spec: StudioParam;
  readonly name: string;
}

function names(actions: readonly StudioAction[]): readonly string[] {
  return actions.flatMap((action) => action.params.map((spec) => `${action.alias}.${spec.name}`));
}

/** The two commands do not address each other: they are how the others are named. */
function held(actions: readonly StudioAction[]): readonly StudioAction[] {
  return actions.filter((action) => action.alias !== "set" && action.alias !== "get");
}

function addressOf(actions: readonly StudioAction[], word: string): Address {
  const at = word.lastIndexOf(".");
  const alias = word.slice(0, at);
  const action = actions.find((candidate) => candidate.alias === alias);
  const spec = action?.params.find((candidate) => candidate.name === word.slice(at + 1));
  // The registry has already refused a word that names no parameter; this is the last word.
  if (action === undefined || spec === undefined) {
    throw new ActionRefusal("unknown-param", `\`${word}\` is not a parameter; \`help\` lists them all`);
  }
  return { action, spec, name: word };
}

/** The new value, read by the registry's own reading of that parameter, or refused. */
function valueFor(found: Address, raw: string, state: StudioState): ArgValue {
  return readParam(found.spec, raw, state);
}

function withValue(found: Address, value: ArgValue, state: StudioState): Args {
  const { action, spec } = found;
  return Object.fromEntries(action.params.map((other) => [
    other.name, other.name === spec.name ? value : other.value(state),
  ]));
}

function refusal(found: Address, state: StudioState): ActionRefusal | null {
  const reason = found.action.available?.(state) ?? null;
  return reason === null ? null : new ActionRefusal("unavailable", `\`${found.action.alias}\` cannot run: ${reason}`);
}

function setting(context: StudioContext, args: Args): Outcome | Promise<Outcome> {
  const found = addressOf(context.actions(), textArg(args, "param"));
  const state = context.state();
  const why = refusal(found, state);
  if (why !== null) throw why;
  const values = withValue(found, valueFor(found, textArg(args, "value"), state), state);
  return found.action.run(context, values);
}

function reading(context: StudioContext, args: Args): Outcome {
  const found = addressOf(context.actions(), textArg(args, "param"));
  const choices = found.spec.choices?.(context.state()) ?? [];
  const notes = choices.length > 0 ? [`${found.name} is one of: ${choices.join(", ")}`] : [];
  return { message: `${found.name} = ${String(found.spec.value(context.state()))}`, notes };
}

function address(every: readonly string[]): StudioParam {
  return {
    name: "param", kind: "choice", title: "Parameter", control: "select",
    choices: () => every, value: () => every[0] ?? "",
  };
}

/** The two commands, over the actions the studio holds. */
export function paramActions(actions: readonly StudioAction[]): readonly StudioAction[] {
  const every = names(held(actions));
  return [
    {
      id: "param.set", alias: "set", title: "Set a parameter", section: null,
      params: [
        address(every),
        { name: "value", kind: "text", title: "Value", control: "text", value: () => "" },
      ],
      run: setting,
    },
    { id: "param.get", alias: "get", title: "Read a parameter", section: null, params: [address(every)], run: reading },
  ];
}
