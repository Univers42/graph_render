/**
 * The hover line, as text. It lives in its own module rather than inside the
 * panel so the unit tests can reach it without a JSX toolchain — `node --test
 * --experimental-strip-types` runs the studio's `.ts` helpers directly, and a
 * component file is not one of them.
 */

import type { AnalysisResult } from "../../../crates/graph-sdk-js/src/index.ts";
import { hoverValue } from "../core/analysis.ts";
import type { NodeStyle } from "../render/palette.ts";

/**
 * The line for the node under the cursor: its label, its kind, and the dense
 * index the click handlers speak in — plus the applied analysis's value for that
 * node, which is the number the colour on the canvas is standing for. An index
 * the face does not cover adds nothing rather than another node's value.
 */
export function tooltipText(
  styles: readonly NodeStyle[],
  index: number,
  analysis: AnalysisResult | null,
): string {
  const style = styles[index];
  const head = style === undefined ? `#${index}` : `${style.label} · ${style.kind} · #${index}`;
  const value = analysis === null ? null : hoverValue(analysis, index);
  return value === null ? head : `${head} · ${value}`;
}
