import { InvalidOptionsError } from "./errors.ts";
import type { MotorOptions } from "./types.ts";

/** `MotorOptions`'s closed shape this phase (C16): an unknown key, or an `exec` value
 * other than `"auto"`, is refused loudly rather than silently ignored.
 *
 * A non-object argument is refused before it is read: `Object.keys(null)` throws a bare
 * `TypeError` from inside this module, which tells a caller nothing about which option is
 * wrong, and a non-object `Object.keys` accepts (`[]`, `"auto"`, `7`) would otherwise pass
 * straight through the key scan. */
export function checkOptions(options: MotorOptions | undefined): void {
  if (options === undefined) return;
  if (typeof options !== "object" || options === null || Array.isArray(options)) {
    throw new InvalidOptionsError(
      `options must be an object or undefined, got ${Array.isArray(options) ? "an array" : typeof options}`,
    );
  }
  for (const key of Object.keys(options)) {
    if (key !== "exec") throw new InvalidOptionsError(`unknown option "${key}"`);
  }
  if (options.exec !== undefined && options.exec !== "auto") {
    throw new InvalidOptionsError(
      `options.exec must be "auto" this phase, got ${JSON.stringify(options.exec)} (docs/decisions/compute-tiers.md)`,
    );
  }
}
