/**
 * The parity page's entry: the fixture drawn by the studio's own painter, with its state on
 * `window.__parity` for scripts/studio-parity.sh. No motor, no layout, no dock — the
 * positions are SciGraphs' own (docs/decisions/scigraphs-reference-fixture.md).
 */
import { mountParity } from "../../packages/graph-studio/src/parity/page.ts";

const canvas = document.querySelector("canvas");
if (canvas === null) throw new Error("parity page: the page has no canvas to draw on");

try {
  window.__parity = await mountParity(canvas, "./");
} catch (error) {
  window.__parityError = error instanceof Error ? error.message : String(error);
}
