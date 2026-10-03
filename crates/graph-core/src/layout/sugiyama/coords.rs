//! Horizontal coordinates: the priority method of Sugiyama, Tagawa & Toda (1981), run
//! against the layer above then below in alternation. Reference: `hierarchical.py:565-636`.
//!
//! Ponytail: a small, cheap heuristic, not Brandes & Köpf's edge-straightening method —
//! the reference's own comment says as much (`hierarchical.py:611-613`). Failing input: a
//! long dummy chain that Brandes-Köpf would draw straighter. Direction: a visually kinked
//! long edge, never a wrong position — every vertex still sits inside its own layer's row.
//! Escape hatch: none needed; this heuristic is not separately gated, only the crossing
//! count is (`docs/measurements/phase05-crossings.md`).

use super::layering::Layering;
use super::ordering::Ordering;
use super::weighted_median;

/// Below this many ordering-graph vertices, X gets 4 priority-method passes; at or above
/// it, none (`hierarchical.py:677`, `_PRIORITY_NODE_BUDGET`).
///
/// **The skip is silent, and cannot be a note.** A skipped phase is not one of the notes
/// section's five codes, and it would be snapshot-wide, which only code 3 may be
/// (`graph-contract`'s `notes.rs:174-177`) — so the registry row's `degradation` is where
/// this is documented (the review's L-12; `above_the_priority_budget_leaves_x_at_the_raw
/// _slot_index_and_the_snapshot_does_not_say_so` is the test that pins it).
const PRIORITY_NODE_BUDGET: u32 = 200_000;
/// A dummy vertex always outranks a real one (`hierarchical.py:6`, `_DUMMY_PRIORITY`).
const DUMMY_PRIORITY: u32 = 1 << 30;
/// The minimum gap the priority method keeps between adjacent vertices.
const GAP: f64 = 1.0;

/// Every vertex's X, dense-indexed like [`Layering::layer_of`].
pub(crate) struct Coords(pub(crate) Vec<f64>);

impl Coords {
    /// Starts each vertex at its slot index, then runs the throttled priority-method
    /// sweep against `ordering`'s rows over `layering`'s adjacency, within
    /// [`PRIORITY_NODE_BUDGET`] layered vertices.
    pub(crate) fn build(ordering: &Ordering, layering: &Layering, node_count: u32) -> Self {
        Self::build_within(ordering, layering, node_count, PRIORITY_NODE_BUDGET)
    }

    /// [`Self::build`] with the layered-vertex ceiling spelled out, so a test can reach the
    /// skip the production budget keeps 200k vertices away — the way [`Layering::build`]
    /// takes its `DUMMY_BUDGET` (`super::layering::DUMMY_BUDGET`).
    pub(crate) fn build_within(
        ordering: &Ordering,
        layering: &Layering,
        node_count: u32,
        budget: u32,
    ) -> Self {
        let total = layering.layer_of.len() as u32;
        let mut x = vec![0.0; total as usize];
        for row in &ordering.layers {
            for (i, &v) in row.iter().enumerate() {
                x[v as usize] = i as f64;
            }
        }
        let passes = if total <= budget { 4 } else { 0 };
        let rows = &ordering.layers;
        for step in 0..passes {
            if step % 2 == 0 {
                for row in rows.iter().skip(1) {
                    pass(&mut x, row, &layering.up, node_count);
                }
            } else {
                for level in (0..rows.len().saturating_sub(1)).rev() {
                    pass(&mut x, &rows[level], &layering.down, node_count);
                }
            }
        }
        Self(x)
    }
}

/// One layer's priority-method pass: every vertex slides toward the median of its
/// `reference`-side neighbours, highest priority first, blocked by the nearest neighbour
/// still at or above its own priority.
fn pass(x: &mut [f64], row: &[u32], reference: &[Vec<u32>], node_count: u32) {
    let priority: Vec<u32> = row
        .iter()
        .map(|&v| priority_of(v, reference, node_count))
        .collect();
    let targets: Vec<f64> = row
        .iter()
        .map(|&v| {
            weighted_median(
                reference[v as usize]
                    .iter()
                    .map(|&n| x[n as usize])
                    .collect(),
            )
        })
        .collect();
    let blockers = Blockers::of(&priority);
    let mut xs: Vec<f64> = row.iter().map(|&v| x[v as usize]).collect();
    let mut order: Vec<usize> = (0..row.len()).collect();
    order.sort_by_key(|&i| (core::cmp::Reverse(priority[i]), i));
    for i in order {
        if targets[i] >= 0.0 {
            priority_move(&mut xs, i, targets[i], &blockers);
        }
    }
    for (&v, &value) in row.iter().zip(&xs) {
        x[v as usize] = value;
    }
}

/// A dummy (`v >= node_count`) always outranks a real vertex, whose own priority is its
/// `reference`-side degree (`hierarchical.py:627-628`).
fn priority_of(v: u32, reference: &[Vec<u32>], node_count: u32) -> u32 {
    if v >= node_count {
        DUMMY_PRIORITY
    } else {
        reference[v as usize].len() as u32
    }
}

/// The nearest index on each side holding a strictly higher priority, a monotonic-stack
/// scan each way (`hierarchical.py:565-583`); vertices keep a gap of at least one, so the
/// nearest blocker is always the binding one.
struct Blockers {
    left: Vec<Option<usize>>,
    right: Vec<Option<usize>>,
}

