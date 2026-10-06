/**
 * A view face that answers everything and draws nothing, recording each call by name. What the
 * studio asks of the view is then readable out of `calls`, and nothing else about the rendering
 * can leak into an action's result — which is what an action test wants.
 */
import type { ViewFace } from "../src/studio/pipeline.ts";

export function silentView(calls: string[]): ViewFace {
  const noop = (): void => undefined;
  return {
    setFrame: noop, setStyle: noop, setTheme: noop, setLabels: noop, crossFade: noop,
    fit: () => void calls.push("fit"),
    reset: () => void calls.push("reset"),
    zoomBy: () => void calls.push("zoomBy"), panBy: () => void calls.push("panBy"),
    limits: () => ({ min: 0.02, max: 40 }),
    focus: () => void calls.push("focus"), select: () => void calls.push("select"),
    selectMany: () => void calls.push("selectMany"),
    local: (node) => [node], showAll: () => void calls.push("showAll"),
    pinned: () => [], togglePin: () => void calls.push("togglePin"), hide: () => void calls.push("hide"),
    on: () => noop,
    toPNG: () => Promise.resolve(new Blob(["png"], { type: "image/png" })),
    setCamera: () => void calls.push("setCamera"),
    // The 3D camera's four faces. A silent view holds no frame, so it has no orbit.
    orbit: () => null, projected: () => null, setOrbit: noop, resetOrbit: noop,
    frame: () => { throw new Error("the silent view holds no frame"); },
    viewport: () => ({ width: 800, height: 600 }),
  };
}