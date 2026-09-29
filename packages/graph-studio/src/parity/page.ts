/**
 * The parity page: the fixture drawn on a canvas, with nothing else on it, and its own
 * state on `window` so the gate can read what the painter drew rather than what it was
 * asked to draw. The camera is the identity and the frame carries no bounds, so the
 * fixture's own pixel coordinates are the pixels of the fig1 frame
 * (docs/decisions/scigraphs-reference-fixture.md).
 */
import { createView, type View } from "../../../graph-render/src/view.ts";
import { DEFAULT_PRESET, LABEL_FONT_PX, lookOf } from "../../../graph-render/src/look/presets.ts";
import { type Fixture, FIXTURE_PATH, readFixture } from "./fixture.ts";
import { type ParityScene, parityScene } from "./scene.ts";

declare global {
  interface Window {
    __parity?: ParityState;
    /** Set instead of `__parity` when the fixture could not be read or drawn. */
    __parityError?: string;
  }
}

/** How long the page waits for the first painted frame before reporting a count of -1. */
const PAINT_MS = 2000;

/** One node as the page reports it: where it is, how big, and what it is filled with. */
export interface ParityNode {
  readonly id: number;
  readonly x: number;
  readonly y: number;
  readonly r: number;
  /** The palette entry the node is filled with, as the painter spells it. */
  readonly fill: string;
  readonly label: string;
  readonly betweenness: number;
  /** True when this node's centre is under its own label's box, so it cannot be sampled. */
  readonly covered: boolean;
}

/** Where a label's box lands on the screenshot, as the sprite bake puts it. */
export interface LabelRect {
  readonly id: number;
  readonly label: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface ParityState {
  readonly look: string;
  readonly width: number;
  readonly height: number;
  /** The theme's background, as the painter spells it. */
  readonly background: string;
  /** The names the page draws, in node order. */
  readonly labels: readonly string[];
  readonly labelNodes: readonly number[];
  /** How many labels the first painted frame drew, or -1 when none arrived. */
  readonly drawn: number;
  /** The box of every drawn label, so a covered node centre can be told from a wrong fill. */
  readonly labelRects: readonly LabelRect[];
  readonly nodes: readonly ParityNode[];
}

/**
 * Half the width a name is estimated at when nothing has measured it: 0.30 of the font
 * per character, which is what the source's own declutter assumes
 * (text_overlay.py:309-313, labels2d/declutter.ts:38-42).
 *
 * Ponytail: this is the fallback for a host with no 2D context, and on DejaVu Sans at 26 px
 * it is narrow for wide glyphs (MmePontmercy is 204 px of text against 187 estimated), so
 * a reported box can be a few pixels short of the one that was baked. The escape hatch is
 * to pass a measurer, which is what the browser page does.
 */
function estimatedWidth(text: string): number {
  return 0.3 * LABEL_FONT_PX * Math.max(Array.from(text).length, 1);
}

function labelRects(scene: ParityScene, given: Fixture, measure: (text: string) => number): LabelRect[] {
  const { camera, frame, theme } = scene;
  const pad = theme.labelBox?.padding ?? 0;
  return scene.labelled.map((node) => {
    const label = given.nodes[node]?.label ?? "";
    const width = Math.ceil(measure(label)) + pad * 2;
    const x = (frame.x[node] ?? 0) * camera.scale + camera.x;
    const y = (frame.y[node] ?? 0) * camera.scale + camera.y;
    return { id: node, label, x: x - width / 2, y: y - theme.labelHeight / 2, width, height: theme.labelHeight };
  });
}

function nodesOf(scene: ParityScene, given: Fixture): ParityNode[] {
  const { frame, style } = scene;
  return given.nodes.map((node) => ({
    id: node.id,
    x: frame.x[node.id] ?? 0,
    y: frame.y[node.id] ?? 0,
    r: frame.r?.[node.id] ?? 0,
    fill: style.palette[style.colours[node.id] ?? 0] ?? "",
    label: node.label,
    betweenness: node.betweenness,
    covered: (style.labels[node.id] ?? "") !== "",
  }));
}

export function parityStateOf(
  scene: ParityScene,
  given: Fixture,
  drawn: number,
  measure: (text: string) => number = estimatedWidth,
): ParityState {
  const nodes = nodesOf(scene, given);
  return {
    look: DEFAULT_PRESET,
    width: scene.params.resolution[0],
    height: scene.params.resolution[1],
    background: scene.theme.background,
    labels: scene.labelled.map((node) => nodes[node]?.label ?? ""),
    labelNodes: scene.labelled,
    drawn,
    labelRects: labelRects(scene, given, measure),
    nodes,
  };
}

/** The width of a name in the page's own font, which is what the sprite bake measures. */
function measurer(canvas: HTMLCanvasElement, font: string): (text: string) => number {
  const ctx = canvas.getContext("2d");
  if (ctx === null) return estimatedWidth;
  ctx.font = font;
  return (text: string): number => ctx.measureText(text).width;
}

/** The first frame's label count, or -1 when the loop never paints one. */
function firstFrame(view: View, ms: number): Promise<number> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      off();
      resolve(-1);
    }, ms);
    const off = view.on("frame", (stats) => {
      clearTimeout(timer);
      off();
      resolve(stats.drawnLabels);
    });
  });
}

export async function mountParity(canvas: HTMLCanvasElement, base: string): Promise<ParityState> {
  const response = await fetch(`${base}${FIXTURE_PATH}`);
  if (!response.ok) throw new Error(`parity page: ${FIXTURE_PATH} is ${response.status}, not 200`);
  const given = readFixture(await response.text());
  const scene = parityScene(given, lookOf(DEFAULT_PRESET));
  const view = createView(canvas, { theme: scene.theme, labels: scene.policy });
  // setFrame, setStyle and setCamera each ask for a frame; the first one paints all three.
  view.setFrame(scene.frame, { fit: false });
  view.setStyle(scene.style);
  view.setCamera(scene.camera);
  return parityStateOf(scene, given, await firstFrame(view, PAINT_MS), measurer(canvas, scene.theme.labelFont));
}
