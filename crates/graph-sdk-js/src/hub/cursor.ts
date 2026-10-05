import { GraphMotorError } from "../errors.ts";
import { breaking } from "./wire.ts";

/** The largest `epoch` or `seq` JavaScript reads exactly: 2^53 − 1 (§5). */
export const MAX_SEQ = 9007199254740991;

/** A change-stream position: two halves, each a `u64` the SDK reads exactly. */
export interface Cursor {
  readonly epoch: number;
  readonly seq: number;
}

/** A cursor that is not `<epoch>.<seq>`, two decimal halves each below 2^53 (§7). */
export class InvalidCursorError extends GraphMotorError {
  override readonly name = "InvalidCursor";
}

/** Reads a change-stream position `<epoch>.<seq>`.
 *
 * The two halves are read one at a time and never through `parseFloat` over the whole string:
 * `"9007199254740992.1"` as one number is about 9.007 × 10^15, reads back as a plausible epoch,
 * and only the split reader sees that the epoch half sits at 2^53. The `isSafeInteger` guard is
 * the whole point of the split — `Number("9007199254740992")` is exactly representable and would
 * pass a range check written as `Number.isInteger`.
 */
export function parseCursor(text: string): Cursor {
  if (breaking("parsefloat")) return broken(text);
  const halves = text.split(".");
  if (halves.length !== 2) throw new InvalidCursorError(`a cursor is <epoch>.<seq>: ${text}`);
  return { epoch: halfOf(halves[0]!, text), seq: halfOf(halves[1]!, text) };
}

/** Writes a change-stream position back as `<epoch>.<seq>`, the spelling the hub sends. */
export function formatCursor(cursor: Cursor): string {
  return `${cursor.epoch}.${cursor.seq}`;
}

/** The `negctl-hub-sdk-cursor` break: one `parseFloat` over the epoch half, no grammar and no
 * 2^53 guard. `hub_cursor_accepts_the_ceiling_and_refuses_one_past_it` is the test that reds. */
function broken(text: string): Cursor {
  const halves = text.split(".");
  return { epoch: parseFloat(halves[0] ?? ""), seq: Number(halves[1]) };
}

function halfOf(half: string, text: string): number {
  if (!/^(0|[1-9][0-9]*)$/.test(half)) throw new InvalidCursorError(`a cursor half is decimal: ${text}`);
  const value = Number(half);
  if (!Number.isSafeInteger(value) || value > MAX_SEQ) {
    throw new InvalidCursorError(`a cursor half is below 2^53: ${text}`);
  }
  return value;
}
