/**
 * The SciGraphs look presets, the gallery look, and the size constants the studio
 * painter reads. Values are linear-light; the painter encodes once with cssOf.
 *   SciGraphs/api/render.py:45-101 (PRESETS)
 */
import type { Rgb } from "../colour/srgb.ts";
import type { ColormapName } from "../colour/tables.ts";

export interface Look {
  readonly name: string;
  readonly background: Rgb;
  readonly colormap: ColormapName;
  /** The flat node colour, used when no metric is coloured in. */
  readonly node: Rgb;
  readonly edge: Rgb;
}

function rgb(v: readonly [number, number, number]): Rgb {
  return [v[0], v[1], v[2]];
}

function look(name: string, v: {
  background: readonly [number, number, number];
  colormap: ColormapName;
  node: readonly [number, number, number];
  edge: readonly [number, number, number];
}): Look {
  return { name, background: rgb(v.background), colormap: v.colormap, node: rgb(v.node), edge: rgb(v.edge) };
}

/**
 * The six SciGraphs presets, field for field out of PRESETS: `background`, `colormap`,
 * `color` and `edge_color` (api/render.py:45-101). Keys, fill and lamp values are the
 * EEVEE render path's business and have no place in a 2D painter.
 */
const SCIGRAPHS: readonly Look[] = [
  look("slate", { background: [0.055, 0.06, 0.075], colormap: "viridis", node: [0.55, 0.6, 0.68], edge: [0.16, 0.17, 0.21] }),
  look("paper", { background: [0.93, 0.93, 0.91], colormap: "plasma", node: [0.36, 0.39, 0.46], edge: [0.22, 0.23, 0.26] }),
  look("ink", { background: [0.015, 0.015, 0.02], colormap: "turbo", node: [0.5, 0.52, 0.58], edge: [0.13, 0.12, 0.15] }),
  look("blueprint", { background: [0.035, 0.065, 0.115], colormap: "cividis", node: [0.45, 0.56, 0.7], edge: [0.14, 0.2, 0.3] }),
  look("terrain", { background: [0.27, 0.25, 0.22], colormap: "inferno", node: [0.62, 0.58, 0.52], edge: [0.22, 0.2, 0.18] }),
  look("relief", { background: [0.27, 0.25, 0.22], colormap: "inferno", node: [0.8, 0.76, 0.7], edge: [0.26, 0.24, 0.21] }),
];

/**
 * The gallery look, the fig1/fig6 pipeline of the worked example:
 *   docs/examples/05-reproducible-pipeline.qmd:121 (colormap inferno)
 *   docs/examples/05-reproducible-pipeline.qmd:132-137 (edge_base_color 0.16, 0.16, 0.19)
 *   docs/examples/05-reproducible-pipeline.qmd:139-145 (world colour 0.035, 0.035, 0.045)
 * The example colours the nodes by a metric, so it sets no flat node colour and this
 * look borrows slate's for the case where no metric is coloured in.
 */
const GALLERY = look("gallery", {
  background: [0.035, 0.035, 0.045],
  colormap: "inferno",
  node: [0.55, 0.6, 0.68],
  edge: [0.16, 0.16, 0.19],
});

export const LOOKS: Readonly<Record<string, Look>> = Object.fromEntries(
  [...SCIGRAPHS, GALLERY].map((entry) => [entry.name, entry]),
);

export const LOOK_NAMES: readonly string[] = Object.keys(LOOKS);

/**
 * Node radius as a fraction of the world radius, and the 2D stroke width is the edge
 * radius as a diameter (docs/examples/05-reproducible-pipeline.qmd:126-127).
 */
export const NODE_RADIUS_REL = 0.022;

export const EDGE_WIDTH_REL = 0.007;

/** Fit the world box into the viewport with this much slack (05-reproducible-pipeline.qmd:164). */
export const FIT_MARGIN = 1.12;

/** At most this many labels are drawn (05-reproducible-pipeline.qmd:673). */
export const LABEL_LIMIT = 18;

/** Label font size in CSS px (05-reproducible-pipeline.qmd:675). */
export const LABEL_FONT_PX = 26;

/** Padding around the label text (SciGraphs/core/visualization/text_overlay.py:547). */
export const LABEL_PADDING_PX = 3;

/** The label backdrop, halo alpha 0.6 over a black halo colour (05-reproducible-pipeline.qmd:679). */
export const LABEL_BOX = "rgba(0,0,0,0.6)";

/** The world bounding box the radius constants multiply. */
export interface Bounds {
  readonly minX: number;
  readonly minY: number;
  readonly maxX: number;
  readonly maxY: number;
}

/** The source floors the radius so a single-node graph does not divide by zero (executor.py:650). */
const MIN_RADIUS = 1e-6;

/**
 * Half the diagonal of the box, the radius every relative size is a fraction of. The
 * source computes 0.5 * sqrt(sum of squared extents) over three axes and floors it
 * (executor.py:632,650); a 2D painter has no z extent, so this is the two-axis form of
 * the same expression. The gallery layout is SPRING_3D (05-reproducible-pipeline.qmd:614),
 * so a projected box of an isotropic cloud under-reports the source's radius by about 18%.
 */
export function worldRadius(bounds: Bounds): number {
  const diagonal = Math.hypot(bounds.maxX - bounds.minX, bounds.maxY - bounds.minY) / 2;
  return Math.max(diagonal, MIN_RADIUS);
}

/** The look of a given name, falling back to slate the way api/render.py:106-109 does. */
export function lookOf(name: string): Look {
  const key = name.toLowerCase();
  const found = Object.hasOwn(LOOKS, key) ? LOOKS[key] : undefined;
  const fallback = LOOKS.slate;
  if (!found) return fallback ?? GALLERY;
  return found;
}
