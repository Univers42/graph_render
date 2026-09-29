/** What the user is doing on top of the drawing: the box being dragged, and the extra selected nodes. */
import type { LoopState } from "./loop.ts";
import { MIN_SCREEN_RADIUS } from "./nodes.ts";

const RING_GAP = 3;
const BOX_FILL_ALPHA = 0.12;

function paintRings(state: LoopState): void {
  const { ctx, camera, theme } = state;
  ctx.strokeStyle = theme.ring;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  for (const node of state.selection) {
    if (node === state.selected) continue;
    const radius = Math.max(MIN_SCREEN_RADIUS, (state.scene.extent[node] ?? 0) * camera.scale) + RING_GAP;
    const x = (state.x[node] ?? 0) * camera.scale + camera.x;
    const y = (state.y[node] ?? 0) * camera.scale + camera.y;
    ctx.moveTo(x + radius, y);
    ctx.arc(x, y, radius, 0, Math.PI * 2);
  }
  ctx.stroke();
}

/** A pinned node is a dashed ring, so it reads apart from the solid selection ring. */
function paintPins(state: LoopState): void {
  if (state.pinned.length === 0) return;
  const { ctx, camera, theme } = state;
  ctx.strokeStyle = theme.ring;
  ctx.lineWidth = 1.5;
  ctx.setLineDash([3, 3]);
  ctx.beginPath();
  for (const node of state.pinned) {
    const radius = Math.max(MIN_SCREEN_RADIUS, (state.scene.extent[node] ?? 0) * camera.scale) + RING_GAP * 2;
    const x = (state.x[node] ?? 0) * camera.scale + camera.x;
    const y = (state.y[node] ?? 0) * camera.scale + camera.y;
    ctx.moveTo(x + radius, y);
    ctx.arc(x, y, radius, 0, Math.PI * 2);
  }
  ctx.stroke();
  ctx.setLineDash([]);
}

function paintMarquee(state: LoopState): void {
  const { ctx, marquee, theme } = state;
  if (marquee === null) return;
  const width = marquee.maxX - marquee.minX;
  const height = marquee.maxY - marquee.minY;
  ctx.fillStyle = theme.ring;
  ctx.globalAlpha = BOX_FILL_ALPHA;
  ctx.fillRect(marquee.minX, marquee.minY, width, height);
  ctx.globalAlpha = 1;
  ctx.strokeStyle = theme.ring;
  ctx.lineWidth = 1;
  ctx.strokeRect(marquee.minX + 0.5, marquee.minY + 0.5, width, height);
}

export function paintOverlay(state: LoopState): void {
  state.ctx.globalAlpha = 1;
  paintRings(state);
  paintPins(state);
  paintMarquee(state);
}
