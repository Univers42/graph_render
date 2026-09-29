import { InvalidOptionsError } from "./errors.ts";
import type { MotorOptions } from "./types.ts";

/** `MotorOptions`'s closed shape this phase (C16): an unknown key, or an `exec` value
 * other than `"auto"`, is refused loudly rather than silently ignored. */
export function checkOptions(options: MotorOptions | undefined): void {
  if (options === undefined) return;
  for (const key of Object.keys(options)) {
    if (key !== "exec") throw new InvalidOptionsError(`unknown option "${key}"`);
  }
  if (options.exec !== undefined && options.exec !== "auto") {
    throw new InvalidOptionsError(
      `options.exec must be "auto" this phase, got ${JSON.stringify(options.exec)} (docs/decisions/compute-tiers.md)`,
    );
  }
}
