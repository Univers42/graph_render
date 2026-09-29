/**
 * The motor, as React state: loaded once, degraded or live, with the layout
 * registry that only a live motor can answer.
 */

import { useEffect, useState } from "react";

import { describeError, type ShownError } from "../core/errors.ts";
import { MotorSession } from "../motor/session.ts";

/** Where the wasm module is; `scripts/studio.sh` copies it into `public/`. */
export const WASM_URL = "graph_wasm.wasm";

export interface MotorState {
  readonly session: MotorSession | null;
  readonly layouts: readonly string[];
  readonly degraded: ShownError | null;
  readonly available: boolean;
}

export function useMotorSession(fail: (error: unknown) => void): MotorState {
  const [session, setSession] = useState<MotorSession | null>(null);
  const [layouts, setLayouts] = useState<readonly string[]>([]);
  const [degraded, setDegraded] = useState<ShownError | null>(null);

  useEffect(() => {
    let cancelled = false;
    void MotorSession.open(WASM_URL).then(
      (opened) => {
        if (cancelled) {
          opened.release();
          return;
        }
        setSession(opened);
        try {
          // The one call that says whether the module really loaded: a degraded
          // motor refuses here with the latched WasmUnavailableError (the reason
          // worth showing), a live one returns the registry (C1) and the picker
          // fills from it.
          setLayouts(opened.layouts());
        } catch (error) {
          setDegraded(describeError(error));
        }
      },
      (error: unknown) => fail(error),
    );
    return () => {
      cancelled = true;
    };
  }, [fail]);

  return { session, layouts, degraded, available: session !== null && session.available };
}
