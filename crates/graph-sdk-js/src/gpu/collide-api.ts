/**
 * `collide-api.ts` — the public `probeCollide`, the collide pass without its fault parameter.
 *
 * A separate wrapper for `charge-api.ts:1-13`'s reason: `runCollide`'s second parameter is the
 * harness's `--break`, and a public signature that took it would let a caller make the tier
 * report a failure it did not have.
 */

import { runCollide } from "./collide.ts";
import type { CollideRequest } from "./collide.ts";
import type { PassReport } from "./pass-report.ts";

export type { CollideRequest, PassReport };

/**
 * The collide pass for one fixture: hash, scan, scatter, resolve, twice, against the fixture's
 * `delta_collide`. The report's `pass` is false when the guard or a ceiling is exceeded, when
 * the two runs' bytes differ, or when a bucket's member list is not ascending (`exact.order`).
 *
 * Throws a `Refusal` when there is no adapter, when the adapter is refused by `arm`, or when a
 * limit the pass needs is below what the device reports.
 */
export function probeCollide(request: CollideRequest): Promise<PassReport> {
  return runCollide(request, undefined);
}
