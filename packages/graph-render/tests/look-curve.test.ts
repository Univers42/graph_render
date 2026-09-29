/**
 * The quadratic control point of a Line edge, SciGraphs' AUTO bend.
 *   core/scigraphs_core/mesh/edge_styles.py:108-145 (perp, sign) and 162-188 (offset, midpoint)
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { controlPoint } from "../src/edges2d/curve.ts";

function near(actual: { x: number; y: number } | null, x: number, y: number): void {
  assert.ok(actual, "expected a control point");
  assert.ok(Math.abs(actual.x - x) < 1e-12, `x ${actual.x} vs ${x}`);
  assert.ok(Math.abs(actual.y - y) < 1e-12, `y ${actual.y} vs ${y}`);
}

test("a horizontal edge bends toward +y: L=100, offset = 100*0.5*0.5 = 25", () => {
  // sign = (x0+y0 > x1+y1) ? 1 : -1 -> -1 here; perp of (1,0) is (0,-1) (edge_styles.py:122-124).
  near(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }), 50, 25);
});

test("the same edge reversed bends the same way, the AUTO property of edge_styles.py:113", () => {
  const forward = controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 });
  const backward = controlPoint({ x: 100, y: 0 }, { x: 0, y: 0 });
  assert.ok(forward && backward);
  near(backward, forward.x, forward.y);
});

test("a vertical edge bends toward -x, whichever end is named (edge_styles.py:122-124,143)", () => {
  // d = (0,1), perp = (dy, -dx) = (1, 0); sign = (0 > 100) ? 1 : -1 = -1.
  near(controlPoint({ x: 0, y: 0 }, { x: 0, y: 100 }), -25, 50);
  // Reversed, d = (0,-1), perp = (-1, 0) and sign = (100 > 0) ? 1 : -1 = 1: -25, 50 again.
  near(controlPoint({ x: 0, y: 100 }, { x: 0, y: 0 }), -25, 50);
});

test("a tie on x0+y0 takes the -1 sign, and the two directions then differ (edge_styles.py:143)", () => {
  // 0+0 == 1+(-1), and the comparison is not strict, so the sign is -1 both ways.
  // AUTO only promises the same bend when the two sums differ.
  near(controlPoint({ x: 0, y: 0 }, { x: 1, y: -1 }), 0.75, -0.25);
  near(controlPoint({ x: 1, y: -1 }, { x: 0, y: 0 }), 0.25, -0.75);
});

test("a diagonal edge uses the normalised (dy, -dx) perpendicular", () => {
  // L = sqrt(2), offset = sqrt(2)/4; perp = (1,-1)/sqrt(2); sign = (0 > 2) ? 1 : -1 = -1.
  const got = controlPoint({ x: 0, y: 0 }, { x: 1, y: 1 });
  assert.ok(got);
  near(got, 0.5 - 0.25, 0.5 + 0.25);
});

test("a zero-length edge has no control point", () => {
  assert.equal(controlPoint({ x: 5, y: 5 }, { x: 5, y: 5 }), null);
});

test("a length under 1e-10 has no direction to bend in (edge_styles.py:117)", () => {
  assert.equal(controlPoint({ x: 0, y: 0 }, { x: 1e-12, y: 0 }), null);
  assert.equal(controlPoint({ x: 0, y: 0 }, { x: 0.99e-10, y: 0 }), null);
  // Exactly at the threshold the edge still bends, so the test is a strict <.
  assert.ok(controlPoint({ x: 0, y: 0 }, { x: 1e-10, y: 0 }));
  assert.ok(controlPoint({ x: 0, y: 0 }, { x: 1e-9, y: 0 }));
});

test("curvature below 0.001 comes back straight, so the edge is drawn as a line", () => {
  assert.equal(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }, 0), null);
  assert.equal(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }, 0.0009), null);
  assert.ok(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }, 0.001));
});

test("the default curvature is CYTOSCAPE_BEZIER's own 0.5 (edge_styles.py:21-28)", () => {
  near(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }), 50, 25);
  near(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }, 0.5), 50, 25);
});

test("curvature scales the bend linearly with edge length", () => {
  near(controlPoint({ x: 0, y: 0 }, { x: 100, y: 0 }, 1), 50, 50);
});
