/** A studio on a view that records what it is told. The motor behind it is the caller's. */
import type { Frame } from "../../graph-render/src/frame.ts";
import type { LabelPolicy } from "../../graph-render/src/labels.ts";
import type { Style } from "../../graph-render/src/style.ts";
import type { Theme } from "../../graph-render/src/theme.ts";
import type { ViewEvents } from "../../graph-render/src/view.ts";
import type { MotorClient } from "../src/motor/client.ts";
import type { Settings } from "../src/state/settings.ts";
import type { ViewFace } from "../src/studio/pipeline.ts";
import { type Studio, createStudio } from "../src/studio/studio.ts";

export interface Seen {
  readonly frames: { readonly frame: Frame; readonly animate: boolean }[];
  readonly styles: Style[];
  readonly themes: Theme[];
  readonly policies: LabelPolicy[];
  readonly calls: string[];
}

export interface Saved {
  readonly name: string;
  readonly data: Blob;
}

export interface Desk {
  readonly studio: Studio;
  readonly seen: Seen;
  readonly saved: Saved[];
  /** What a click on a node does. */
  readonly choose: (node: number) => void;
}

type Handlers = { [Name in keyof ViewEvents]: Set<(payload: ViewEvents[Name]) => void> };

function recordingView(seen: Seen, handlers: Handlers): ViewFace {
  return {
    setFrame: (frame, options = {}) => void seen.frames.push({ frame, animate: options.animate === true }),
    setStyle: (style) => void seen.styles.push(style),
    setTheme: (theme) => void seen.themes.push(theme),
    setLabels: (policy) => void seen.policies.push(policy),
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
  const seen: Seen = { frames: [], styles: [], themes: [], policies: [], calls: [] };
  const handlers: Handlers = { hover: new Set(), select: new Set(), camera: new Set(), frame: new Set() };
  const saved: Saved[] = [];
  let clock = 0;
  const studio = createStudio({
    client,
    view: recordingView(seen, handlers),
    save: (name, data) => void saved.push({ name, data }),
    now: () => (clock += 1),
    ...(settings === undefined ? {} : { settings }),
  });
  return {
    studio, seen, saved,
    choose: (node) => {
      for (const handler of handlers.select) handler(node);
    },
  };
}
