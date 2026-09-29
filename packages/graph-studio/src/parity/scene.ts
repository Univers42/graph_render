/**
 * The parity scene: the fixture's own pixels as a frame, the SciGraphs preset as a theme
 * and a style, and the camera that puts world on screen without a transform. No layout
 * runs here: the positions are SciGraphs' own (docs/decisions/scigraphs-reference-fixture.md).
 *   SciGraphs/core/visualization/text_overlay.py:182-187 (the sensor projection)
 *   SciGraphs/core/repro/executor.py:285-307 (the *_radius_rel multipliers)
 *   SciGraphs/core/mesh/edge_styles.py:108-188 (the AUTO bezier bend)
 */
import { IDENTITY, type Camera } from "../../../graph-render/src/camera.ts";
import { type LabelPolicy } from "../../../graph-render/src/labels.ts";
import type { Frame } from "../../../graph-render/src/frame.ts";
import type { Look } from "../../../graph-render/src/look/presets.ts";
import { lookStyle } from "../../../graph-render/src/look/style.ts";
import { lookTheme } from "../../../graph-render/src/look/theme.ts";
import type { Style } from "../../../graph-render/src/style.ts";
import type { Theme } from "../../../graph-render/src/theme.ts";
import { controlPoint } from "../../../graph-render/src/edges2d/curve.ts";
import type { Fixture, FixtureParams } from "./fixture.ts";
import { candidatesOf } from "./occlusion.ts";

/** The curvature CYTOSCAPE_BEZIER carries (edge_styles.py:21-28, 162-188). */
const CURVATURE = 0.5;

export interface ParityScene {
  readonly frame: Frame;
  readonly style: Style;
  readonly theme: Theme;
  readonly camera: Camera;
  readonly policy: LabelPolicy;
  /** The node indices the reference pipeline labelled, in the fixture's own order. */
  readonly labelled: readonly number[];
  /** The nodes whose betweenness is 0, and so carry one shared fill. */
  readonly zero: readonly number[];
  /** That shared fill, as the palette spells it. */
  readonly zeroFill: string;
  readonly params: FixtureParams;
}

/**
 * Pixels per world unit at `depth`: lens * res_x / sensor, the scale of the sensor-plane
 * projection (text_overlay.py:182-187), with the half-sensor of the source folded in.
 */
function pixelScale(params: FixtureParams, depth: number): number {
  return (params.camera_lens_mm * params.resolution[0]) / (params.camera_sensor_mm * depth);
}

/** The node's glyph radius in pixels: node_radius_rel * R, through the same projection. */
function radiusOf(given: Fixture, node: number): number {
  return given.radii.node * pixelScale(given.params, given.nodes[node]?.depth ?? 1);
}

/**
 * The AUTO control point of one edge, in the same pixel space as the frame.
 *
 * The bend is computed in SciGraphs' y-up world (text_overlay.py:231-233 flips y only on
 * the way into pixels) and the formula is not invariant under that flip, so the two ends
 * go in negated and the control point comes back negated.
 */
function control(given: Fixture, from: number, to: number): readonly [number, number] {
  const a = given.nodes[from]?.screen ?? [0, 0];
  const b = given.nodes[to]?.screen ?? [0, 0];
  const bent = controlPoint({ x: a[0], y: -a[1] }, { x: b[0], y: -b[1] }, CURVATURE);
  return bent === null ? [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2] : [bent.x, -bent.y];
}

function edgesOf(given: Fixture): Pick<Frame, "source" | "target" | "curveDegree" | "offsets" | "pts" | "edgeKind"> {
  const count = given.edges.length;
  const offsets = new Uint32Array(count + 1);
  const pts = new Float32Array(count * 2);
  given.edges.forEach(([from, to], at) => {
    offsets[at] = at;
    const [cx, cy] = control(given, from, to);
    pts[2 * at] = cx;
    pts[2 * at + 1] = cy;
  });
  offsets[count] = count;
  return {
    source: Uint32Array.from(given.edges, (edge) => edge[0]),
    target: Uint32Array.from(given.edges, (edge) => edge[1]),
    edgeKind: "Curve", curveDegree: 2, offsets, pts,
  };
}

function nodesOf(given: Fixture): Pick<Frame, "x" | "y" | "r" | "nodeCount" | "nodeKind"> {
  return {
    nodeKind: "Circle", nodeCount: given.nodes.length,
    x: Float32Array.from(given.nodes, (node) => node.screen[0]),
    y: Float32Array.from(given.nodes, (node) => node.screen[1]),
    r: Float32Array.from(given.nodes, (_, node) => radiusOf(given, node)),
  };
}

/**
 * One edge width for the whole batch, taken at the camera's own distance: the fit put the
 * graph at that distance, and a node nearer the camera draws wider by the ratio of the
 * depths.
 *
 * Ponytail: the source's tube has one world radius, so its pixel width is 2 * edge_radius
 * * scale(depth) and it varies across the frame by the ratio of the depths, which here is
 * 20.6/10.2 = 2.0; the batched painter has one stroke width for all 254 edges, so a node at
 * depth 10.2 is drawn 1.3x too narrow and one at 20.6 the same too wide. The escape hatch
 * is to lay the edges out in depth bands, one batch per band.
 */
function edgeWidthOf(given: Fixture): number {
  return 2 * given.radii.edge * pixelScale(given.params, given.params.camera_distance);
}

export function parityScene(given: Fixture, look: Look): ParityScene {
  const nodes = nodesOf(given);
  const style = lookStyle({
    look,
    norm: Float64Array.from(given.nodes, (node) => node.t),
    scores: Float64Array.from(given.nodes, (node) => node.betweenness),
    // The occlusion stage runs before the declutter, as executor.py:494-507 has it.
    candidates: candidatesOf(given),
    text: given.nodes.map((node) => node.label),
    screenX: Float64Array.from(given.nodes, (node) => node.screen[0]),
    screenY: Float64Array.from(given.nodes, (node) => node.screen[1]),
    edgeWidth: edgeWidthOf(given),
    // Flat fills, not impostor spheres: the gate measures a node's *fill* (the spec's own
    // word), and the fill the figure's nodes carry is the colormap's entry for their t
    // (05-reproducible-pipeline.qmd:120-124). The impostor pass is the interactive studio's
    // 3D look (sprite/impostor.ts), and a lit sphere's centre is the brightest pixel of
    // the node rather than its fill.
    spheres: null,
  });
  const frame: Frame = {
    ...nodes, ...edgesOf(given),
    edgeCount: given.edges.length,
    w: null, h: null, bounds: null, factor: 1,
  };
  const labelled = given.nodes.flatMap((node) => (style.labels[node.id] === "" ? [] : [node.id]));
  const zero = given.nodes.flatMap((node) => (node.betweenness === 0 ? [node.id] : []));
  const first = zero.at(0) ?? 0;
  return {
    frame, style, theme: lookTheme(look), camera: IDENTITY,
    policy: { threshold: 1.1, budget: given.params.label_max_count },
    labelled, zero,
    zeroFill: style.palette[style.colours[first] ?? 0] ?? "",
    params: given.params,
  };
}
