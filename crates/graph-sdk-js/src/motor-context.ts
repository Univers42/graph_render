// What a loaded module becomes before any call reaches it: the raw exports, the view epoch, the
// registries, the parameter caches and the per-handle kind cache. Split out of `motor.ts`, which
// is at the house's 300-line limit, because this is the one piece of loading that is not about
// the ABI: everything it builds is per-motor state that no call site may construct for itself, so
// a second construction would be a second epoch counter and a second kind cache.
//
// It is a function rather than a constructor argument so the failure has one owner: a module
// that does not load throws *here*, and `Motor.create`'s catch turns that into the degraded motor
// the loader pattern promises.

import { type WasmSource, loadMotor } from "./wasm.ts";
import { loadThreaded } from "./threads.ts";
import { ColumnViews } from "./views.ts";
import { LayoutParams } from "./params.ts";
import { Registries } from "./registries.ts";
import type { GeometryKinds } from "./geometry-kinds.ts";
import type { StageContext } from "./stages.ts";
import type { Handle, MotorOptions } from "./types.ts";

/** Loads `source` and binds every piece of per-motor state {@link StageContext} names, or
 *  throws whatever the loader threw — `Motor.create` is what turns that into a `Motor`. */
export async function loadContext(source: WasmSource, options?: MotorOptions): Promise<StageContext> {
  const exports = await (options?.threads === undefined ? loadMotor(source) : loadThreaded(source, options.threads));
  return {
    exports,
    views: new ColumnViews(exports),
    registries: new Registries(),
    kinds: new Map<Handle, GeometryKinds>(),
    params: new LayoutParams(),
  };
}
