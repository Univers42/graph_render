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
  /** The code the failure names itself with: the ABI's number and name, or a reader's own code. */
  readonly code: string | null;
  readonly detail: string;
  /** What the reader can do about it. */
  readonly hint: string;
}

const DEFAULT_HINT = "Unexpected studio error — see the browser console for the stack.";

const HINTS: ReadonlyMap<string, string> = new Map([
  ["BuildRefusedError", "The ingest document was refused. Check the JSON, or regenerate the synthetic graph."],
  ["RunRefusedError", "The layout refused to run on this graph. Pick another layout, or load a graph it accepts."],
  ["PostRefusedError", "The edge pass refused: it needs a finished layout run on the same graph. The layout's own edges are shown instead."],
  ["AnalysisRefusedError", "The analysis refused this graph. It is a function of the topology: pick another analysis, or load the graph again."],
  ["InvalidHandleError", "The graph handle is no longer live. Load the graph again."],
  ["TamperedGeometryError", "A column view wrote a non-finite value. Reload the page and rebuild."],
  ["MotorTrapError", "The wasm module trapped. Reload the page; if it repeats, that stage is broken."],
  ["WasmUnavailableError", "The wasm motor did not load, so no layout can run. Run scripts/studio.sh so graph_wasm.wasm is in app/public/."],
  ["InvalidOptionsError", "The motor rejected the options the studio passed. This is a studio bug."],
  ["IngestRefusal", "The document is not the ingest shape. Fix the JSON, or load one of the bundled fixtures."],
  ["SnapshotRefusal", "The motor's snapshot could not be read. This studio draws 2D only, so a 3D snapshot is refused by design; anything else is a motor or decoder bug, and nothing was drawn from it."],
  ["ActionRefusal", "Type `help` in the console for the commands and their values."],
  ["SettingsRefusal", "The recipe does not hold settings this studio reads. Export a fresh recipe."],
  ["CancelledError", "The run was stopped. Nothing changed."],
  ["CommandRefusal", "Type `help` in the console for the commands and their values."],
  ["QueryRefusal", "A field is one of `id` `tag` `kind` `db` `path` `degree`; join with AND and OR, and group with `(` `)`."],
  ["RecipeMismatch", "This motor does not draw what the recipe recorded. Compare the motor builds, or export a fresh recipe."],
  ["MetaMismatch", "The snapshot and the document name different nodes. That is a studio or motor bug — nothing was drawn from it."],
  ["SessionRefusal", "Load a graph first."],
]);

function memberOf(error: Error, name: string): unknown {
  return Object.entries(error).find(([key]) => key === name)?.[1];
}

function textOf(value: unknown): string {
  if (value instanceof Error) return value.message;
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return Object.prototype.toString.call(value);
}

function reasonOf(error: Error): string {
  const reason = memberOf(error, "reason");
  if (reason === undefined || reason === null) return "";
  return ` — ${textOf(reason)}`;
}

function codeOf(error: Error): string | null {
  const code = memberOf(error, "code");
  // A reader or a refusal class names itself with a string; the ABI names a wire code with a
  // number and its code name. Both are the code the reader is meant to see.
  if (typeof code === "string") return code;
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
