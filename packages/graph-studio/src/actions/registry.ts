/**
 * The one list of things the studio can do. The dock, the console, the shortcuts and a
 * host all go through `resolve`, so a value is read, checked and refused in one place.
 */
import type { DeltaEdge, DeltaNode, GraphBatch } from "../motor/protocol.ts";

export type ArgValue = string | number | boolean;
export type Args = Readonly<Record<string, ArgValue>>;
export type RawArgs = Readonly<Record<string, unknown>>;
export type ParamKind = "int" | "number" | "text" | "choice" | "flag";
/** `file` reads the value from a file the user picks; the console takes it as text. */
export type Control = "list" | "select" | "segmented" | "slider" | "number" | "text" | "toggle" | "file";

export interface ParamSpec<State> {
  readonly name: string;
  readonly kind: ParamKind;
  readonly title: string;
  readonly min?: number;
  /** The largest value; for text, the most characters. */
  readonly max?: number;
  readonly step?: number;
  readonly choices?: (state: State) => readonly string[];
  /** What a caller that leaves the parameter out gets, and what a control shows. */
  readonly value: (state: State) => ArgValue;
  readonly control?: Control;
}

export interface Outcome {
  readonly message: string;
  readonly digest?: string | null;
  readonly notes?: readonly string[];
}

export interface Action<State, Context> {
  readonly id: string;
  /** The console's word for it. */
  readonly alias: string;
  readonly title: string;
  /** The dock section that shows it; `null` for console and shortcut only. */
  readonly section: string | null;
  readonly params: readonly ParamSpec<State>[];
  /** Why it cannot run now, or `null`. */
  readonly available?: (state: State) => string | null;
  readonly run: (context: Context, args: Args) => Promise<Outcome> | Outcome;
}

export type RefusalCode = "unknown-action" | "unknown-param" | "bad-value" | "unavailable";

export class ActionRefusal extends Error {
  readonly code: RefusalCode;

  constructor(code: RefusalCode, message: string) {
    super(message);
    this.name = "ActionRefusal";
    this.code = code;
  }
}

export interface Resolved<State, Context> {
  readonly action: Action<State, Context>;
  readonly args: Args;
}

export interface Registry<State, Context> {
  readonly actions: readonly Action<State, Context>[];
  readonly find: (idOrAlias: string) => Action<State, Context> | undefined;
  readonly suggest: (word: string) => readonly string[];
  /** Checks everything and runs nothing. */
  readonly resolve: (idOrAlias: string, raw: RawArgs, state: State) => Resolved<State, Context>;
}

const FLAGS: ReadonlyMap<string, boolean> = new Map([
  ["on", true], ["true", true], ["1", true], ["yes", true],
  ["off", false], ["false", false], ["0", false], ["no", false],
]);

function distance(a: string, b: string): number {
  let row = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i += 1) {
    const next = [i];
    for (let j = 1; j <= b.length; j += 1) {
      const swap = (row[j - 1] ?? 0) + (a[i - 1] === b[j - 1] ? 0 : 1);
      next.push(Math.min(swap, (row[j] ?? 0) + 1, (next[j - 1] ?? 0) + 1));
    }
    row = next;
  }
  return row[b.length] ?? 0;
}

/**
 * The choice `input` names: itself, the one choice ending in `.input`, or the one choice
 * containing it. A list when it fits several or none: the candidates, never a guess.
 */
export function matchChoice(input: string, choices: readonly string[]): string | readonly string[] {
  const wanted = input.toLowerCase();
  const exact = choices.find((choice) => choice.toLowerCase() === wanted);
  if (exact !== undefined) return exact;
  const ending = choices.filter((choice) => choice.toLowerCase().endsWith(`.${wanted}`));
  if (ending.length === 1 && ending[0] !== undefined) return ending[0];
  const holding = wanted === "" ? [] : choices.filter((choice) => choice.toLowerCase().includes(wanted));
  return holding.length === 1 && holding[0] !== undefined ? holding[0] : holding;
}

function bad(name: string, message: string): ActionRefusal {
  return new ActionRefusal("bad-value", `\`${name}\` ${message}`);
}

function numberFrom<State>(spec: ParamSpec<State>, raw: unknown): number {
  const whole = spec.kind === "int";
  const text = typeof raw === "string" ? raw.trim() : "";
  const value = typeof raw === "number" ? raw : text === "" ? Number.NaN : Number(text);
  if (!Number.isFinite(value) || (whole && !Number.isInteger(value))) {
    throw bad(spec.name, `must be a ${whole ? "whole number" : "number"}, not ${JSON.stringify(raw)}`);
  }
  const min = spec.min ?? -Infinity;
  const max = spec.max ?? Infinity;
  if (value < min || value > max) throw bad(spec.name, `must be in ${min}..${max}, not ${value}`);
  return value;
}

