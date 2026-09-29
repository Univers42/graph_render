/**
 * What `help` prints. Every line is read off the one registry: the sections an action
 * declares, the words and values its parameters declare, and its own title as the
 * one-line description. There is no second list here to forget to add to.
 *
 * Ponytail: the groups are in the order the actions are declared, which is the order the
 * dock shows them in, rather than a second order kept beside it. An action whose section
 * is `null` is one the dock does not show, and lands in the last group, `View`.
 *
 * The commands are marked with a `·` and not indented: the log prints every note as its
 * own line of text, and a run of leading spaces does not survive that.
 */
import { ActionRefusal, type Action, type Outcome, type ParamSpec } from "../actions/registry.ts";

/** The commands the dock does not show: the camera, the console, the motor's stop. */
const UNSHOWN = "View";
/** What marks a command under its group, in a log that drops leading spaces. */
const MARK = "·";

/** The line that, typed in the console, asks for `action` and nothing else. */
export function usage<State, Context>(action: Action<State, Context>): string {
  const values = action.params.map((spec) => ` <${spec.name}>`).join("");
  return `${action.alias}${values} — ${action.title}`;
}

function valuesOf<State>(spec: ParamSpec<State>, state: State): string {
  if (spec.kind === "choice") return (spec.choices?.(state) ?? []).join(", ");
  if (spec.kind === "flag") return "on, off";
  if (spec.kind === "text") return "text";
  return `${spec.kind === "int" ? "a whole number" : "a number"} in ${spec.min ?? "-∞"}..${spec.max ?? "∞"}`;
}

/** The sections in the order the actions are declared in; `null` is the last, `View`. */
function sections<State, Context>(actions: readonly Action<State, Context>[]): readonly string[] {
  const seen: string[] = [];
  for (const action of actions) {
    const name = action.section ?? UNSHOWN;
    if (!seen.includes(name)) seen.push(name);
  }
  return seen;
}

function every<State, Context>(actions: readonly Action<State, Context>[]): readonly string[] {
  return sections(actions).flatMap((section) => [
    section,
    ...actions.filter((action) => (action.section ?? UNSHOWN) === section)
      .map((action) => `${MARK} ${usage(action)}`),
  ]);
}

function one<State, Context>(actions: readonly Action<State, Context>[], state: State, word: string): Outcome {
  const action = actions.find((candidate) => candidate.alias === word || candidate.id === word);
  if (action === undefined) throw new ActionRefusal("bad-value", `\`${word}\` is not a command; \`help\` lists them all`);
  return { message: usage(action), notes: action.params.map((spec) => `${spec.name}: ${valuesOf(spec, state)}`) };
}

/** `help` with no word is the whole listing; with a word, that one command's values. */
export function listing<State, Context>(actions: readonly Action<State, Context>[], state: State, word: string): Outcome {
  if (word !== "") return one(actions, state, word);
  return { message: `${actions.length} commands`, notes: every(actions) };
}
