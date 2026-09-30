import { AnalysisRefusedError } from "./errors.ts";
import type { AnalysisResult, AnalysisValueKind } from "./types.ts";

/** One optional member of an analysis face, when it is present and of the right type.
 *  An absent member is `undefined`; a member of the *wrong* type is a refusal, never a
 *  silent `undefined` — a caller told `converged: undefined` would read it as "no flag
 *  was handed back" and trust numbers that were never verified.
 */
function optionalBoolean(face: Record<string, unknown>, key: string, asked: string): boolean | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (typeof value !== "boolean") throw new AnalysisRefusedError(`${asked}: ${key} is not a boolean`);
  return value;
}

/** As {@link optionalBoolean}, for a number. `max` is a depth level and `modularity` a
 *  score; both are wire numbers, and a non-number in either is a broken face.
 */
function optionalNumber(face: Record<string, unknown>, key: string, asked: string): number | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new AnalysisRefusedError(`${asked}: ${key} is not a finite number`);
  }
  return value;
}

/** The ABI's canonical JSON face (`docs/contract/wasm-abi.md` "ANALYSIS"), parsed into
 *  {@link AnalysisResult} and **checked member by member rather than cast**. A face whose
 *  `id`, `kind` or `nodeCount` did not agree with itself would otherwise reach a caller
 *  as a plausible-looking object holding someone else's numbers, which is the one failure
 *  mode a typed wrapper exists to prevent — so each disagreement is an
 *  {@link AnalysisRefusedError} naming what did not match.
 */
export function parseAnalysisFace(text: string, asked: string): AnalysisResult {
  const parsed: unknown = JSON.parse(text);
  if (typeof parsed !== "object" || parsed === null) {
    throw new AnalysisRefusedError(`${asked}: the result is not a JSON object`);
  }
  const face = parsed as Record<string, unknown>;
  const { id, kind, nodeCount, values } = face;
  if (id !== asked) {
    throw new AnalysisRefusedError(`${asked}: the motor answered for "${String(id)}"`);
  }
  if (kind !== "f64" && kind !== "u32") {
    throw new AnalysisRefusedError(`${asked}: unknown value kind ${JSON.stringify(kind)}`);
  }
  if (!Array.isArray(values) || values.some((v) => typeof v !== "number")) {
    throw new AnalysisRefusedError(`${asked}: values is not an array of numbers`);
  }
  const numbers = values as number[];
  if (nodeCount !== numbers.length) {
    throw new AnalysisRefusedError(`${asked}: nodeCount ${String(nodeCount)} but ${numbers.length} values`);
  }
  const typedKind: AnalysisValueKind = kind;
  return {
    id,
    kind: typedKind,
    nodeCount: numbers.length,
    values: numbers,
    converged: optionalBoolean(face, "converged", asked),
    modularity: optionalNumber(face, "modularity", asked),
    max: optionalNumber(face, "max", asked),
  };
}