function flagFrom<State>(spec: ParamSpec<State>, raw: unknown): boolean {
  if (typeof raw === "boolean") return raw;
  const flag = typeof raw === "string" ? FLAGS.get(raw.toLowerCase()) : undefined;
  if (flag === undefined) throw bad(spec.name, `must be on or off, not ${JSON.stringify(raw)}`);
  return flag;
}

function textFrom<State>(spec: ParamSpec<State>, raw: unknown): string {
  if (typeof raw === "number" || typeof raw === "boolean") return String(raw);
  if (typeof raw !== "string") throw bad(spec.name, `must be text, not ${JSON.stringify(raw)}`);
  // The length only: a refused text may be a whole file, and echoing it would copy it again.
  if (spec.max !== undefined && raw.length > spec.max) throw bad(spec.name, `is ${raw.length} characters; at most ${spec.max}`);
  return raw;
}

function choiceFrom<State>(spec: ParamSpec<State>, raw: unknown, state: State): string {
  const choices = spec.choices?.(state) ?? [];
  const match = matchChoice(textFrom(spec, raw), choices);
  if (typeof match === "string") return match;
  if (match.length > 0) throw bad(spec.name, `fits several: ${match.join(", ")}`);
  throw bad(spec.name, `is not one of: ${choices.join(", ")}`);
}

function valueFrom<State>(spec: ParamSpec<State>, raw: unknown, state: State): ArgValue {
  if (raw === undefined) return spec.value(state);
  if (spec.kind === "int" || spec.kind === "number") return numberFrom(spec, raw);
  if (spec.kind === "flag") return flagFrom(spec, raw);
  if (spec.kind === "choice") return choiceFrom(spec, raw, state);
  return textFrom(spec, raw);
}

function argsFrom<State, Context>(action: Action<State, Context>, raw: RawArgs, state: State): Args {
  const names = action.params.map((spec) => spec.name);
  for (const key of Object.keys(raw)) {
    if (names.includes(key)) continue;
    const takes = names.length === 0 ? "nothing" : names.join(", ");
    throw new ActionRefusal("unknown-param", `\`${action.alias}\` does not take \`${key}\`; it takes ${takes}`);
  }
  return Object.fromEntries(action.params.map((spec) => [spec.name, valueFrom(spec, raw[spec.name], state)]));
}

function indexOf<State, Context>(actions: readonly Action<State, Context>[]): Map<string, Action<State, Context>> {
  const byName = new Map<string, Action<State, Context>>();
  for (const action of actions) {
    if (byName.has(action.id)) throw new Error(`two actions share the id \`${action.id}\``);
    byName.set(action.id, action);
  }
  for (const action of actions) {
    const holder = byName.get(action.alias);
    if (holder !== undefined && holder !== action) throw new Error(`two actions share the alias \`${action.alias}\``);
    byName.set(action.alias, action);
  }
  return byName;
}

export function createRegistry<State, Context>(actions: readonly Action<State, Context>[]): Registry<State, Context> {
  const byName = indexOf(actions);
  const aliases = actions.map((action) => action.alias).sort();
  // Ponytail: nearest means at most two single-character edits away, so a word that is
  // wrong in three places gets the whole list instead. Nothing is ever run from a suggestion.
  const suggest = (word: string): readonly string[] => {
    const near = aliases.filter((alias) => distance(alias, word.toLowerCase()) <= 2);
    return near.length > 0 ? near : aliases;
  };
  return {
    actions,
    find: (idOrAlias) => byName.get(idOrAlias),
    suggest,
    resolve: (idOrAlias, raw, state) => {
      const action = byName.get(idOrAlias);
      if (action === undefined) {
        throw new ActionRefusal("unknown-action", `\`${idOrAlias}\` is not a command; nearest: ${suggest(idOrAlias).join(", ")}`);
      }
      // Asked before the values are read: with nothing drawn a value has nothing to be one of,
      // and "is not one of:" would hide the reason that matters.
      const reason = action.available?.(state) ?? null;
      if (reason !== null) throw new ActionRefusal("unavailable", `\`${action.alias}\` cannot run: ${reason}`);
      return { action, args: argsFrom(action, raw, state) };
    },
  };
}

