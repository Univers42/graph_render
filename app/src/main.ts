/**
 * The standalone studio: the element, the size of the page. `?worker=off` runs the motor
 * on the page's own thread, where every layout freezes it: the perf gate's negative control.
 */
import { defineGraphStudio } from "../../packages/graph-studio/src/element.ts";

if (new URLSearchParams(location.search).get("worker") === "off") {
  const { spawnLocal } = await import("../../packages/graph-studio/src/motor/local.ts");
  defineGraphStudio({ spawn: spawnLocal });
} else {
  defineGraphStudio();
}
