/**
 * Failures, newest first, capped at four. One sink for the whole studio: the
 * hooks below report into it and the banner above the graph reads it, so a
 * refusal anywhere has exactly one place to appear.
 */

import { useCallback, useState } from "react";

import { describeError, type ShownError } from "../core/errors.ts";

export interface FailureSink {
  readonly errors: readonly ShownError[];
  readonly fail: (error: unknown) => void;
  readonly clear: () => void;
}

export function useFailureSink(): FailureSink {
  const [errors, setErrors] = useState<readonly ShownError[]>([]);
  const fail = useCallback((error: unknown) => {
    setErrors((previous) => [describeError(error), ...previous].slice(0, 4));
  }, []);
  const clear = useCallback(() => setErrors([]), []);
  return { errors, fail, clear };
}
