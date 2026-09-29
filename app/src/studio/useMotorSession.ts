/**
 * The motor, as React state: loaded once, degraded or live, with the three
 * registries only a live motor can answer — layouts, POST passes, analyses.
 */

import { useEffect, useState } from "react";

import { describeError, type ShownError } from "../core/errors.ts";
import { MotorSession } from "../motor/session.ts";

/** Where the wasm module is; `scripts/studio.sh` copies it into `public/`. */
export const WASM_URL = "graph_wasm.wasm";

export interface MotorState {
  readonly session: MotorSession | null;
  readonly layouts: readonly string[];
  readonly posts: readonly string[];
  readonly analyses: readonly string[];
  readonly degraded: ShownError | null;
  readonly available: boolean;
}

export function useMotorSession(fail: (error: unknown) => void): MotorState {
  const [session, setSession] = useState<MotorSession | null>(null);
  const [layouts, setLayouts] = useState<readonly string[]>([]);
  const [posts, setPosts] = useState<readonly string[]>([]);
  const [analyses, setAnalyses] = useState<readonly string[]>([]);
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
          // worth showing), a live one returns its registries (C1) and every
          // picker fills from them. All three are read in the same block so the
          // three panels cannot disagree about which build is loaded.
          setLayouts(opened.layouts());
          setPosts(opened.posts());
          setAnalyses(opened.analyses());
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

  return {
    session, layouts, posts, analyses, degraded,
    available: session !== null && session.available,
  };
}
