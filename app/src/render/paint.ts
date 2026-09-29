/**
 * The draw passes. World coordinates throughout: the caller has already applied
 * `dpr · camera`, so a "radius" here is a world radius and every width is
 * divided by the zoom to stay a constant size on screen.
 *
 * All six geometry kinds are here, which is the reason the studio has its own
 * pass list: `src/core/render` draws nodes from a pre-baked disc sprite and edges
 * as straight segments between two node positions, so it cannot express a `Box`
 * node or a routed `Polyline`/`Curve` edge — see docs/studio.md.
 */

import { edgeStroke } from "../../../src/core/theme/colors.ts";
import type { SceneTheme } from "../../../src/core/theme/tokens.ts";
import { NodeSpriteCache, DISC_FRACTION } from "../../../src/core/render/sprites.ts";
import { styleKey } from "../../../src/core/render/nodeShape.ts";
import type { EdgeKind } from "../../../src/core/types.ts";
import type { DrawList } from "../core/drawList.ts";
import type { Frame } from "../core/frame.ts";
import type { NodeStyle } from "./palette.ts";
import { NEUTRAL_FILL } from "./palette.ts";
import type { Camera, WorldBounds } from "../../../src/core/camera/transform.ts";

/** Node sprites stop being legible below this on-screen radius; a plain disc
 *  reads the same and costs nothing. */
const DOT_MAX_SCREEN_RADIUS = 3.2;
/** Labels are drawn from this zoom up, and only for this many nodes. */
const LABEL_MIN_SCALE = 1.05;
const LABEL_BUDGET = 240;
const EDGE_LOD_MIN_SCALE = 0.25;
const EDGE_WIDTH = 1.1;
const SELECT_RING_WIDTH = 2;

export interface PaintState {
  readonly ctx: CanvasRenderingContext2D;
  readonly theme: SceneTheme;
  readonly camera: Camera;
  readonly view: WorldBounds;
  readonly list: DrawList;
  readonly frame: Frame;
  readonly styles: readonly NodeStyle[];
  readonly edgeKinds: readonly EdgeKind[];
  readonly sprites: NodeSpriteCache;
  readonly hover: number;
  readonly selected: number;
  readonly neighbors: ReadonlySet<number>;
  /** Global alpha for the whole pass (1, or `t` mid-cross-fade). */
  readonly alpha: number;
  /** Backing-store ratio: the camera below is in CSS pixels. */
  readonly dpr: number;
}

function styleAt(styles: readonly NodeStyle[], index: number): NodeStyle | null {
  return styles[index] ?? null;
}

function inView(view: WorldBounds, x: number, y: number, margin: number): boolean {
  return x >= view.minX - margin && x <= view.maxX + margin && y >= view.minY - margin && y <= view.maxY + margin;
}

/** Stroke one edge: its own path when it has one, else the segment between its
 *  two node positions. A `Curve`'s points are already sampled, so degree 1 and 3
 *  differ only in how many points the motor emitted. */
function strokeEdge(state: PaintState, index: number): boolean {
  const { ctx, frame, list } = state;
  const edge = list.edges[index];
  const path = frame.paths[index] ?? null;
  const ax = frame.x[edge.source];
  const ay = frame.y[edge.source];
  if (path !== null && path.length >= 4) {
    ctx.moveTo(path[0], path[1]);
    for (let i = 2; i < path.length; i += 2) ctx.lineTo(path[i], path[i + 1]);
    return true;
  }
  if (path === null && list.edgeKind !== "Line") return false;
  if (!inView(state.view, ax, ay, 64) && !inView(state.view, frame.x[edge.target], frame.y[edge.target], 64)) {
    return false;
  }
  ctx.moveTo(ax, ay);
  ctx.lineTo(frame.x[edge.target], frame.y[edge.target]);
  return true;
}

export function drawEdges(state: PaintState): void {
  const { ctx, camera, theme, edgeKinds, list } = state;
  if (state.alpha <= 0.001) return;
  if (camera.scale < EDGE_LOD_MIN_SCALE && list.edges.length > 4000) return;
  ctx.lineCap = "round";
  ctx.lineWidth = EDGE_WIDTH / camera.scale;
  ctx.globalAlpha = state.alpha;
  for (let i = 0; i < list.edges.length; i += 1) {
    const kind = edgeKinds[i];
    ctx.strokeStyle = edgeStroke(theme, kind ?? "relation");
    ctx.beginPath();
    if (strokeEdge(state, i)) ctx.stroke();
  }
  ctx.globalAlpha = 1;
}

/** A `Box` node, painted rather than blitted: the sprite cache bakes discs.
 *  The centre comes from the FRAME, not from the list, or a Box would sit still
 *  through the very transition every other kind moves through. */
