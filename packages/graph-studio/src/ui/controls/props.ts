/** What every control is given, and the two things it may do with it. */
import type { StudioParam } from "../../actions/context.ts";
import type { ArgValue } from "../../actions/registry.ts";

/** One parameter's control: what to show, what the draft says now, and where to put a change. */
export interface ControlProps {
  readonly spec: StudioParam;
  readonly value: ArgValue;
  readonly choices: readonly string[];
  readonly disabled: boolean;
  /**
   * The action declares a parameter called `name`. A file control needs this because the
   * six members above cannot say it: a picked file names itself, and only the action knows
   * whether there is somewhere to put that name.
   */
  readonly named: boolean;
  /** Puts the values in the draft without running anything. */
  readonly onDraft: (patch: Readonly<Record<string, ArgValue>>) => void;
  /** Puts the values in the draft and runs the action with all of it. */
  readonly onCommit: (patch: Readonly<Record<string, ArgValue>>) => void;
}
