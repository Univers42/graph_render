//! Which children, in which order, and grouped into which row — the half of
//! `squarifyRatio` that decides *membership* rather than geometry. The parent module's
//! `squarify_children` places the rows this returns.
//!
//! Split out of `treemap.rs` for the house line limit. Nothing here departs from
//! `d3-hierarchy@3.1.2`'s `hierarchy/sum.js` and `treemap/squarify.js`; the port's
//! conventions are restated in that module's doc.

use super::Hierarchy;
use super::clamp_weight;
use crate::index::Topology;

/// Every row's `value`: own (clamped) weight plus children, summed last-child-first
/// (`hierarchy/sum.js`'s `while (--i >= 0) sum += children[i].value`) over
/// [`Hierarchy::order`] reversed — every child strictly deeper than its parent, so a node
/// is never summed before all of its own children are.
pub(super) fn node_values(topology: &Topology, hierarchy: &Hierarchy) -> Vec<f64> {
    let n = topology.node_count();
    let rows = n + u32::from(hierarchy.virtual_root().is_some());
    let mut value = vec![0.0; rows as usize];
    for &v in hierarchy.order().iter().rev() {
        let own = if v < n {
            clamp_weight(topology.node(v).weight)
        } else {
            0.0
        };
        let mut sum = own;
        for &child in hierarchy.children(v).iter().rev() {
            sum += value[child as usize];
        }
        value[v as usize] = sum;
    }
    value
}

/// `node.children.sort((a, b) => b.value - a.value)`: descending value; stable, so a tie
/// keeps the ascending dense index [`Hierarchy::children`] already hands in.
///
/// **`total_cmp` is the oracle's comparator on every value this module can hold.** The
/// aggregate is a sum of [`clamp_weight`] outputs, so it is never `NaN` and never `-0.0`,
/// and a non-finite *weight* is already clamped; `+inf` is reachable (a subtree summing
/// past `f64::MAX`) and there the two comparators agree: `b.value - a.value` is
/// `inf - inf == NaN`, which `Array.prototype.sort` coerces to `+0` — keep order — and
/// `total_cmp` reports two `+inf` as equal, which a stable sort also keeps in order. The
/// only input on which the two would differ is a `NaN` aggregate, and the clamp rules that
/// out. `tests`'s `two_infinite_aggregates_sort_as_equal_and_js_keeps_their_order_too`
/// builds the overflow and checks both orders. (d3's `hierarchy/sort.js` takes the
/// comparator as an argument; the treemap itself never sorts.)
pub(super) fn sorted_children(children: &[u32], value: &[f64]) -> Vec<u32> {
    let mut sorted = children.to_vec();
    sorted.sort_by(|&a, &b| value[b as usize].total_cmp(&value[a as usize]));
    sorted
}

/// The row's worst aspect ratio so far (`squarify.js`): `beta = sum^2 * alpha` folds in
/// the row's accumulated value and the box aspect.
fn ratio(max_value: f64, min_value: f64, sum_value: f64, alpha: f64) -> f64 {
    let beta = sum_value * sum_value * alpha;
    (max_value / beta).max(beta / min_value)
}

/// The next row from `children[i0..]` (`squarify.js`'s greedy grouping): grows it while
/// doing so keeps the worst ratio the same or better. Returns the row's end (exclusive)
/// and its summed value. Values are always positive (the weight clamp), so the zero-skip
/// `squarify.js` guards against never triggers here; kept so a looser future clamp does
/// not silently drop it.
///
/// **The rejected candidate is subtracted back, not discarded.** `squarify.js` folds
/// `nodeValue` into the running total as it walks and, on rejecting it, undoes that with
/// `sumValue -= nodeValue; break;`. That is a round trip through `f64`, and it does not
/// always land where it started: `(1.0 + 1e-6) - 1e-6` is `0.9999999999999999`, not
/// `1.0`. The row's value then feeds `alpha`, every `ratio` comparison, and each `k`
/// downstream, so keeping the untouched `sum_value` here instead diverges from the oracle
/// by an ulp across the whole subtree — and, because a d3 `S.y1` lands one ulp *past* the
/// unit square as a result, it is visible in the emitted geometry. The parent module's
/// `tests` pin it with `to_bits()`.
pub(super) fn extend_row(children: &[u32], i0: usize, alpha: f64, value: &[f64]) -> (usize, f64) {
    let n = children.len();
    let mut i1 = i0;
    let mut sum_value = value[children[i1] as usize];
    i1 += 1;
    while sum_value == 0.0 && i1 < n {
        sum_value = value[children[i1] as usize];
        i1 += 1;
    }
    let (mut min_value, mut max_value) = (sum_value, sum_value);
    let mut min_ratio = ratio(max_value, min_value, sum_value, alpha);
    while i1 < n {
        let node_value = value[children[i1] as usize];
        sum_value += node_value;
        min_value = min_value.min(node_value);
        max_value = max_value.max(node_value);
        let new_ratio = ratio(max_value, min_value, sum_value, alpha);
        if new_ratio > min_ratio {
            sum_value -= node_value;
            break;
        }
        min_ratio = new_ratio;
        i1 += 1;
    }
    (i1, sum_value)
}
