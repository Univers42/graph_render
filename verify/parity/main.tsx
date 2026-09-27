/**
 * Visual-parity harness.
 *
 * Mounts THREE engines in ONE document and compares them pairwise:
 *
 *   A "host"        osionos' in-tree packages/graph-engine   — the reference
 *   B "standalone"  this repo's src                          — the extraction
 *   C "host2"       osionos' in-tree copy AGAIN              — the control
 *
 * The control is the whole point. A/B alone cannot distinguish "the extraction
 * renders differently" from "the force layout had not settled to the same tick
 * count in both panels", and a graph engine's layout is time-driven, so the
 * second explanation is the likely one. A vs C is two instances of the SAME
 * source, so whatever it reports is pure measurement noise. Only a difference
 * that exceeds the A-vs-C floor is evidence of a real behavioural divergence.
 *
 * One document means one rAF clock, so the aurora background is phase-aligned
 * across panels and is not itself a source of difference.
 *
 * Verification rig — not part of the published package.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { ReactElement } from "react";

import { GraphView as HostView, useControls as useHostControls } from "@ge-host";
import {
  GraphView as StandaloneView,
  useControls as useStandaloneControls,
  buildSyntheticModel,
} from "@ge-standalone";

/**
 * Both variants get the identical model object, so input is held constant.
 * The fixture comes from the standalone copy because that is where
 * buildSyntheticModel now lives; the host copy predates it and does not need it,
 * since the model is data, not rendering.
 */
const MODEL = buildSyntheticModel(220);

type Variant = "host" | "standalone" | "host2";

type LayerDiff = {
  differing: number;
  maxDelta: number;
  total: number;
  checksumA: number;
  checksumB: number;
  bbox: DiffShape;
  grid: string;
};

type PairDiff = {
  fg: LayerDiff;
  bg: LayerDiff;
  differing: number;
  maxDelta: number;
  total: number;
};

type Result = {
  nodeCount: number;
  edgeCount: number;
  dpr: number;
  canvasSize: string;
  /** A vs C: two instances of the SAME source. This is the noise floor. */
  control: PairDiff | null;
  /** A vs B: host vs extraction. Compare `differing` against the control. */
  signal: PairDiff | null;
  controlGeo: Geometry | null;
  signalGeo: Geometry | null;
  /** Did the host's CSS custom properties actually reach the cascade? */
  tokensResolved: boolean;
  /** Presence check: a parity result is meaningless if the layer is blank. */
  ink: { host: { inkPixels: number; distinctColours: number; total: number } } | null;
  notes: string[];
};

declare global {
  interface Window {
    __parity?: Result;
    __parityReady?: boolean;
    __parityError?: string;
  }
}

/** Live engine handles, captured from onReady so state can be read directly. */
const ENGINES: Partial<Record<Variant, { getMinimapData: () => unknown }>> = {};

function Panel({ variant, onSettled }: { variant: Variant; onSettled: () => void }): ReactElement {
  const hostControls = useHostControls(`parity-${variant}`);
  const standaloneControls = useStandaloneControls(`parity-${variant}`);
  const View = variant === "standalone" ? StandaloneView : HostView;
  // useControls returns a { controls, update, reset } wrapper; GraphView wants the
  // Controls object itself. Passing the wrapper makes `controls.filter` undefined
  // and the scene throws on `controls.filter.tagColors`.
  const controls = variant === "standalone" ? standaloneControls.controls : hostControls.controls;
  const done = useRef(false);

  return (
    <div className="cell" id={variant}>
      <View
        model={MODEL}
        controls={controls}
        onReady={(engine) => {
          ENGINES[variant] = engine as unknown as { getMinimapData: () => unknown };
          // Freeze: the layout is seeded deterministically and ticked by hand, so
          // its state is a pure function of the model plus however many ticks have
          // landed. Freezing stops further movement, so the state we compare is
          // final.
          engine.freeze();
          if (done.current) return;
          done.current = true;
          onSettled();
        }}
      />
    </div>
  );
}

const ALL: Variant[] = ["host", "standalone", "host2"];