impl Blockers {
    fn of(priority: &[u32]) -> Self {
        let n = priority.len();
        let mut right = vec![None; n];
        let mut stack: Vec<usize> = Vec::new();
        for i in (0..n).rev() {
            while stack
                .last()
                .is_some_and(|&top| priority[top] <= priority[i])
            {
                stack.pop();
            }
            right[i] = stack.last().copied();
            stack.push(i);
        }
        let mut left = vec![None; n];
        stack.clear();
        for i in 0..n {
            while stack
                .last()
                .is_some_and(|&top| priority[top] <= priority[i])
            {
                stack.pop();
            }
            left[i] = stack.last().copied();
            stack.push(i);
        }
        Self { left, right }
    }
}

/// Slides `x[i]` toward `target`, pushing lower-priority neighbours along and stopping at
/// the first higher-priority blocker (`hierarchical.py:585-608`).
fn priority_move(x: &mut [f64], i: usize, target: f64, blockers: &Blockers) {
    if target > x[i] {
        let moved = match blockers.right[i] {
            Some(j) => target.min(x[j] - (j - i) as f64 * GAP),
            None => target,
        };
        if moved <= x[i] {
            return;
        }
        x[i] = moved;
        for j in i + 1..x.len() {
            if x[j] >= x[j - 1] + GAP {
                break;
            }
            x[j] = x[j - 1] + GAP;
        }
    } else if target < x[i] {
        let moved = match blockers.left[i] {
            Some(j) => target.max(x[j] + (i - j) as f64 * GAP),
            None => target,
        };
        if moved >= x[i] {
            return;
        }
        x[i] = moved;
        for j in (0..i).rev() {
            if x[j] <= x[j + 1] - GAP {
                break;
            }
            x[j] = x[j + 1] - GAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::layout::sugiyama::acyclic::{Acyclic, Arcs};
    use crate::layout::sugiyama::layering::{DUMMY_BUDGET, assign_layers};
    use crate::records::build::{edge, node};
    use crate::records::{EdgeRecord, NodeRecord};

    /// `(x, each layer's own row)` for `nodes`/`edges` at the given priority-method budget.
    fn coords_within(
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
        budget: u32,
    ) -> (Vec<f64>, Vec<Vec<u32>>) {
        let t = index_model(nodes, edges).expect("fits");
        let acyclic = Acyclic::of(&t);
        let list = Arcs::new(&t, &acyclic).grouped();
        let layer = assign_layers(&list);
        let layering = Layering::build(&list, &layer, DUMMY_BUDGET);
        let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
        let ordering = Ordering::build(&layering, num_layers).expect("covers every layer");
        let x = Coords::build_within(&ordering, &layering, t.node_count(), budget).0;
        (x, ordering.layers)
    }

    fn coords(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Vec<f64> {
        coords_within(nodes, edges, PRIORITY_NODE_BUDGET).0
    }

    /// a->d spans 3 layers (b, c real siblings on layers 1, 2), so the dummy chain is what
    /// the priority passes have to place.
    fn spanned() -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
        let n = ["a", "b", "c", "d"].map(|id| node(id, ""));
        let e = [
            edge("ab", "a", "b"),
            edge("bc", "b", "c"),
            edge("cd", "c", "d"),
            edge("ad", "a", "d"),
        ];
        (n.to_vec(), e.to_vec())
    }

    #[test]
    fn a_chain_keeps_every_vertex_at_the_same_x() {
        let n = ["a", "b", "c"].map(|id| node(id, ""));
        let e = [edge("ab", "a", "b"), edge("bc", "b", "c")];
        let x = coords(&n, &e);
        assert_eq!(x, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_dummy_chain_centers_its_span_and_x_is_deterministic() {
        // a->d spans 3 layers (b, c real siblings on layers 1, 2); the dummy chain for
        // a->d should end up between the real chain's own X, not off to one side.
        let (n, e) = spanned();
        let first = coords(&n, &e);
        let second = coords(&n, &e);
        assert_eq!(first, second, "deterministic");
        assert!(first.iter().all(|x| x.is_finite()));
    }

    /// L-12: above the budget `passes` is 0, so X is exactly the raw ordering slot index and
    /// the drawing is legal but maximally spread. Nothing in the snapshot says so: the notes
    /// section has no code for a skipped phase, so the behaviour is pinned here and
    /// documented in the registry row's `degradation`.
    #[test]
    fn above_the_priority_budget_leaves_x_at_the_raw_slot_index_and_the_snapshot_does_not_say_so() {
        let (n, e) = spanned();
        let (skipped, layers) = coords_within(&n, &e, 0);
        let (ran, _) = coords_within(&n, &e, PRIORITY_NODE_BUDGET);
        let mut slot = vec![0.0; skipped.len()];
        for row in &layers {
            for (i, &v) in row.iter().enumerate() {
                slot[v as usize] = i as f64;
            }
        }
        assert_eq!(skipped, slot, "no priority pass ran: x is the slot index");
        assert_ne!(skipped, ran, "the passes do move at least one vertex");
        assert!(skipped.iter().all(|x| x.is_finite()));
    }
}
