/** The 3D layer's backend policy: which frames `auto` and the two forced choices hand it, and on which renderer. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { SPACE_THRESHOLD, spaceWanted } from "../src/webgl2/hook3d.ts";
import { softwareNamed } from "../src/webgl2/layer3d.ts";
import { BULK_THRESHOLD } from "../src/webgl2/plan.ts";

test("auto takes the 3D layer from its own measured threshold, not the 2D layer's", () => {
  assert.equal(spaceWanted("auto", BULK_THRESHOLD, true), false);
  assert.equal(spaceWanted("auto", SPACE_THRESHOLD - 1, true), false);
  assert.equal(spaceWanted("auto", SPACE_THRESHOLD, true), true);
});

test("webgl2 takes the 3D layer at any size, canvas2d never, and a layer that could not be had is never wanted", () => {
  assert.equal(spaceWanted("webgl2", 3, true), true);
  assert.equal(spaceWanted("canvas2d", 10 * SPACE_THRESHOLD, true), false);
  assert.equal(spaceWanted("webgl2", 10 * SPACE_THRESHOLD, false), false);
  assert.equal(spaceWanted("auto", 10 * SPACE_THRESHOLD, false), false);
});

test("a software rasteriser is read from the renderer's name, and the measured GPU is not one", () => {
  const swiftshader = "ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)";
  const radeon = "ANGLE (AMD, Vulkan 1.4.305 (AMD Radeon RX 6600 (RADV NAVI23) (0x000073FF)), radv)";
  assert.equal(softwareNamed(swiftshader), true);
  assert.equal(softwareNamed("llvmpipe (LLVM 17.0.6, 256 bits)"), true);
  assert.equal(softwareNamed(radeon), false);
  assert.equal(softwareNamed(""), false);
});