function drawBox(
  ctx: CanvasRenderingContext2D,
  node: DrawList["nodes"][number],
  x: number,
  y: number,
  scale: number,
  fill: string,
  theme: SceneTheme,
): void {
  const halfW = node.w / 2;
  const halfH = node.h / 2;
  const radius = Math.min(6 / scale, halfW, halfH);
  ctx.fillStyle = theme.nodeBacking;
  ctx.beginPath();
  ctx.roundRect(x - halfW - 1, y - halfH - 1, node.w + 2, node.h + 2, radius);
  ctx.fill();
  ctx.fillStyle = fill;
  ctx.beginPath();
  ctx.roundRect(x - halfW, y - halfH, node.w, node.h, radius);
  ctx.fill();
  ctx.strokeStyle = theme.nodeRim;
  ctx.lineWidth = 1 / scale;
  ctx.stroke();
}

/** Every node: a blitted sprite for `Point`/`Circle`, a painted rect for `Box`,
 *  and a batched plain disc for the far-zoom case where no sprite is legible. */
export function drawNodes(state: PaintState): void {
  const { ctx, camera, frame, list, sprites, theme } = state;
  if (state.alpha <= 0.001) return;
  const dimmed = state.selected >= 0;
  for (const node of list.nodes) {
    const x = frame.x[node.index];
    const y = frame.y[node.index];
    if (!inView(state.view, x, y, 32)) continue;
    const style = styleAt(state.styles, node.index);
    const faded = state.alpha * alphaFor(state, node.index, dimmed);
    if (faded <= 0.01) continue;
    const radius = node.r > 0 ? node.r : 4.5;
    ctx.globalAlpha = faded;
    if (list.nodeKind === "Box") {
      drawBox(ctx, node, x, y, camera.scale, style?.fill ?? NEUTRAL_FILL, theme);
      continue;
    }
    if (radius * camera.scale <= DOT_MAX_SCREEN_RADIUS) {
      ctx.fillStyle = style?.fill ?? NEUTRAL_FILL;
      ctx.beginPath();
      ctx.arc(x, y, radius, 0, Math.PI * 2);
      ctx.fill();
      continue;
    }
    const sprite = sprites.get(styleKey(style?.shape ?? "disc", style?.fill ?? NEUTRAL_FILL));
    const half = (radius / DISC_FRACTION) * sprite.width * 0.5;
    ctx.drawImage(sprite, x - half, y - half, half * 2, half * 2);
  }
  ctx.globalAlpha = 1;
}

/** 1 for everything in view, dimmed for everything outside the selection's
 *  neighbourhood — the click-to-highlight read. */
function alphaFor(state: PaintState, index: number, dimmed: boolean): number {
  if (!dimmed) return 1;
  if (index === state.selected || state.neighbors.has(index)) return 1;
  return 0.18;
}

/** The hover and selection rings, drawn last so they sit over the nodes. */
export function drawRings(state: PaintState): void {
  const { ctx, camera, frame, list, theme } = state;
  const ring = (index: number, colour: string, width: number): void => {
    const node = list.nodes[index];
    if (node === undefined) return;
    const radius = (node.r > 0 ? node.r : 4.5) + 4 / camera.scale;
    ctx.strokeStyle = colour;
    ctx.lineWidth = width / camera.scale;
    ctx.beginPath();
    ctx.arc(frame.x[index], frame.y[index], radius, 0, Math.PI * 2);
    ctx.stroke();
  };
  if (state.selected >= 0) ring(state.selected, theme.selectRing, SELECT_RING_WIDTH);
  if (state.hover >= 0 && state.hover !== state.selected) ring(state.hover, theme.hoverRing, SELECT_RING_WIDTH);
}

/** Labels for the nearest-to-centre nodes, once the zoom makes them legible. */
export function drawLabels(state: PaintState): void {
  const { ctx, camera, frame, list, theme } = state;
  if (camera.scale < LABEL_MIN_SCALE) return;
  ctx.font = "11px ui-sans-serif, system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  let drawn = 0;
  for (const node of list.nodes) {
    if (drawn >= LABEL_BUDGET) return;
    const style = styleAt(state.styles, node.index);
    if (style === null || style.label === "") continue;
    const x = frame.x[node.index];
    const y = frame.y[node.index];
    if (!inView(state.view, x, y, 16)) continue;
    const lift = (node.h > 0 ? node.h / 2 : node.r) + 4 / camera.scale;
    ctx.globalAlpha = state.alpha * alphaFor(state, node.index, state.selected >= 0);
    ctx.lineWidth = 3 / camera.scale;
    ctx.strokeStyle = theme.labelHalo;
    ctx.strokeText(style.label, x, y + lift);
    ctx.fillStyle = theme.label;
    ctx.fillText(style.label, x, y + lift);
    drawn += 1;
  }
  ctx.globalAlpha = 1;
}

/** One frame: edges under, nodes over, rings and labels on top. The context is
 *  left in CSS pixels, then moved into world coordinates, so every width above
 *  is a screen width divided by the zoom. */
export function paintFrame(state: PaintState): void {
  const { ctx, camera, dpr } = state;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.translate(camera.x, camera.y);
  ctx.scale(camera.scale, camera.scale);
  drawEdges(state);
  drawNodes(state);
  drawRings(state);
  drawLabels(state);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}
