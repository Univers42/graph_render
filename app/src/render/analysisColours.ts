/**
 * The two colour vocabularies an analysis overlay needs, and nothing else.
 *
 * The engine's theme has node fills and edge strokes but no way to say "this
 * number, higher is hotter" or "this community, distinct from that one" — so
 * these two ramps are the studio's own, pinned here rather than computed, because
 * a studio whose colours moved between loads could not be compared with a
 * screenshot of itself. `NEUTRAL_FILL` is the same bargain: a studio colour, not
 * an engine one.
 */

/**
 * Cold to hot, for a magnitude. The stops are equally spaced, so a value at the
 * middle of the range lands on the middle stop; the ends are the range's own
 * minimum and maximum as they are read, not the ramp's first and last sample.
 */
export const RAMP_STOPS: readonly string[] = [
  "#12324a", "#1f6f8b", "#4bb0a4", "#f2d04b", "#e2603f",
];

/**
 * Distinct hues for a labelling, assigned by RANK and not by the label's own
 * value: a community id is an opaque number, so the palette order is the only
 * thing that decides which community is which colour. Eight is as many as stay
 * tellable apart at the studio's node size; past eight the palette cycles, and
 * the panel says how many groups there are rather than pretending 36 groups are
 * 36 colours.
 */
export const GROUP_COLOURS: readonly string[] = [
  "#e0937a", "#7fb2c4", "#b6d7a8", "#f2c14e", "#c3aed6", "#f4a582", "#8dd3c7", "#feffb3",
];

function channelsOf(hex: string): readonly [number, number, number] {
  return [
    Number.parseInt(hex.slice(1, 3), 16),
    Number.parseInt(hex.slice(3, 5), 16),
    Number.parseInt(hex.slice(5, 7), 16),
  ];
}

function hex(r: number, g: number, b: number): string {
  const byte = (value: number): string => Math.round(value).toString(16).padStart(2, "0");
  return `#${byte(r)}${byte(g)}${byte(b)}`;
}

/**
 * The ramp colour at `t` in 0..1: the stops are interpolated channel-wise
 * between the two neighbouring ones. `t` outside the range — and a non-finite `t`,
 * which a diverged eigen-iterate can produce — is clamped to an end rather than
 * extrapolated into a colour nobody chose.
 */
export function rampColour(t: number): string {
  if (!Number.isFinite(t)) return RAMP_STOPS[0];
  const clamped = Math.min(1, Math.max(0, t));
  const last = RAMP_STOPS.length - 1;
  const at = clamped * last;
  const low = Math.min(last, Math.floor(at));
  const from = channelsOf(RAMP_STOPS[low]);
  if (low === last) return RAMP_STOPS[last];
  const to = channelsOf(RAMP_STOPS[low + 1]);
  const mix = at - low;
  return hex(
    from[0] + (to[0] - from[0]) * mix,
    from[1] + (to[1] - from[1]) * mix,
    from[2] + (to[2] - from[2]) * mix,
  );
}

/** The palette colour for a group of this rank; a negative rank is the first. */
export function groupColour(rank: number): string {
  const size = GROUP_COLOURS.length;
  return GROUP_COLOURS[((Math.max(0, Math.trunc(rank)) % size) + size) % size];
}
