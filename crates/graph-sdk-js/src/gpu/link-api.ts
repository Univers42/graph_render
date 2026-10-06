/**
 * `link-api.ts` — the public `probeLink`, the link pass with no fault parameter.
 *
 * The same split as `charge-api.ts`, for its reason (`charge-api.ts:1-13`): `runLink` takes a
 * fault as its second argument so the harness can run a control, and a public signature that
 * took one would let a caller make the tier report a failure it did not have. So the public
 * function is this wrapper, and the fault stays the harness's.
 */

import { runLink } from "./link.ts";
import type { LinkRequest } from "./link.ts";
import type { PassReport } from "./pass-report.ts";

export type { LinkRequest, PassReport };

/**
 * The link pass for one fixture: the per-node gather, run twice, against `delta_link`.
 *
 * The report's `pass` is false when the derived guard, this arm's measured ceiling or the
 * two runs' byte equality failed. Throws a `Refusal` when there is no adapter, when `arm`
 * refuses the one offered, or when a limit the pass needs is below the device's.
 */
export function probeLink(request: LinkRequest): Promise<PassReport> {
  return runLink(request, undefined);
}
