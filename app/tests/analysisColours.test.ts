// The two colour ramps an analysis overlay paints with: a sequential ramp for a
// magnitude (`f64` centrality) and a categorical palette for a labelling
// (`u32` community, component or depth). The stops are PINNED — a studio that
// recoloured a community on a rebuild would be a different picture every load,
// and a swapped stop or a rounding change is invisible anywhere else.

import assert from "node:assert/strict";
import { test } from "node:test";

import { GROUP_COLOURS, RAMP_STOPS, groupColour, rampColour } from "../src/render/analysisColours.ts";

function channels(hex: string): [number, number, number] {
  assert.match(hex, /^#[0-9a-f]{6}$/, `${hex} is not a lowercase #rrggbb`);
  return [
    Number.parseInt(hex.slice(1, 3), 16),
    Number.parseInt(hex.slice(3, 5), 16),
    Number.parseInt(hex.slice(5, 7), 16),
  ];
}

test("the sequential ramp is a five-stop cold-to-hot run", () => {
  assert.deepEqual(RAMP_STOPS, ["#12324a", "#1f6f8b", "#4bb0a4", "#f2d04b", "#e2603f"]);
});

test("the ends of the ramp are the ends of the stop list, exactly", () => {
  assert.equal(rampColour(0), RAMP_STOPS[0]);
  assert.equal(rampColour(1), RAMP_STOPS[RAMP_STOPS.length - 1]);
});

test("a value between two stops is the channel-wise mean, rounded to a byte", () => {
  // Five stops over 0..1, so a quarter of the range IS a stop and the
  // interpolation is only visible at three-eighths.
  assert.equal(rampColour(0.25), RAMP_STOPS[1]);
  const [r0, g0, b0] = channels(RAMP_STOPS[1]);
  const [r1, g1, b1] = channels(RAMP_STOPS[2]);
  const mid = channels(rampColour(0.375));
  assert.deepEqual(mid, [
    Math.round((r0 + r1) / 2),
    Math.round((g0 + g1) / 2),
    Math.round((b0 + b1) / 2),
  ]);
});

test("the half-way point of the whole ramp is the middle stop, not a mean of the ends", () => {
  assert.equal(rampColour(0.5), RAMP_STOPS[2]);
});

test("t outside 0..1, and a non-finite t, clamp to the ends rather than extrapolate", () => {
  assert.equal(rampColour(-4), RAMP_STOPS[0]);
  assert.equal(rampColour(7), RAMP_STOPS[RAMP_STOPS.length - 1]);
  assert.equal(rampColour(Number.NaN), RAMP_STOPS[0]);
});

test("every stop is a real colour and the run is cold at the bottom, hot at the top", () => {
  for (const stop of RAMP_STOPS) assert.match(stop, /^#[0-9a-f]{6}$/);
  const luma = (hex: string): number => {
    const [r, g, b] = channels(hex);
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const first = RAMP_STOPS[0];
  const last = RAMP_STOPS[RAMP_STOPS.length - 1];
  assert.ok(luma(first) < 80, `the cold end is too light to read as cold (${first})`);
  assert.ok(luma(last) > 120, `the hot end is too dark for a dark canvas (${last})`);
  assert.ok(channels(first)[2] > channels(first)[0], "the cold end is blue");
  assert.ok(channels(last)[0] > channels(last)[2], "the hot end is red");
});

test("the group palette is eight distinct colours, listed in rank order", () => {
  assert.equal(GROUP_COLOURS.length, 8);
  assert.equal(new Set(GROUP_COLOURS).size, 8);
  assert.equal(GROUP_COLOURS[0], "#e0937a");
});

test("groupColour cycles by rank, so a graph with more groups than colours still paints", () => {
  assert.equal(groupColour(0), GROUP_COLOURS[0]);
  assert.equal(groupColour(3), GROUP_COLOURS[3]);
  assert.equal(groupColour(GROUP_COLOURS.length), GROUP_COLOURS[0]);
  assert.equal(groupColour(GROUP_COLOURS.length + 2), GROUP_COLOURS[2]);
});

test("a negative rank clamps to the first colour rather than indexing off the front", () => {
  assert.equal(groupColour(-1), GROUP_COLOURS[0]);
});