/** The verb a host feature-tests with (`"applyDeltas" in el`, host-api condition 8). */
export const APPLY_DELTAS = "applyDeltas";

/** An object of named members — what a batch of nodes and edges arrives as. */
function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function text(record: Readonly<Record<string, unknown>>, name: string, at: string): string {
  const value = record[name];
  if (typeof value !== "string") throw bad(`${at}.${name}`, `must be text, not ${JSON.stringify(value ?? null)}`);
  return value;
}

function maybeText(record: Readonly<Record<string, unknown>>, name: string, at: string): string | null {
  const value = record[name];
  if (value === null) return null;
  if (typeof value !== "string") throw bad(`${at}.${name}`, `must be text or null, not ${JSON.stringify(value)}`);
  return value;
}

function count(record: Readonly<Record<string, unknown>>, name: string, at: string): number {
  const value = record[name];
  if (typeof value !== "number" || !Number.isFinite(value)) throw bad(`${at}.${name}`, `must be a number, not ${JSON.stringify(value ?? null)}`);
  return value;
}

function flag(record: Readonly<Record<string, unknown>>, name: string, at: string): boolean {
  const value = record[name];
  if (typeof value !== "boolean") throw bad(`${at}.${name}`, `must be on or off, not ${JSON.stringify(value ?? null)}`);
  return value;
}

function nodeOf(value: unknown, at: string): DeltaNode {
  if (!isRecord(value)) throw bad(at, "must be objects");
  const record = value;
  return {
    id: text(record, "id", at), kind: text(record, "kind", at), database_id: maybeText(record, "database_id", at),
    source: text(record, "source", at), label: text(record, "label", at), group: maybeText(record, "group", at),
    weight: count(record, "weight", at), version: count(record, "version", at),
    has_note: flag(record, "has_note", at), icon: maybeText(record, "icon", at),
  };
}

function edgeOf(value: unknown, at: string): DeltaEdge {
  if (!isRecord(value)) throw bad(at, "must be objects");
  const record = value;
  return {
    id: text(record, "id", at), source: text(record, "source", at), target: text(record, "target", at),
    kind: text(record, "kind", at), label: text(record, "label", at), strength: count(record, "strength", at),
    directed: flag(record, "directed", at), record_id: maybeText(record, "record_id", at),
    child_first: flag(record, "child_first", at),
  };
}

function entriesOf<T extends DeltaNode | DeltaEdge>(value: unknown, name: string, of: (entry: unknown, at: string) => T): readonly T[] {
  if (!Array.isArray(value)) throw bad(name, `must be an array, not ${JSON.stringify(value ?? null)}`);
  return value.map((entry, index) => of(entry, `${name}[${index}]`));
}

/**
 * The batch `applyDeltas` takes, checked here where every other value is, and rebuilt member by
 * member: the motor's reader refuses an unknown member, so a host's batch is normalised to the
 * shape it reads.
 */
export function deltaBatch(raw: unknown): GraphBatch {
  if (!isRecord(raw)) throw bad("batch", "must be an object with a nodes array and an edges array");
  return { nodes: entriesOf(raw["nodes"], "nodes", nodeOf), edges: entriesOf(raw["edges"], "edges", edgeOf) };
}

export interface Deltas {
  readonly id: typeof APPLY_DELTAS;
  /** Checks the batch and sends it; rejects with an `ActionRefusal` or the motor's own error. */
  readonly apply: (batch: unknown) => Promise<number>;
}

/**
 * `applyDeltas`, registered and resolved in this file next to every other verb — asked before
 * the batch is read, refused with a reason when no live session can take it.
 *
 * Ponytail: a sibling of `resolve` rather than an entry in the dock's actions, because an
 * `Action`'s arguments are `Args` (`Record<string, ArgValue>`) and no control, console line or
 * dock button can carry a batch of nodes. Failing input: a batch that is not `{nodes, edges}` of
 * objects is refused here, before the worker is asked. Direction: the batch is rebuilt member
 * by member, so what the motor stages is what this read. Escape hatch: the batch succeeds whole
 * or is refused whole; there is no per-id refusal.
 */
export function createDeltas(reason: () => string | null, send: (batch: GraphBatch) => Promise<number>): Deltas {
  return {
    id: APPLY_DELTAS,
    apply: async (batch) => {
      const because = reason();
      if (because !== null) throw new ActionRefusal("unavailable", `\`${APPLY_DELTAS}\` cannot run: ${because}`);
      return send(deltaBatch(batch));
    },
  };
}
