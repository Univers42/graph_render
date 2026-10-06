/** A studio on a view that records what it is told. The motor behind it is the caller's. */
import type { Camera } from "../../graph-render/src/camera.ts";
import type { Frame } from "../../graph-render/src/frame.ts";
import type { LabelPolicy } from "../../graph-render/src/labels.ts";
import { EMPTY_FRAME } from "../../graph-render/src/scene.ts";
import type { Style } from "../../graph-render/src/style.ts";
import type { Theme } from "../../graph-render/src/theme.ts";
import type { ViewEvents } from "../../graph-render/src/view.ts";
import type { ForceLink } from "../src/actions/forces.ts";
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

/** The bytes of what the studio saved, as `Uint8Array`; throws when it saved nothing. */
export async function savedBytes(saved: readonly Saved[], at = 0): Promise<Uint8Array> {
  const blob = saved[at]?.data;
  if (blob === undefined) throw new Error(`the studio saved nothing at ${at}`);
  return new Uint8Array(await blob.arrayBuffer());
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

/** The 3D camera of the frame on the desk, or null when that frame is 2D. */
const ORBIT = { yaw: 0.4, pitch: 0, distance: 1, target: { x: 0, y: 0, z: 0 }, fov: 1, limits: { min: 0.1, max: 10 } };

function held(seen: Seen): Frame {
  return seen.frames.at(-1)?.frame ?? EMPTY_FRAME;
}

/**
 * The 3D camera's four faces, off the frame the desk last drew. A 2D frame has no z column,
 * so it has no orbit and no projection — which is the condition the studio's own badge and
 * reset action read, so a test that hangs these off the frame is testing the real thing.
 */
function spaceFace(seen: Seen): Pick<ViewFace, "orbit" | "setOrbit" | "resetOrbit" | "projected"> {
  return {
    orbit: () => (held(seen).z === null ? null : ORBIT),
    setOrbit: (orbit) => void seen.calls.push(`setOrbit ${orbit.yaw}`),
    resetOrbit: () => void seen.calls.push("resetOrbit"),
    projected: () => {
      const frame = held(seen);
      const z = frame.z;
      if (z === null) return null;
      return Array.from({ length: frame.nodeCount }, (_, node) => ({
        node, x: frame.x[node] ?? 0, y: frame.y[node] ?? 0, depth: z[node] ?? 0,
      }));
    },
  };
}

/**
 * The pins this desk holds, in the order they were set; `pinned()` hands the same array back,
 * so a test reads what the view would be showing rather than what it was told to show. Hide is
 * the third of the three node gestures the studio drives, so it is recorded beside them.
 */
function pinFace(seen: Seen, pins: number[]): Pick<ViewFace, "pinned" | "togglePin" | "hide"> {
  return {
    pinned: () => pins,
    togglePin: (node) => {
      const at = pins.indexOf(node);
      if (at >= 0) pins.splice(at, 1);
      else pins.push(node);
      seen.calls.push(`togglePin ${node}`);
    },
    hide: (nodes) => void seen.calls.push(`hide ${nodes.join(" ")}`),
  };
}

/** `selectMany` tells the selection's listeners, as the real view does, so a host event can follow it. */
function selectionFace(seen: Seen, handlers: Handlers): Pick<ViewFace, "select" | "selectMany"> {
  return {
    select: (node) => void seen.calls.push(`select ${node}`),
    selectMany: (nodes) => {
      seen.calls.push(`selectMany ${nodes.join(",")}`);
      for (const handler of handlers.select) handler(nodes.at(-1) ?? -1);
      for (const handler of handlers.selection) handler(nodes);
    },
  };
}

function recordingView(seen: Seen, handlers: Handlers): ViewFace {
  const pins: number[] = [];
  return {
    setFrame: (frame, options = {}) => void seen.frames.push({ frame, animate: options.animate === true }),
    setStyle: (style) => void seen.styles.push(style),
    setTheme: (theme) => void seen.themes.push(theme),
    setLabels: (policy) => void seen.policies.push(policy),
    crossFade: () => undefined,
    setCamera: (camera) => void seen.cameras.push(camera),
    frame: () => held(seen),
    viewport: () => DESK_VIEWPORT,
    fit: () => void seen.calls.push("fit"),
    reset: () => void seen.calls.push("reset"),
    ...spaceFace(seen),
    zoomBy: (factor) => void seen.calls.push(`zoomBy ${factor}`),
    panBy: (delta) => void seen.calls.push(`panBy ${delta.x} ${delta.y}`),
    limits: () => ({ min: 0.02, max: 40 }),
    focus: (node) => void seen.calls.push(`focus ${node}`),
    ...selectionFace(seen, handlers),
    ...pinFace(seen, pins),
    local: (node, options) => {
      seen.calls.push(`local ${node} ${JSON.stringify(options)}`);
      if (node === 0 && options.depth === 2 && options.incoming && !options.outgoing && options.neighbours) {
        return [0, 1, 2] as const;
      }
      if (node === 1 && options.depth === 1 && !options.incoming && !options.outgoing && !options.neighbours) {
        return [1] as const;
      }
      return [node] as const;
    },
    showAll: () => void seen.calls.push("showAll"),
    on: (name, handler) => {
      handlers[name].add(handler);
      return () => void handlers[name].delete(handler);
    },
    toPNG: () => Promise.resolve(new Blob(["png"], { type: "image/png" })),
  };
}

/** `forces` is the live link behind the Forces actions; without it they are all unavailable. */
export function desk(client: MotorClient, settings?: Settings, forces?: ForceLink): Desk {
  const seen: Seen = { frames: [], styles: [], themes: [], policies: [], calls: [], cameras: [] };
  const handlers: Handlers = { hover: new Set(), select: new Set(), selection: new Set(), camera: new Set(), context: new Set(), frame: new Set() };
  const saved: Saved[] = [];
  let clock = 0;
  const view = recordingView(seen, handlers);
  const studio = createStudio({
    client,
    view,
    save: (name, data) => void saved.push({ name, data }),
    now: () => (clock += 1),
    ...(settings === undefined ? {} : { settings }),
    ...(forces === undefined ? {} : { forces }),
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

/**
 * The same three discs, placed in space: a z column spliced in after y and header byte 14
 * set to 1, which is what the motor writes for a 3D layout. Built as bytes rather than by
 * patching a 2D run, because the radius column moves when the z column arrives and a patch
 * would not notice.
 */
export function spaceBytes(): Uint8Array {
  const z = new Uint8Array(Float32Array.of(0, 30, -30).buffer);
  const parts = spaceParts(z);
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.byteLength, 0));
  let at = 0;
  for (const part of parts) { out.set(part, at); at += part.byteLength; }
  return out;
}

function spaceParts(z: Uint8Array): Uint8Array[] {
  const header = new Uint8Array(Uint32Array.of(0x4e534d47, 0, 0, 1, 1, 3, 2).buffer);
  header[14] = 1;
  const x = new Uint8Array(Float32Array.of(0, 40, 80).buffer);
  const y = new Uint8Array(Float32Array.of(0, 0, 0).buffer);
  const r = new Uint8Array(Float32Array.of(4, 4, 4).buffer);
  const source = new Uint8Array(ENDS.source.buffer);
  const target = new Uint8Array(ENDS.target.buffer);
  return [header, table(IDS), table(["e0", "e1"]), source, target, x, y, z, r];
}

/** A motor that answers with the scripted graph: what the wasm is not there to lay out. */
export function scriptedClient(): MotorClient {
  return {
    catalog: () => Promise.resolve({ layouts: ["layout.forceatlas2", "layout.grid"], posts: [], analyses: [] }),
    load: () => Promise.resolve({ name: "scripted", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 }),
    // The values are echoed back, as the motor does: the studio reads them off the run report.
    layout: (layoutId, postId, params = {}) => Promise.resolve({
      layoutId, postId, postError: null, params, bytes: scriptBytes(), digest: null,
      layoutMs: 1, postMs: 0, meta: SCRIPTED_META,
    }),
    params: () => Promise.resolve([]),
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
    params: () => never("publish a schema"),
    analysis: () => never("analyse"),
    cancel: () => false,
    busy: () => false,
    close: () => undefined,
  };
}
