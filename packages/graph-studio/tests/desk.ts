/** A studio on a view that records what it is told. The motor behind it is the caller's. */
import type { Camera } from "../../graph-render/src/camera.ts";
import type { Frame } from "../../graph-render/src/frame.ts";
import type { LabelPolicy } from "../../graph-render/src/labels.ts";
import { EMPTY_FRAME } from "../../graph-render/src/scene.ts";
import type { Style } from "../../graph-render/src/style.ts";
import type { Theme } from "../../graph-render/src/theme.ts";
import type { ViewEvents } from "../../graph-render/src/view.ts";
import type { MotorClient } from "../src/motor/client.ts";
import type { GraphMeta } from "../src/source/meta.ts";
import { metaOf } from "../src/source/meta.ts";
import type { Settings } from "../src/state/settings.ts";
import { type Pipeline, type ViewFace, createPipeline } from "../src/studio/pipeline.ts";
import { type Studio, createStudio } from "../src/studio/studio.ts";
import { node } from "./support.ts";

export interface Seen {
  readonly frames: { readonly frame: Frame; readonly animate: boolean }[];
  readonly styles: Style[];
  readonly themes: Theme[];
  readonly policies: LabelPolicy[];
  readonly calls: string[];
  /** Every camera the studio set, in order. */
  readonly cameras: Camera[];
}

export interface Saved {
  readonly name: string;
  readonly data: Blob;
}

export interface Desk {
  readonly studio: Studio;
  /** The same pipeline the actions drive, for a test that must hand it a settings document. */
  readonly pipeline: Pipeline;
  readonly seen: Seen;
  readonly saved: Saved[];
  /** What a click on a node does. */
  readonly choose: (node: number) => void;
}

type Handlers = { [Name in keyof ViewEvents]: Set<(payload: ViewEvents[Name]) => void> };

/** The canvas the recording view reports; big enough that a fit leaves a readable scale. */
export const DESK_VIEWPORT = { width: 800, height: 600 };

function recordingView(seen: Seen, handlers: Handlers): ViewFace {
  return {
    setFrame: (frame, options = {}) => void seen.frames.push({ frame, animate: options.animate === true }),
    setStyle: (style) => void seen.styles.push(style),
    setTheme: (theme) => void seen.themes.push(theme),
    setLabels: (policy) => void seen.policies.push(policy),
    setCamera: (camera) => void seen.cameras.push(camera),
    frame: () => seen.frames.at(-1)?.frame ?? EMPTY_FRAME,
    viewport: () => DESK_VIEWPORT,
    fit: () => void seen.calls.push("fit"),
    reset: () => void seen.calls.push("reset"),
    zoomBy: (factor) => void seen.calls.push(`zoomBy ${factor}`),
    panBy: (delta) => void seen.calls.push(`panBy ${delta.x} ${delta.y}`),
    limits: () => ({ min: 0.02, max: 40 }),
    focus: (node) => void seen.calls.push(`focus ${node}`),
    select: (node) => void seen.calls.push(`select ${node}`),
    on: (name, handler) => {
      handlers[name].add(handler);
      return () => void handlers[name].delete(handler);
    },
    toPNG: () => Promise.resolve(new Blob(["png"], { type: "image/png" })),
  };
}

export function desk(client: MotorClient, settings?: Settings): Desk {
  const seen: Seen = { frames: [], styles: [], themes: [], policies: [], calls: [], cameras: [] };
  const handlers: Handlers = { hover: new Set(), select: new Set(), camera: new Set(), frame: new Set() };
  const saved: Saved[] = [];
  let clock = 0;
  const view = recordingView(seen, handlers);
  const studio = createStudio({
    client,
    view,
    save: (name, data) => void saved.push({ name, data }),
    now: () => (clock += 1),
    ...(settings === undefined ? {} : { settings }),
  });
  return {
    studio, seen, saved,
    pipeline: createPipeline({ client, view, store: studio.store }),
    choose: (node) => {
      for (const handler of handlers.select) handler(node);
    },
  };
}

const LABELS = ["Alpha", "Beta", "Gamma"];
const IDS = ["a", "b", "c"];
const ENDS = { source: Uint32Array.of(0, 1), target: Uint32Array.of(1, 2) };

/** The graph the scripted motor describes: three nodes, one group of two and one of one. */
export const SCRIPTED_META: GraphMeta = metaOf(
  LABELS.map((label, at) => node(IDS[at] ?? "", { label, group: at < 2 ? "red" : "blue" })),
  IDS,
  ENDS,
);

function table(ids: readonly string[]): Uint8Array {
  const encoder = new TextEncoder();
  const parts = ids.map((id) => encoder.encode(id));
  const total = parts.reduce((sum, part) => sum + part.byteLength, 0);
  const offsets = [0];
  for (const part of parts) offsets.push((offsets.at(-1) ?? 0) + part.byteLength);
  const out = new Uint8Array(4 * (ids.length + 1) + total + ((4 - (total % 4)) % 4));
  new Uint32Array(out.buffer).set(offsets);
  let at = 4 * (ids.length + 1);
  for (const part of parts) { out.set(part, at); at += part.byteLength; }
  return out;
}

/** GMSN bytes for a line of three discs, so a scripted run decodes like a real one. */
export function scriptBytes(): Uint8Array {
  const header = new Uint8Array(Uint32Array.of(0x4e534d47, 0, 0, 1, 1, 3, 2).buffer);
  const x = new Uint8Array(Float32Array.of(0, 40, 80).buffer);
  const y = new Uint8Array(Float32Array.of(0, 0, 0).buffer);
  const r = new Uint8Array(Float32Array.of(4, 4, 4).buffer);
  const source = new Uint8Array(ENDS.source.buffer);
  const target = new Uint8Array(ENDS.target.buffer);
  const parts = [header, table(IDS), table(["e0", "e1"]), source, target, x, y, r];
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.byteLength, 0));
  let at = 0;
  for (const part of parts) { out.set(part, at); at += part.byteLength; }
  return out;
}

/** A motor that answers with the scripted graph: what the wasm is not there to lay out. */
export function scriptedClient(): MotorClient {
  return {
    catalog: () => Promise.resolve({ layouts: ["layout.forceatlas2", "layout.grid"], posts: [], analyses: [] }),
    load: () => Promise.resolve({ name: "scripted", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 }),
    layout: (layoutId, postId) => Promise.resolve({
      layoutId, postId, postError: null, bytes: scriptBytes(), digest: null,
      layoutMs: 1, postMs: 0, meta: SCRIPTED_META,
    }),
    analysis: () => Promise.reject(new Error("the scripted motor measures nothing")),
    cancel: () => false,
    busy: () => false,
    close: () => undefined,
  };
}

/** Nothing in the chrome asks the motor; these refuse if anything ever does. */
export function refusingClient(): MotorClient {
  const never = (what: string): never => {
    throw new Error(`the test client was asked to ${what}`);
  };
  return {
    catalog: () => never("open"),
    load: () => never("load"),
    layout: () => never("lay out"),
    analysis: () => never("analyse"),
    cancel: () => false,
    busy: () => false,
    close: () => undefined,
  };
}
