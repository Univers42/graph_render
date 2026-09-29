/**
 * One shape for every failure the studio shows, so a refusal from the motor, a refused
 * document and a bug in a panel land in the same place with the same fields.
 *
 * Reads the error's own members instead of its class: an error that crossed the worker
 * boundary has lost its prototype, and this module stays free of the SDK.
 */

export interface ShownError {
  /** The error's own name (`RunRefusedError`), never a bare "Error" when more is known. */
  readonly title: string;
  /** The wire code and its name, when the failure came from the ABI. */
  readonly code: string | null;
  readonly detail: string;
  /** What the reader can do about it. */
  readonly hint: string;
}

const DEFAULT_HINT = "Unexpected studio error — see the browser console for the stack.";

const HINTS: ReadonlyMap<string, string> = new Map([
  ["BuildRefusedError", "The ingest document was refused. Check the JSON, or regenerate the synthetic graph."],
  ["RunRefusedError", "The layout refused to run on this graph. Pick another layout, or load a graph it accepts."],
  ["PostRefusedError", "The edge pass refused this drawing. The layout's own edges are shown instead."],
  ["AnalysisRefusedError", "The analysis refused this graph. Pick another analysis."],
  ["InvalidHandleError", "The graph handle is no longer live. Load the graph again."],
  ["TamperedGeometryError", "A column view wrote a non-finite value. Reload the page and rebuild."],
  ["MotorTrapError", "The wasm module trapped. Reload the page; if it repeats, that stage is broken."],
  ["WasmUnavailableError", "The wasm motor did not load, so no layout can run. Run scripts/studio.sh so graph_wasm.wasm is in app/public/."],
  ["InvalidOptionsError", "The motor rejected the options the studio passed. This is a studio bug."],
  ["IngestRefusal", "The document is not the ingest shape. Fix the JSON, or load one of the bundled fixtures."],
  ["SnapshotRefusal", "The motor's snapshot could not be read. That is a motor or decoder bug — nothing was drawn from it."],
  ["ActionRefusal", "Type `help` in the console for the commands and their values."],
  ["SettingsRefusal", "The recipe does not hold settings this studio reads. Export a fresh recipe."],
  ["CancelledError", "The run was stopped. Nothing changed."],
]);

function memberOf(error: Error, name: string): unknown {
  return Object.entries(error).find(([key]) => key === name)?.[1];
}

function reasonOf(error: Error): string {
  const reason = memberOf(error, "reason");
  if (reason === undefined || reason === null) return "";
  return ` — ${reason instanceof Error ? reason.message : String(reason)}`;
}

function codeOf(error: Error): string | null {
  const code = memberOf(error, "code");
  const codeName = memberOf(error, "codeName");
  if (typeof code !== "number" || typeof codeName !== "string") return null;
  return `code ${code} (${codeName})`;
}

export function describeError(error: unknown): ShownError {
  if (!(error instanceof Error)) {
    return { title: "Error", code: null, detail: String(error), hint: DEFAULT_HINT };
  }
  return {
    title: error.name,
    code: codeOf(error),
    detail: `${error.message}${reasonOf(error)}`,
    hint: HINTS.get(error.name) ?? DEFAULT_HINT,
  };
}
