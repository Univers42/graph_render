/**
 * The standalone studio: the element, the size of the page. `?worker=off` runs the motor
 * on the page's own thread, where every layout freezes it: the perf gate's negative control.
 * `?backend=canvas2d|webgl2|auto` picks who draws the edges and nodes (graph-render view.ts).
 * `?threads=N` sets how many threads tick a live settle; 1 is the serial module (motor/threads.ts).
 */
import { backendOf, defineGraphStudio } from "../../packages/graph-studio/src/element.ts";

const query = new URLSearchParams(location.search);
const backend = backendOf(query.get("backend"));
const threads = Number.parseInt(query.get("threads") ?? "", 10);
const chosen = Number.isInteger(threads) ? { backend, threads } : { backend };
if (query.get("worker") === "off") {
  const { spawnLocal } = await import("../../packages/graph-studio/src/motor/local.ts");
  defineGraphStudio({ spawn: spawnLocal, ...chosen });
} else {
  defineGraphStudio(chosen);
}