function App(): ReactElement {
  const [settled, setSettled] = useState<Record<string, boolean>>({});
  const allSettled = ALL.every((v) => settled[v]);

  const mark = useCallback(
    (variant: Variant) => () => setSettled((prev) => (prev[variant] ? prev : { ...prev, [variant]: true })),
    [],
  );

  useEffect(() => {
    if (!allSettled) return;
    let cancelled = false;
    let raf = 0;

    /**
     * Wait until every panel's graph layer stops changing.
     *
     * Reduced motion removes the reveal stagger, but the icon sprite atlas is
     * populated asynchronously — a node whose glyph has not been rasterised yet
     * draws without it, and two panels built microseconds apart can be on
     * opposite sides of that. Polling each panel until its fingerprint repeats
     * means every panel is compared from a warm, fully-settled draw.
     *
     * The fingerprint is a 32x32 downsample rather than a full getImageData: three
     * full 600x1288 reads per frame for hundreds of frames exhausts the renderer's
     * ImageData allocation ("Out of memory at ImageData creation"). A downsample
     * is ~4 KB per panel per frame and still changes whenever anything visible
     * changes.
     */
    const scratch = document.createElement("canvas");
    scratch.width = 32;
    scratch.height = 32;
    const sctx = scratch.getContext("2d", { willReadFrequently: true });

    const fingerprint = (variant: Variant): number => {
      const src = document.querySelector<HTMLCanvasElement>(`#${variant} .osio-graph__fg`);
      if (!src || !sctx) return -1;
      sctx.clearRect(0, 0, 32, 32);
      sctx.drawImage(src, 0, 0, 32, 32);
      return checksum(sctx.getImageData(0, 0, 32, 32).data);
    };

    const settle = async (): Promise<void> => {
      const prev = new Map<Variant, number>();
      const stable = new Map<Variant, number>();
      for (let frame = 0; frame < 150; frame += 1) {
        await new Promise<void>((r) => {
          raf = requestAnimationFrame(() => r());
        });
        if (cancelled) return;
        let allStable = true;
        for (const v of ALL) {
          const sum = fingerprint(v);
          if (prev.get(v) === sum) stable.set(v, (stable.get(v) ?? 0) + 1);
          else stable.set(v, 0);
          prev.set(v, sum);
          if ((stable.get(v) ?? 0) < 3) allStable = false;
        }
        if (allStable) return;
      }
    };

    void (async () => {
      await settle();
      // One more frame so the final draw is definitely flushed before reading.
      await new Promise<void>((r) => {
        raf = requestAnimationFrame(() => r());
      });
      if (cancelled) return;
      try {
        window.__parity = runComparisons();
      } catch (err) {
        window.__parityError = err instanceof Error ? `${err.name}: ${err.message}` : String(err);
      }
      window.__parityReady = true;
    })();

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
    };
  }, [allSettled]);

  return (
    <div className="row">
      {ALL.map((v) => (
        <Panel key={v} variant={v} onSettled={mark(v)} />
      ))}
    </div>
  );
}

