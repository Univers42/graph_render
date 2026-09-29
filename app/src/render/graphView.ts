/**
 * One graph panel: the aurora field, the graph canvas above it, the camera, the
 * pointer, and the 600 ms transition between two runs.
 *
 * This is the studio's own view rather than the engine's `CanvasScene`, and the
 * reason is geometry, not taste: `src/core/render` draws every node from a baked
 * DISC sprite and every edge as a straight segment between two node positions, so
 * a `Box` node or a routed `Polyline`/`Curve` edge cannot be expressed through it
 * — and the studio has to render all six kinds (docs/studio.md). What IS reused
 * is the look and the maths underneath: the `AuroraBackground` field, the
 * resolved `--osio-*` theme, the `NodeSpriteCache` that bakes the aurora-glass
 * node, and the engine's own `zoomAt`/`panBy`/`visibleWorldRect` camera.
 */

import { AuroraBackground } from "../../../src/core/render/background.ts";
import { NodeSpriteCache } from "../../../src/core/render/sprites.ts";
import { resolveSceneTheme, type SceneTheme } from "../../../src/core/theme/tokens.ts";
import { visibleWorldRect } from "../../../src/core/camera/controls.ts";
import { IDENTITY, type Camera } from "../../../src/core/camera/transform.ts";
import type { DrawList } from "../core/drawList.ts";
import { frameFor, type Frame } from "../core/frame.ts";
import { type Point, hitTestNode, neighborsOf } from "../core/hitTest.ts";
import { TRANSITION_MS, easeInOutCubic, transitionProgress } from "../core/transition.ts";
import { boundsOf, fitCamera } from "../core/fit.ts";
import { toStudioWorld } from "../core/worldScale.ts";
import { paintFrame, type PaintState } from "./paint.ts";
import { bindPointer, HIT_TOLERANCE_PX, type PointerHost } from "./pointer.ts";
import type { EdgeKind } from "../../../src/core/types.ts";
import type { NodeStyle } from "./palette.ts";

const FIT_PADDING = 48;

export interface PanelCallbacks {
  /** Dense index under the cursor, or -1. */
  readonly onHover: (index: number, at: Point | null) => void;
  /** A node was clicked (or the background, for -1). */
  readonly onSelect: (index: number) => void;
}

/** What the React layer hands over when a run finishes. */
export interface PanelData {
  readonly list: DrawList;
  readonly styles: readonly NodeStyle[];
  readonly edgeKinds: readonly EdgeKind[];
  /** Animate from the previous run's geometry? False for a first run. */
  readonly animate: boolean;
}

export class GraphView implements PointerHost {
  readonly #bgCanvas: HTMLCanvasElement;
  readonly #canvas: HTMLCanvasElement;
  readonly #background: AuroraBackground;
  readonly #sprites = new NodeSpriteCache();
  readonly #callbacks: PanelCallbacks;
  readonly #detachPointer: () => void;
  #theme: SceneTheme;
  #observer: ResizeObserver | null = null;

  #camera: Camera = { ...IDENTITY };
  #dpr = 1;
  #width = 640;
  #height = 480;

  #list: DrawList | null = null;
  #styles: readonly NodeStyle[] = [];
  #edgeKinds: readonly EdgeKind[] = [];
  #from: DrawList | null = null;
  #transitionStart = 0;
  #transitioning = false;

  #hover = -1;
  #selected = -1;
  #neighbors: ReadonlySet<number> = new Set<number>();
  #rafId: number | null = null;
  #destroyed = false;
  #reducedMotion = false;

