/**
 * What Tab offers: command words for the first word, a parameter's choices after it.
 *
 * Ponytail: the line is split on spaces, so a quoted value holding a space is completed
 * as if its last word stood alone, and a value after a quoted one counts one position too
 * far. It offers too little in both cases and never rewrites what was typed before.
 */
import type { Action, Registry } from "../actions/registry.ts";

export interface Completion {
  readonly candidates: readonly string[];
  /** The line after completing: extended as far as the candidates agree. */
  readonly text: string;
}

function sharedStart(candidates: readonly string[]): string {
  let shared = candidates[0] ?? "";
  for (const candidate of candidates) {
    let length = 0;
    while (length < shared.length && shared[length] === candidate[length]) length += 1;
    shared = shared.slice(0, length);
  }
  return shared;
}

function extend(head: string, typed: string, candidates: readonly string[]): Completion {
  const [only] = candidates;
  if (candidates.length === 1 && only !== undefined) return { candidates, text: `${head}${only} ` };
  const shared = sharedStart(candidates);
  const grows = shared.length > typed.length && shared.toLowerCase().startsWith(typed.toLowerCase());
  const holds = shared.length > 0 && !shared.toLowerCase().startsWith(typed.toLowerCase());
  return { candidates, text: grows || holds ? `${head}${shared}` : `${head}${typed}` };
}

function choicesFor<State, Context>(action: Action<State, Context>, words: readonly string[], state: State): readonly string[] {
  const last = words[words.length - 1] ?? "";
  const split = last.indexOf("=");
  const spec = split > 0
    ? action.params.find((candidate) => candidate.name === last.slice(0, split))
    : action.params[words.slice(1, -1).filter((word) => !word.includes("=")).length];
  if (spec === undefined || spec.kind !== "choice") return [];
  return spec.choices?.(state) ?? [];
}

export function complete<State, Context>(text: string, registry: Registry<State, Context>, state: State): Completion {
  const line = text.trimStart();
  const words = line.split(/\s+/);
  const [first] = words;
  if (words.length <= 1) {
    const typed = first ?? "";
    const aliases = registry.actions.map((action) => action.alias).filter((alias) => alias.startsWith(typed)).sort();
    return extend("", typed, aliases);
  }
  const action = registry.find(first ?? "");
  if (action === undefined) return { candidates: [], text };
  const last = words[words.length - 1] ?? "";
  const typed = last.slice(last.indexOf("=") + 1);
  const head = line.slice(0, line.length - typed.length);
  const candidates = choicesFor(action, words, state).filter((choice) => choice.toLowerCase().includes(typed.toLowerCase()));
  return candidates.length === 0 ? { candidates, text } : extend(head, typed, candidates);
}
