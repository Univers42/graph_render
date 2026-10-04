/**
 * The edge pass a run ends with. A refused pass is a note on the run, not a failed run: the
 * layout's own edges stay in place and are drawn.
 */
import { type ShownError, describeError } from "../state/errors.ts";

export interface Pass {
  readonly postId: string | null;
  readonly postError: ShownError | null;
  readonly postMs: number;
}

/** The one member of the motor the pass calls, so this module needs nothing else of it. */
interface PostMotor<Handle> {
  post(handle: Handle, postId: string): unknown;
}

export function runPass<Handle>(motor: PostMotor<Handle>, handle: Handle, postId: string | null, now: () => number): Pass {
  if (postId === null) return { postId, postError: null, postMs: 0 };
  const started = now();
  try {
    motor.post(handle, postId);
    return { postId, postError: null, postMs: now() - started };
  } catch (error) {
    return { postId: null, postError: describeError(error), postMs: now() - started };
  }
}