  constructor(host: HTMLElement, callbacks: PanelCallbacks) {
    this.#callbacks = callbacks;
    this.#bgCanvas = document.createElement("canvas");
    this.#bgCanvas.className = "panel__bg";
    this.#canvas = document.createElement("canvas");
    this.#canvas.className = "panel__fg";
    host.append(this.#bgCanvas, this.#canvas);
    this.#theme = resolveSceneTheme(document.documentElement);
    this.#background = new AuroraBackground(this.#bgCanvas, this.#theme, this.#reducedMotion);
    this.#background.start();
    this.#sprites.setTheme({
      backing: this.#theme.nodeBacking,
      mode: this.#theme.mode,
      rim: this.#theme.nodeRim,
      shadow: this.#theme.nodeShadow,
    });
    this.#observer = new ResizeObserver(() => this.#resize());
    this.#observer.observe(host);
    this.#resize();
    this.#detachPointer = bindPointer(this);
  }

  setReducedMotion(reduced: boolean): void {
    this.#reducedMotion = reduced;
  }

  /** Whether a run is already on screen — i.e. whether the next one has a
   *  previous geometry to transition from. */
  hasData(): boolean {
    return this.#list !== null;
  }

  /** How many nodes the run on screen has. A transition only means anything
   *  between two runs of the SAME graph, and this is how the caller checks. */
  nodeCount(): number {
    return this.#list === null ? 0 : this.#list.nodes.length;
  }

  /** A finished run. The previous geometry becomes the transition's starting
   *  point, so a layout change reads as one graph moving. */
  setData(data: PanelData): void {
    this.#from = data.animate ? this.#list : null;
    this.#list = toStudioWorld(data.list);
    this.#styles = data.styles;
    this.#edgeKinds = data.edgeKinds;
    this.#transitioning = data.animate && !this.#reducedMotion;
    this.#transitionStart = performance.now();
    this.#selected = -1;
    this.#neighbors = new Set<number>();
    this.fit();
    this.#request();
  }

  /** Frame the whole run, whatever kind it is. */
  fit(): void {
    const bounds = this.#list === null ? null : boundsOf(this.#list.nodes);
    this.#camera = fitCamera(bounds, this.#width, this.#height, FIT_PADDING);
    this.#request();
  }

  /** The aurora field and the graph, composited — what "Export PNG" saves. */
  exportPng(): string {
    const out = document.createElement("canvas");
    out.width = this.#canvas.width;
    out.height = this.#canvas.height;
    const ctx = out.getContext("2d");
    if (ctx === null) return this.#canvas.toDataURL("image/png");
    ctx.drawImage(this.#bgCanvas, 0, 0);
    ctx.drawImage(this.#canvas, 0, 0);
    return out.toDataURL("image/png");
  }

  destroy(): void {
    this.#destroyed = true;
    if (this.#rafId !== null) cancelAnimationFrame(this.#rafId);
    this.#background.destroy();
    this.#detachPointer();
    this.#observer?.disconnect();
    this.#bgCanvas.remove();
    this.#canvas.remove();
  }

  // ---- PointerHost ----------------------------------------------------------

  canvas(): HTMLCanvasElement {
    return this.#canvas;
  }

  camera(): Camera {
    return this.#camera;
  }

  setCamera(camera: Camera): void {
    this.#camera = camera;
    this.#request();
  }

  hitAt(point: Point): number {
    if (this.#list === null) return -1;
    const world = { x: (point.x - this.#camera.x) / this.#camera.scale, y: (point.y - this.#camera.y) / this.#camera.scale };
    return hitTestNode(this.#list, world, { scale: this.#camera.scale, tolerancePx: HIT_TOLERANCE_PX });
  }

  onHover(index: number, at: Point | null): void {
    if (index === this.#hover) return;
    this.#hover = index;
    this.#callbacks.onHover(index, at);
    this.#request();
  }

  /** Click to highlight the neighbourhood; click the background to clear it. */
  onSelect(index: number): void {
    this.#selected = index;
    this.#neighbors = index < 0 || this.#list === null ? new Set<number>() : neighborsOf(this.#list, index);
    this.#callbacks.onSelect(index);
    this.#request();
  }

  // ---- frame loop -----------------------------------------------------------

  #resize(): void {
    const rect = this.#canvas.parentElement?.getBoundingClientRect() ?? this.#canvas.getBoundingClientRect();
    this.#width = Math.max(1, Math.round(rect.width));
    this.#height = Math.max(1, Math.round(rect.height));
    this.#dpr = Math.min(2, globalThis.devicePixelRatio || 1);
    this.#background.setSize(this.#width, this.#height, this.#dpr);
    this.#canvas.width = Math.round(this.#width * this.#dpr);
    this.#canvas.height = Math.round(this.#height * this.#dpr);
    this.#request();
  }

  #request(): void {
    if (this.#destroyed || this.#rafId !== null) return;
    this.#rafId = requestAnimationFrame(() => this.#frame());
  }

  #frame(): void {
    this.#rafId = null;
    if (this.#destroyed) return;
    if (this.#transitioning && performance.now() - this.#transitionStart >= TRANSITION_MS) {
      this.#transitioning = false;
    }
    this.#draw();
    // Nothing else drives the loop: a parked studio idles at zero frames, and a
    // transition or a hover change asks for exactly the frames it needs.
    if (this.#transitioning) this.#request();
  }

  #frameState(): Frame | null {
    if (this.#list === null) return null;
    const t = easeInOutCubic(transitionProgress(performance.now() - this.#transitionStart));
    return frameFor(this.#list, this.#from, this.#transitioning ? t : 1);
  }

  #paintState(frame: Frame, list: DrawList): PaintState {
    return {
      ctx: this.#canvas.getContext("2d") as CanvasRenderingContext2D,
      theme: this.#theme,
      camera: this.#camera,
      view: visibleWorldRect(this.#camera, this.#width, this.#height),
      list, frame, styles: this.#styles, edgeKinds: this.#edgeKinds,
      sprites: this.#sprites, hover: this.#hover, selected: this.#selected,
      neighbors: this.#neighbors, alpha: frame.alpha, dpr: this.#dpr,
    };
  }

  #draw(): void {
    const frame = this.#frameState();
    if (frame === null || this.#list === null || this.#canvas.getContext("2d") === null) return;
    paintFrame(this.#paintState(frame, this.#list));
  }
}
