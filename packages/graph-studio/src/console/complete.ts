/**
 * What Tab offers: command words for the first word, a parameter's choices after it.
 * `ghost` is the same offer as text: what the line would grow by, for the line to show it
 * before Tab is pressed.
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

/** What is being completed: the word, what stands in front of it, and what could be it. */
interface Offer {
  readonly head: string;
  readonly typed: string;
  readonly candidates: readonly string[];
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

/** The line is read as a command and its values; nothing here is run. */
function offer<State, Context>(text: string, registry: Registry<State, Context>, state: State): Offer {
  const line = text.trimStart();
  const words = line.split(/\s+/);
  const [first] = words;
  if (words.length <= 1) {
    const typed = first ?? "";
    return { head: "", typed, candidates: aliasesFrom(registry, typed) };
  }
  const action = registry.find(first ?? "");
  if (action === undefined) return { head: text, typed: "", candidates: [] };
  const last = words[words.length - 1] ?? "";
  const typed = last.slice(last.indexOf("=") + 1);
  const head = line.slice(0, line.length - typed.length);
  const holds = (choice: string): boolean => choice.toLowerCase().includes(typed.toLowerCase());
  return { head, typed, candidates: choicesFor(action, words, state).filter(holds) };
}

function aliasesFrom<State, Context>(registry: Registry<State, Context>, typed: string): readonly string[] {
  return registry.actions.map((action) => action.alias).filter((alias) => alias.startsWith(typed)).sort();
}

export function complete<State, Context>(text: string, registry: Registry<State, Context>, state: State): Completion {
  const { head, typed, candidates } = offer(text, registry, state);
  return candidates.length === 0 ? { candidates, text: head + typed } : extend(head, typed, candidates);
}

/**
 * What Tab would add to the line, or `""` when there is nothing to add.
 *
 * Ponytail: only what the candidates *agree* on, and only when the line grows at the end
 * of the word being typed. Where Tab would rewrite the word instead — `layout fo` into
 * `layout layout.force` — the ghost stays away: showing a completion that deletes what is
 * already typed reads as a line that is not the one being written. What a rewrite would
 * offer is `help <command>`, and Tab itself.
 */
export function ghost<State, Context>(text: string, registry: Registry<State, Context>, state: State): string {
  const { typed, candidates } = offer(text, registry, state);
  if (candidates.length === 0) return "";
  const [only] = candidates;
  const whole = candidates.length === 1 ? only ?? "" : sharedStart(candidates);
  if (!whole.toLowerCase().startsWith(typed.toLowerCase())) return "";
  return whole.slice(typed.length) + (candidates.length === 1 ? " " : "");
}
