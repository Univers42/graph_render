/** The colours nodes take. Hues that hold on both the dark and the light ground. */

export const GROUP_PALETTE: readonly string[] = [
  "#7c9cf5", "#f2a65a", "#6fcf97", "#e57b9e", "#b48ef0",
  "#4fc3d9", "#e6c65a", "#ef7f6b", "#8fbf5f", "#a9a9b8",
];

/** A node nothing is known about, or colouring switched off. */
export const MUTED = "#8b8d98";

const STOPS: readonly (readonly [number, number, number])[] = [
  [0x3d, 0x2f, 0x8f], [0x7a, 0x3f, 0xb5], [0xc2, 0x47, 0x8f], [0xf0, 0x80, 0x5a], [0xf7, 0xd1, 0x54],
];

function hex(channel: number): string {
  return Math.round(channel).toString(16).padStart(2, "0");
}

function colourAt(t: number): string {
  const scaled = Math.min(1, Math.max(0, t)) * (STOPS.length - 1);
  const low = Math.min(STOPS.length - 2, Math.floor(scaled));
  const from = STOPS[low] ?? [0, 0, 0];
  const to = STOPS[low + 1] ?? from;
  const mix = scaled - low;
  return `#${from.map((channel, i) => hex(channel + ((to[i] ?? channel) - channel) * mix)).join("")}`;
}

/** Low to high, `steps` colours, the two ends included. */
export function rampOf(steps: number): readonly string[] {
  return Array.from({ length: steps }, (_, i) => colourAt(steps < 2 ? 0 : i / (steps - 1)));
}

export const RAMP: readonly string[] = rampOf(24);
