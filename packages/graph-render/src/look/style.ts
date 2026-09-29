/**
 * The look style: the SciGraphs node pass as one `Style` for the studio's painter. The
 * colour path is the library's own — `coloursOf` walks the generated ramp, `cssOf` encodes
 * once, and the labels come from `selectLabels` in the source's own order. Nothing here
 * computes a colour.
 *   SciGraphs/core/scigraphs_core/coloring/colormaps.py:520-543 (the value that is coloured)
 *   SciGraphs/core/repro/executor.py:498-507 (rank, declutter, then the cut)
 *   SciGraphs/core/visualization/text_overlay.py:266-326 (the declutter)
 */
import { coloursOf } from "../colour/colormap.ts";
import type { Rgb } from "../colour/srgb.ts";
import { cssOf } from "../colour/srgb.ts";
import { type LabelBox, selectLabels } from "../labels2d/declutter.ts";
import { type Style, styleFrom } from "../style.ts";
import { LABEL_FONT_PX, LABEL_LIMIT, type Look } from "./presets.ts";

export interface LookStyleInput {
  readonly look: Look;
  /** The normalised value per node, or null when no metric is coloured in. */
  readonly norm: Float64Array | null;
  /** The ranking score per node: the labels are ordered by it (executor.py:498-501). */
  readonly scores: Float64Array;
  /**
   * 1 for a node the labels may be drawn on, or null for every node. The source filters
   * its candidates before the declutter (executor.py:494-497), so a masked-out node is
   * never a candidate and never competes for a box.
   */
  readonly candidates?: Uint8Array | null;
  /** One label per node; an empty string draws none. */
  readonly text: readonly string[];
  /** The screen point of each node, in the space the declutter measures in. */
  readonly screenX: Float64Array;
  readonly screenY: Float64Array;
  /** Stroke width of an edge, in world units; null leaves it to the zoom. */
  readonly edgeWidth?: number | null;
  /** One linear base colour per palette entry, for the impostor spheres; null for flat. */
  readonly spheres?: readonly Rgb[] | null;
}

/** The nodes the labels may be drawn on: the mask's, or all of them. */
function candidatesOf(input: LookStyleInput, count: number): number[] {
  const mask = input.candidates ?? null;
  const nodes: number[] = [];
  for (let node = 0; node < count; node += 1) {
    if (mask === null || mask[node] === 1) nodes.push(node);
  }
  return nodes;
}

/** The text of the kept nodes and "" for the rest, one entry per node. */
function labelsOf(input: LookStyleInput, count: number): string[] {
  const labels = Array.from({ length: count }, () => "");
  const nodes = candidatesOf(input, count);
  const boxes: LabelBox[] = nodes.map((node) => ({
    x: input.screenX[node] ?? 0,
    y: input.screenY[node] ?? 0,
    text: input.text[node] ?? "",
  }));
  const scores = Float64Array.from(nodes, (node) => input.scores[node] ?? Number.NaN);
  for (const at of selectLabels(scores, boxes, LABEL_FONT_PX, LABEL_LIMIT)) {
    const node = nodes[at];
    if (node === undefined) continue;
    const text = input.text[node];
    if (text !== undefined && text !== "") labels[node] = text;
  }
  return labels;
}

/** The palette and the slot per node the painter batches its fills by. */
interface Colours {
  readonly palette: readonly string[];
  readonly colours: Uint16Array;
}

/** The flat node colour of a preset with no metric coloured in (api/render.py:50). */
function flat(input: LookStyleInput, count: number): Colours {
  return { palette: [cssOf(input.look.node)], colours: new Uint16Array(count) };
}

function metric(input: LookStyleInput, count: number): Colours {
  const norm = input.norm ?? new Float64Array(count);
  const { palette, slots } = coloursOf(norm, input.look.colormap);
  return { palette, colours: slots };
}

export function lookStyle(input: LookStyleInput): Style {
  const count = input.text.length;
  const { palette, colours } = input.norm === null ? flat(input, count) : metric(input, count);
  return styleFrom({
    labels: labelsOf(input, count),
    weights: Float32Array.from(input.norm ?? new Float64Array(count)),
    colours,
    palette,
    placement: "centred",
    edgeWidth: input.edgeWidth ?? null,
    spheres: input.spheres ?? null,
  });
}
