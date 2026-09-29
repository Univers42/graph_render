/** What an action may touch. The studio builds it; an action never reaches past it. */
import type { StudioState } from "../state/model.ts";
import type { Settings, Source } from "../state/settings.ts";
import type { Pipeline, ViewFace } from "../studio/pipeline.ts";
import type { Reveal } from "../studio/reveal.ts";
import type { Action, ArgValue, Args, ParamSpec } from "./registry.ts";

/** Hands a file to whoever is using the studio: a download on a page, a message in a host. */
export type Save = (name: string, data: Blob) => void;

export interface StudioContext extends Pipeline {
  readonly state: () => StudioState;
  readonly view: ViewFace;
  /** Stops what the motor is doing. False when it was doing nothing. */
  readonly stop: () => boolean;
  readonly save: Save;
  readonly clearLog: () => void;
  readonly animation: Reveal;
  readonly actions: () => readonly StudioAction[];
  /** The settings last kept for `source`, or null. */
  readonly recall: (source: Source) => Settings | null;
}

export type StudioAction = Action<StudioState, StudioContext>;
export type StudioParam = ParamSpec<StudioState>;
export type Change = (settings: Settings) => Settings;

function valueOf(args: Args, name: string): ArgValue {
  const value = args[name];
  if (value === undefined) throw new Error(`the action read \`${name}\`, which it does not declare`);
  return value;
}

export function textArg(args: Args, name: string): string {
  return String(valueOf(args, name));
}

export function numberArg(args: Args, name: string): number {
  return Number(valueOf(args, name));
}

export function flagArg(args: Args, name: string): boolean {
  return valueOf(args, name) === true;
}

/** `value` as one of `choices`; the registry has already refused anything else. */
export function chosen<Choice extends string>(choices: readonly Choice[], value: string, fallback: Choice): Choice {
  return choices.find((choice) => choice === value) ?? fallback;
}
