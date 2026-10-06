/**
 * `charge-api.ts` — the public `probeCharge`, the one function the harness's page calls.
 *
 * It is a separate file and not a re-export of `charge.ts`'s `runCharge` because the two take
 * different arguments and the difference is the whole of settled point 3: the public
 * `ChargeRequest` carries **no fault knob**, so a caller cannot inject a fault by constructing
 * a request, while the harness's page calls `runCharge(request, fault)` directly with the fault
 * it was given on argv.
 *
 * That is why this wrapper exists rather than `probeCharge = runCharge`: `runCharge`'s second
 * parameter would become part of the public signature, and a public signature that takes a
 * fault string is a public signature a caller can use to make the tier report a failure it did
 * not have.
 */

import { runCharge } from "./charge.ts";
import type { ChargeReport, ChargeRequest } from "./charge.ts";

export type { ChargeReport, ChargeRequest };

/**
 * The charge pass for one fixture: bounds, deposit, FFT, kernel, field read, compare.
 *
 * `request.fixture` is the `.gmfx` bytes exactly as `emit-gpu-fixtures` wrote them, and
 * `request.arm` is which adapter to insist on; `"any"` takes what the device offers. The
 * returned report's `pass` is false when any guard or ceiling is exceeded, or when one of the
 * exactness checks — the fixed-point total, the two runs' byte equality, the extent — failed.
 *
 * Throws a `Refusal` when there is no adapter, when the adapter is refused by `arm`, or when a
 * limit the pass needs is below what the device reports. A refusal is not a report: it is the
 * arm declining, and the harness turns it into exit 3 as `webgpu.py:236-250` does.
 */
export function probeCharge(request: ChargeRequest): Promise<ChargeReport> {
  return runCharge(request, undefined);
}