/** FNV-1a over the RGBA bytes — order-sensitive, so it detects any shift. */
function checksum(data: Uint8ClampedArray): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < data.length; i += 1) {
    h ^= data[i];
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

function readPanel(variant: Variant, layer: string): ImageData {
  const el = document.querySelector<HTMLCanvasElement>(`#${variant} .${layer}`);
  if (!el) throw new Error(`missing canvas #${variant} .${layer}`);
  const ctx = el.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error(`no 2d context on #${variant} .${layer}`);
  return ctx.getImageData(0, 0, el.width, el.height);
}

/**
 * Where do the differing pixels actually sit?
 *
 * A count alone cannot distinguish "one node's icon is missing" from "every edge
 * is antialiased differently", and those mean very different things. The bounding
 * box plus a coarse 6x4 occupancy grid localises the difference without shipping
 * megabytes of coordinates: a tight box means a single element, a diffuse fill
 * means a systematic rendering difference.
 */
type DiffShape = {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  grid: string;
};

function shapeOf(ia: Uint8ClampedArray, ib: Uint8ClampedArray, w: number, h: number): DiffShape {
  const GX = 6;
  const GY = 4;
  const cells = new Array(GX * GY).fill(0) as number[];
  let x0 = w;
  let y0 = h;
  let x1 = -1;
  let y1 = -1;
  const n = Math.min(ia.length, ib.length);
  for (let i = 0; i < n; i += 4) {
    if (
      ia[i] === ib[i] &&
      ia[i + 1] === ib[i + 1] &&
      ia[i + 2] === ib[i + 2] &&
      ia[i + 3] === ib[i + 3]
    ) {
      continue;
    }
    const p = i / 4;
    const px = p % w;
    const py = (p - px) / w;
    if (px < x0) x0 = px;
    if (py < y0) y0 = py;
    if (px > x1) x1 = px;
    if (py > y1) y1 = py;
    const gx = Math.min(GX - 1, Math.floor((px / w) * GX));
    const gy = Math.min(GY - 1, Math.floor((py / h) * GY));
    cells[gy * GX + gx] += 1;
  }
  const rows: string[] = [];
  for (let gy = 0; gy < GY; gy += 1) {
    rows.push(cells.slice(gy * GX, gy * GX + GX).map((c) => String(c).padStart(5, " ")).join(""));
  }
  return { x0, y0, x1, y1, grid: rows.join("\n") };
}


function diffPair(aVariant: Variant, bVariant: Variant): PairDiff {
  // The canvas class names are not the PairDiff field names; map explicitly
  // rather than indexing by the raw class string.
  const LAYERS = [
    { cls: "osio-graph__fg", key: "fg" },
    { cls: "osio-graph__bg", key: "bg" },
  ] as const;
  const out = {} as Record<string, LayerDiff>;
  for (const { cls, key } of LAYERS) {
    const imgA = readPanel(aVariant, cls);
    const ia = imgA.data;
    const ib = readPanel(bVariant, cls).data;
    const elWidth = imgA.width;
    const elHeight = imgA.height;
    let differing = 0;
    let maxDelta = 0;
    const n = Math.min(ia.length, ib.length);
    for (let i = 0; i < n; i += 4) {
      const d =
        Math.abs(ia[i] - ib[i]) +
        Math.abs(ia[i + 1] - ib[i + 1]) +
        Math.abs(ia[i + 2] - ib[i + 2]) +
        Math.abs(ia[i + 3] - ib[i + 3]);
      if (d !== 0) {
        differing += 1;
        if (d > maxDelta) maxDelta = d;
      }
    }
    out[key] = {
      differing,
      maxDelta,
      total: n / 4,
      checksumA: checksum(ia),
      checksumB: checksum(ib),
      bbox: shapeOf(ia, ib, elWidth, elHeight),
      grid: shapeOf(ia, ib, elWidth, elHeight).grid,
    };
  }
  const fg = out.fg;
  const bg = out.bg;
  return {
    fg,
    bg,
    differing: fg.differing + bg.differing,
    maxDelta: Math.max(fg.maxDelta, bg.maxDelta),
    total: fg.total + bg.total,
  };
}

/**
 * Compare two engines' actual layout state, not their rasterized output.
 *
 * This is the measurement that matters. The force layout is time-driven, so two
 * panels that were constructed microseconds apart can be at different tick counts
 * by the time they are frozen, and their pixels will differ even though the code
 * is identical — which is exactly what the pixel control demonstrates. Reading the
 * positions, camera and bounds directly turns "the pictures look different" into
 * "the largest disagreement in node position is N world units", and that number
 * is directly comparable between the control pair and the signal pair.
 */
type Geometry = {
  count: number;
  maxAbsPosDelta: number;
  posDeltas: number;
  maxAbsBoundsDelta: number;
  cameraEqual: boolean;
  cameraA: string;
  cameraB: string;
  signatureA: string;
  signatureB: string;
};

function readGeometry(variant: Variant): {
  x: Float32Array;
  y: Float32Array;
  count: number;
  bounds: unknown;
  camera: { x: number; y: number; scale: number };
  width: number;
  height: number;
} {
  const engine = ENGINES[variant];
  if (!engine) throw new Error(`no engine captured for ${variant}`);
  return engine.getMinimapData() as never;
}

function flatBounds(b: unknown): number[] {
  if (!b || typeof b !== "object") return [];
  return Object.values(b as Record<string, number>);
}

function compareGeometry(aVariant: Variant, bVariant: Variant): Geometry {
  const a = readGeometry(aVariant);
  const b = readGeometry(bVariant);
  const n = Math.min(a.count, b.count);
  let maxAbsPosDelta = 0;
  let posDeltas = 0;
  for (let i = 0; i < n; i += 1) {
    const d = Math.max(Math.abs(a.x[i] - b.x[i]), Math.abs(a.y[i] - b.y[i]));
    if (d !== 0) {
      posDeltas += 1;
      if (d > maxAbsPosDelta) maxAbsPosDelta = d;
    }
  }
  const ba = flatBounds(a.bounds);
  const bb = flatBounds(b.bounds);
  let maxAbsBoundsDelta = 0;
  for (let i = 0; i < Math.max(ba.length, bb.length); i += 1) {
    const d = Math.abs((ba[i] ?? 0) - (bb[i] ?? 0));
    if (d > maxAbsBoundsDelta) maxAbsBoundsDelta = d;
  }
  const camA = `${a.camera.x},${a.camera.y},${a.camera.scale}`;
  const camB = `${b.camera.x},${b.camera.y},${b.camera.scale}`;
  return {
    count: n,
    maxAbsPosDelta,
    posDeltas,
    maxAbsBoundsDelta,
    cameraEqual: camA === camB,
    cameraA: camA,
    cameraB: camB,
    signatureA: "",
    signatureB: "",
  };
}

/**
 * Is the graph layer actually drawn?
 *
 * This exists because the check once reported PIXEL-IDENTICAL across every palette
 * while the graph layer was entirely transparent: the clock had been pinned to a
 * constant, which froze the reveal stagger at progress 0, so no node was ever
 * painted. Two empty canvases are trivially identical, so the comparison was
 * meaningless while looking perfect.
 *
 * A parity check must therefore establish that it is comparing something. Ink
 * pixels are pixels with any non-zero alpha, and distinct colours catches a
 * further degenerate case (a flat fill). Both are reported so a future failure
 * mode is diagnosable rather than just "it said identical".
 */
function inkStats(variant: Variant): { inkPixels: number; distinctColours: number; total: number } {
  const img = readPanel(variant, "osio-graph__fg");
  const d = img.data;
  let ink = 0;
  const seen = new Set<number>();
  // Sample every 7th pixel: enough for a presence check, cheap enough not to
  // dominate the comparison cost.
  for (let i = 0; i < d.length; i += 4 * 7) {
    if (d[i + 3] !== 0) {
      ink += 1;
      seen.add(((d[i] >> 3) << 10) | ((d[i + 1] >> 3) << 5) | (d[i + 2] >> 3));
    }
  }
  return { inkPixels: ink, distinctColours: seen.size, total: Math.floor(d.length / (4 * 7)) };
}

function runComparisons(): Result {
  const notes: string[] = [];
  const probe = readPanel("host", "osio-graph__fg");
  const dpr = window.devicePixelRatio;

  const control = diffPair("host", "host2");
  const signal = diffPair("host", "standalone");

  const controlGeo = compareGeometry("host", "host2");
  const signalGeo = compareGeometry("host", "standalone");

  const ink = inkStats("host");
  const ds = document.documentElement.dataset;
  // Report the actual resolved token values, not just the requested palette. A
  // sweep over 16 combinations is only meaningful if the engine really saw 16
  // different token sets; reading the values back off the document is what
  // proves the parameter reached the cascade.
  const cs = getComputedStyle(document.documentElement);
  const probeTokens = ["--osio-graph-select", "--osio-graph-note", "--osio-graph-ink", "--osio-graph-bg-0"].map(
    (name) => `${name}=${cs.getPropertyValue(name).trim()}`,
  );
  // A token that resolves to "" means the stylesheet never loaded and the engine
  // is silently using its own hardcoded fallbacks. Two identical fallback-coloured
  // renders are identical, so this must be able to fail the run.
  const tokensResolved = probeTokens.every((entry) => {
    const value = entry.slice(entry.indexOf("=") + 1).trim();
    return value.length > 0;
  });
  notes.push(`document data-theme=${ds.theme} data-palette=${ds.palette} tokensResolved=${tokensResolved}`);
  notes.push(`resolved ${probeTokens.join("  ")}`);
  notes.push(
    `INK host: ${ink.inkPixels}/${ink.total} sampled px have alpha, ${ink.distinctColours} distinct colours`,
  );
  notes.push(
    `GEOMETRY control  host vs host2 : ${controlGeo.posDeltas}/${controlGeo.count} nodes differ, max |delta| ${controlGeo.maxAbsPosDelta}, cameraEqual ${controlGeo.cameraEqual}`,
  );
  notes.push(
    `GEOMETRY signal   host vs stand.: ${signalGeo.posDeltas}/${signalGeo.count} nodes differ, max |delta| ${signalGeo.maxAbsPosDelta}, cameraEqual ${signalGeo.cameraEqual}`,
  );
  notes.push(
    `PIXELS    control  ${control.differing}/${control.total} differ (maxDelta ${control.maxDelta})`,
  );
  notes.push(`PIXELS    signal   ${signal.differing}/${signal.total} differ (maxDelta ${signal.maxDelta})`);

  return {
    nodeCount: MODEL.nodes.length,
    edgeCount: MODEL.edges.length,
    dpr,
    canvasSize: `${probe.width}x${probe.height}`,
    control,
    signal,
    controlGeo,
    signalGeo,
    ink: { host: ink },
    tokensResolved,
    notes,
  };
}

const rootEl = document.getElementById("root");
if (rootEl) createRoot(rootEl).render(<App />);
