import { InvalidOptionsError } from "./errors.ts";
import type { MotorOptions } from "./types.ts";
import type { MotorThreads } from "./threads.ts";

/** A bound on a runaway count, not a tuning: each helper holds a 4 MiB stack in linear memory,
 * and 8 workers already gain only 1.16x over 4 at 400k (`docs/measurements/perf-p3-session-threads.md`). */
const MAX_HELPERS = 15;

function checkThreads(threads: MotorThreads): void {
  if (!Number.isInteger(threads.helpers) || threads.helpers < 0 || threads.helpers > MAX_HELPERS) {
    throw new InvalidOptionsError(`options.threads.helpers must be an integer in 0..${MAX_HELPERS}, got ${String(threads.helpers)}`);
  }
  if (typeof threads.spawn !== "function") throw new InvalidOptionsError("options.threads.spawn must be a function");
  if (threads.flags !== undefined && (!Number.isInteger(threads.flags) || threads.flags < 0 || threads.flags > 1)) {
    throw new InvalidOptionsError(`options.threads.flags must be 0 or 1, got ${String(threads.flags)}`);
  }
}

/** `MotorOptions`'s closed shape (C16): an unknown key, an `exec` value other than `"auto"` or
 * a malformed `threads` is refused loudly rather than silently ignored. */
export function checkOptions(options: MotorOptions | undefined): void {
  if (options === undefined) return;
  for (const key of Object.keys(options)) {
    if (key !== "exec" && key !== "threads") throw new InvalidOptionsError(`unknown option "${key}"`);
  }
  if (options.exec !== undefined && options.exec !== "auto") {
    throw new InvalidOptionsError(
      `options.exec must be "auto" this phase, got ${JSON.stringify(options.exec)} (docs/decisions/compute-tiers.md)`,
    );
  }
  if (options.threads !== undefined) checkThreads(options.threads);
}
