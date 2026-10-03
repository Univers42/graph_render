/**
 * ST-4: the box the studio's panels leave visible, and the observer that keeps it measured.
 *
 * The camera fits the canvas, which is the whole window, while the dock, the left column,
 * the console and the legend float over it. A fit that is right for the canvas puts part of
 * the drawing where the user cannot see through a panel, so the studio hands the view a safe
 * area and the view fits into that (`graph-render/src/camera.ts` `FitArea`).
 *
 * Two halves, on purpose. `safeAreaOf` is pure — a canvas box and the measured panel boxes in,
 * one `FitArea` out — and that is where the logic lives, so a unit test can drive it with no
 * DOM. `watchSafeArea` is the thin half: it measures the panels and calls back when the box
 * changes, which is the only part that needs a browser.
 */
import { type FitArea, type Viewport } from "../../../graph-render/src/camera.ts";

export type { FitArea };

/**
 * The panels the stylesheet floats over the canvas, in the order they are read
 * (`src/styles/studio.css.ts`): the left column of Search + Inspector, the Controls dock on
 * the right, the console along the bottom, and the legend + camera bar in the bottom left.
 * A selector that matches nothing is not a panel — that is how the console, which the shell
 * renders only while it is open, is found or not found.
 */
export const PANEL_SELECTORS = [".gs-left", ".gs-dock", ".gs-console", ".gs-bottom-left"] as const;

/** The x or y edges the canvas and these rects put in range, ascending, each one once. */
function cuts(panels: readonly FitArea[], axis: "x" | "y", size: number): readonly number[] {
  const edges = [0, size];
  for (const panel of panels) {
    const near = panel[axis], far = near + (axis === "x" ? panel.width : panel.height);
    if (near > 0 && near < size) edges.push(near);
    if (far > 0 && far < size) edges.push(far);
  }
  return [...new Set(edges)].sort((a, b) => a - b);
}

/** Every ordered pair of distinct cuts, low before high: the left and right of a free box. */
function spans(values: readonly number[]): readonly (readonly [number, number])[] {
  return values.flatMap((from, at) => values.slice(at + 1).map((to) => [from, to] as const));
}

/** True when the two boxes share any area at all; boxes that only touch do not overlap. */
function overlaps(a: FitArea, b: FitArea): boolean {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

/**
 * The biggest box in the canvas no panel has any of, or null when the panels leave none.
 *
 * The cuts are the panel edges, so the largest free box always has its sides on one: any
 * other box can be grown to a cut without meeting a panel. Ascending cuts and a strict `>`
 * keep the first of equal-area boxes, so the answer is the same every run (D2).
 */
function freest(canvas: Viewport, panels: readonly FitArea[]): FitArea | null {
  const across = spans(cuts(panels, "x", canvas.width));
  const down = spans(cuts(panels, "y", canvas.height));
  let best: FitArea | null = null;
  for (const [x, right] of across) {
    for (const [y, bottom] of down) {
      const box = { x, y, width: right - x, height: bottom - y };
      if (best !== null && box.width * box.height <= best.width * best.height) continue;
      if (!panels.some((panel) => overlaps(box, panel))) best = box;
    }
  }
  return best;
}

/**
 * The part of the canvas a fit may use, or `null` when the panels leave no room for one.
 *
 * No panel — a studio whose chrome is closed, or a panel that has not been laid out yet —
 * gives the whole canvas, which is the fit the studio had before any of this, pinned by the
 * first test in `tests/safe-area.test.ts`.
 *
 * Ponytail: the box is rounded to whole CSS pixels, so a panel edge a half pixel inside the
 * canvas is rounded away from it. Failing input: a panel edge at x = 507.6 on a canvas 800
 * wide leaves 292 px of drawing room and the fit uses 293. Direction it errs: outward, by
 * under a pixel. Escape hatch: nothing needs it — `FitOptions.padding` (64 px by default) is
 * the slack, and it is an order of magnitude larger than the rounding.
 *
 * Ponytail: panels that leave no free box at all give `null`, which the view reads as the
 * whole canvas — so a window narrower than the panels puts the drawing back under them.
 * Failing input: a 300px-wide window with a 280px dock and a 260px column. Direction it
 * errs: towards showing everything rather than towards a broken camera. Escape hatch: the
 * dock's own toggle in the Controls panel.
 */
export function safeAreaOf(canvas: Viewport, panels: readonly FitArea[]): FitArea | null {
  const whole = { x: 0, y: 0, width: canvas.width, height: canvas.height };
  const over = panels.filter((panel) => panel.width > 0 && panel.height > 0 && overlaps(panel, whole));
  if (over.length === 0) return whole;
  const free = freest(canvas, over);
  if (free === null) return null;
  const area = {
    x: Math.round(free.x), y: Math.round(free.y),
    width: Math.max(1, Math.round(free.width)), height: Math.max(1, Math.round(free.height)),
  };
  return area.width * area.height > 0 ? area : null;
}

/** The panels' boxes in canvas pixels: the canvas's own top left is the origin. */
function boxesIn(chrome: Element, at: DOMRect): readonly FitArea[] {
  const found: FitArea[] = [];
  for (const selector of PANEL_SELECTORS) {
    for (const panel of chrome.querySelectorAll(selector)) {
      const box = panel.getBoundingClientRect();
      if (box.width <= 0 || box.height <= 0) continue;
      found.push({ x: box.left - at.left, y: box.top - at.top, width: box.width, height: box.height });
    }
  }
  return found;
}

/**
 * Measures the panels and hands `apply` the box they leave visible, now and whenever it
 * changes: a window resize, the dock growing, the console opening and closing. A resize
 * observer on the chrome catches the first two — the chrome is `inset: 0`, so it moves with
 * the window — and a mutation observer catches the third, since the console leaves the tree
 * and enters it again. Returns what stops both, which `unmount` must call or they outlive
 * the studio.
 */
export function watchSafeArea(
  canvas: HTMLCanvasElement,
  chrome: HTMLElement,
  apply: (area: FitArea | null) => void,
): () => void {
  let last: string | null = null;
  const measure = (): void => {
    const at = canvas.getBoundingClientRect();
    // A canvas that is not laid out has no box to be a fraction of; the first measure after
    // mount is one, and everything after it too.
    if (at.width <= 0 || at.height <= 0) return;
    const area = safeAreaOf({ width: at.width, height: at.height }, boxesIn(chrome, at));
    const next = JSON.stringify(area);
    if (next === last) return;
    last = next;
    apply(area);
  };
  const watch = (): void => {
    for (const selector of PANEL_SELECTORS) for (const panel of chrome.querySelectorAll(selector)) sizes.observe(panel);
    sizes.observe(canvas);
    sizes.observe(chrome);
    measure();
  };
  const sizes = new ResizeObserver(measure);
  const changes = new MutationObserver(watch);
  changes.observe(chrome, { childList: true, subtree: true });
  watch();
  return () => { sizes.disconnect(); changes.disconnect(); };
}
