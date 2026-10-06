/**
 * `tick-api.ts` — the public `probeTick`, the resident tick with no fault parameter.
 *
 * The same split as `charge-api.ts` and `link-api.ts`, for their reason (`charge-api.ts:1-13`):
 * `runTick` takes a fault as its second argument so the harness can run a control, and a public
 * signature that took one would let a caller make the tier report a failure it did not have. So
 * the public function is this wrapper, and the fault stays the harness's.
 */

import { runTick } from "./tick.ts";
import type { TickReport, TickRequest } from "./tick.ts";

export type { TickReport, TickRequest };

/**
 * The resident tick for one fixture: the passes in graph-core's order, the buffers across ticks.
 *
 * `request.fixture` is the `.gmfx` bytes exactly as `emit-gpu-fixtures` wrote them, and
 * `request.ticks` is how many ticks to run (the first is the warm-up). The report's `pass` is
 * false when any check failed. Throws a `Refusal` when there is no adapter, when `arm` refuses
 * the one offered, or when a limit the tick needs is below the device's.
 */
export function probeTick(request: TickRequest): Promise<TickReport> {
  return runTick(request, undefined);
}
