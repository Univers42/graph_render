/**
 * One shape for every failure the studio can show, so a refusal from the ABI, a
 * refused ingest document and a bug in a panel all land in the same banner with
 * the same three fields — and none of them can reach the page as a blank screen.
 */

import { GraphMotorError, WasmUnavailableError } from "../../../crates/graph-sdk-js/src/index.ts";

export interface ShownError {
  /** The error's own name (`RunRefusedError`), never a generic "Error" when the
   *  ABI told us more. */
  readonly title: string;
  /** The wire code and its name, when the failure came from the ABI. */
  readonly code: string | null;
  readonly detail: string;
  /** What the reader can do about it. */
  readonly hint: string;
}

const HINTS: Record<string, string> = {
  BuildRefusedError: "The ingest document was refused. Check the JSON, or regenerate the synthetic graph.",
  RunRefusedError: "The layout refused to run on this graph. Pick another layout, or load a graph it accepts.",
  PostRefusedError:
    "The post pass refused. It needs a finished layout run on the same handle, so run a layout first, then apply it.",
  AnalysisRefusedError:
    "The analysis refused. It is a function of the topology, so rebuild the graph if the handle is stale.",
  InvalidHandleError: "The graph handle is no longer live. Rebuild the graph and run again.",
  TamperedGeometryError: "A column view wrote a non-finite value. Reload the page and rebuild.",
  MotorTrapError: "The wasm module trapped. Reload the page; if it repeats, that layout is broken.",
  WasmUnavailableError: "The wasm motor did not load, so no layout can run. Run scripts/studio.sh so graph_wasm.wasm is in app/public/.",
  InvalidOptionsError: "The motor rejected the options the studio passed. This is a studio bug.",
  IngestRefusal: "The document is not the ingest shape. Fix the JSON, or load one of the bundled fixtures.",
  DrawListError: "The run's columns are not self-consistent. That is an engine bug — the layout above is suspect.",
  Default: "Unexpected studio error — see the browser console for the stack.",
};

/** The reason carried by a loader failure, appended to its message. */
function reasonOf(error: WasmUnavailableError): string {
  const reason: unknown = error.reason;
  if (reason === undefined || reason === null) return "";
  return ` — ${reason instanceof Error ? reason.message : String(reason)}`;
}

export function describeError(error: unknown): ShownError {
  if (error instanceof WasmUnavailableError) {
    return {
      title: error.name,
      code: null,
      detail: `${error.message}${reasonOf(error)}`,
      hint: HINTS.WasmUnavailableError,
    };
  }
  if (error instanceof GraphMotorError) {
    return {
      title: error.name,
      code: error.code === undefined ? null : `code ${error.code} (${error.codeName})`,
      detail: error.message,
      hint: HINTS[error.name] ?? HINTS.Default,
    };
  }
  if (error instanceof Error) {
    return { title: error.name, code: null, detail: error.message, hint: HINTS[error.name] ?? HINTS.Default };
  }
  return { title: "Error", code: null, detail: String(error), hint: HINTS.Default };
}
