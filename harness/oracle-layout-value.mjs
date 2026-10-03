// The node weight rule harness/oracle-layouts.mjs feeds to d3's `.sum()`, split out so it
// can be unit-tested without the arm's 43 MB fixture read.
//
// A real (non-positive or non-finite) node weight clamps here — `treemap.rs`'s own
// Ponytail. The virtual root contributes 0, and it is discriminated by its `id`, the same
// way `collectById` discriminates it: `weight == null` would also be true of a real node
// whose weight is non-finite, since an f32 NaN serialises as JSON `null`.

/** `d3.hierarchy.sum`'s value function: the virtual root (no id) contributes `0`. */
export const WEIGHT_EPSILON = 1e-6;

const clampWeight = (weight) => (Number.isFinite(weight) && weight > 0 ? weight : WEIGHT_EPSILON);

export const nodeValue = (data) => (data.id === null ? 0 : clampWeight(data.weight));