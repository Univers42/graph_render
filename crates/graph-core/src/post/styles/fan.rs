//! The parallel fan: which offset each edge takes so that two edges between the same
//! pair of nodes never overlay. Split from the parent for the house line limit; the
//! parent's module doc carries the reasoning, and this file only carries the
//! arithmetic.
//!
//! Grouping is by **unordered** node pair — `(min, max)` of the two dense indices, so
//! `a -> b` and `b -> a` are one group, exactly as SciGraphs'
//! `identify_parallel_edges` keys it. Within a group of `k` edges, ascending edge index
//! `i` takes `base * (i - (k-1) / 2)`: centred on the chord, so a group of one takes
//! zero and a group of two straddles it.
//!
//! Two chained stable counting sorts, never a map (D4) and never an unstable sort (D5).
//! The first orders the edges by the **higher** endpoint, the second orders that by the
//! **lower**, so walking the second CSR by row and then by arrival order visits every
//! edge in `(lower, higher, edge index)` order: ascending, total, and the same on every
//! target. Within a group, arrival order is ascending edge index because both counting
//! sorts are stable and the input arrives in edge order. `O(n + m)`.

use crate::arena::CapacityError;
use crate::csr::Csr;
use crate::index::Topology;

/// Every edge's fan offset, in edge order.
pub(super) fn fan(t: &Topology, base: f64) -> Result<Vec<f64>, CapacityError> {
    let (n, m) = (t.node_count(), t.edge_count());
    let by_high = Csr::from_pairs(n, (0..m).map(|e| (pair(t, e).1, e)))?;
    let mut pairs = Vec::with_capacity(m as usize);
    for high in 0..n {
        pairs.extend(by_high.row(high).iter().map(|&e| (pair(t, e).0, e)));
    }
    let by_low = Csr::from_pairs(n, pairs.into_iter())?;
    let mut amounts = vec![0.0f64; m as usize];
    for low in 0..n {
        fan_row(by_low.row(low), t, base, &mut amounts);
    }
    Ok(amounts)
}

/// One row of the second counting sort: the edges of one lower endpoint, already
/// grouped by their higher endpoint, each group in ascending edge index. Every run of
/// equal higher endpoint is one parallel group, and the run's own order is the fan's
/// edge index, so a run of `k` fills `base * (i - (k-1) / 2)` for `i` in `0..k`.
fn fan_row(row: &[u32], t: &Topology, base: f64, amounts: &mut [f64]) {
    let mut start = 0;
    while start < row.len() {
        let high = pair(t, row[start]).1;
        let mut end = start;
        while end < row.len() && pair(t, row[end]).1 == high {
            end += 1;
        }
        let span = f64::from((end - start) as u32 - 1) * 0.5;
        for (i, &e) in row[start..end].iter().enumerate() {
            amounts[e as usize] = base * (f64::from(i as u32) - span);
        }
        start = end;
    }
}

/// Edge `e`'s endpoints as `(lower, higher)` dense index — the parallel group's key,
/// direction ignored.
fn pair(t: &Topology, e: u32) -> (u32, u32) {
    let (s, g) = (t.edges().source[e as usize], t.edges().target[e as usize]);
    (s.min(g), s.max(g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edgekind::EdgeKind;
    use crate::stage::seeded_model;
    use crate::weights::REFERENCE_DEGREE;
    use crate::{EdgeRecord, NodeKind, NodeRecord, index_model};

    fn node(id: &str) -> NodeRecord {
        NodeRecord {
            kind: NodeKind::Record,
            database_id: None,
            source: "test".into(),
            label: id.into(),
            group: None,
            weight: 1.0,
            version: 0.0,
            has_note: false,
            icon: None,
            id: id.into(),
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> EdgeRecord {
        EdgeRecord {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            kind: EdgeKind::Relation,
            child_first: false,
            label: "relates_to".into(),
            strength: 1.0,
            directed: true,
            record_id: None,
        }
    }

    /// A topology over node ids `a`, `b`, `c`, `d` — declared in that order, so the
    /// dense indices are 0, 1, 2, 3 and "the lower endpoint" is a testable thing.
    fn four(edges: &[(&str, &str, &str)]) -> Topology {
        let nodes = ["a", "b", "c", "d"].map(node);
        let records: Vec<EdgeRecord> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
        index_model(&nodes, &records).expect("fits")
    }

    #[test]
    fn a_group_of_one_takes_no_offset_and_a_group_of_four_straddles_the_chord() {
        // (a,b) x4 — directions mixed, so a fan keyed on direction would split this.
        let t = four(&[
            ("e0", "a", "b"),
            ("e1", "b", "a"),
            ("e2", "a", "b"),
            ("e3", "b", "a"),
            ("e4", "a", "c"),
            ("e5", "a", "c"),
            ("e6", "c", "c"),
        ]);
        let got = fan(&t, 2.0).expect("fits");
        assert_eq!(got, [-3.0, -1.0, 1.0, 3.0, -1.0, 1.0, 0.0]);
    }

    #[test]
    fn an_interleaved_group_is_still_one_group() {
        // (a,b), (a,c), (a,b) — the two (a,b) edges are not contiguous, so a fan that
        // read each endpoint's edges as one run would call them two groups of one and
        // give both zero. Only the second counting sort, which orders a row by the higher
        // endpoint before the row is split into runs, puts them in the same run.
        let t = four(&[("e0", "a", "b"), ("e1", "a", "c"), ("e2", "a", "b")]);
        assert_eq!(fan(&t, 2.0).expect("fits"), [-1.0, 0.0, 1.0]);
        // Three groups, fully interleaved: (a,b), (a,c), (a,d), (a,b), (a,c), (a,d).
        let t = four(&[
            ("e0", "a", "b"),
            ("e1", "a", "c"),
            ("e2", "a", "d"),
            ("e3", "a", "b"),
            ("e4", "a", "c"),
            ("e5", "a", "d"),
        ]);
        assert_eq!(
            fan(&t, 4.0).expect("fits"),
            [-2.0, -2.0, -2.0, 2.0, 2.0, 2.0]
        );
    }

    #[test]
    fn a_self_loop_is_a_group_of_its_own_and_two_of_them_separate() {
        let t = four(&[("e0", "c", "c"), ("e1", "c", "c"), ("e2", "c", "c")]);
        assert_eq!(fan(&t, 1.0).expect("fits"), [-1.0, 0.0, 1.0]);
    }

    #[test]
    fn no_edges_is_an_empty_fan_not_a_single_zero() {
        let t = four(&[]);
        assert_eq!(fan(&t, 1.0).expect("fits"), Vec::<f64>::new());
    }

    #[test]
    fn the_fan_is_pure_and_lands_on_the_spacing() {
        let (nodes, edges) = seeded_model(9, 40, REFERENCE_DEGREE);
        let t = index_model(&nodes, &edges).expect("fits");
        let once = fan(&t, 0.5).expect("fits");
        let again = fan(&t, 0.5).expect("fits");
        assert_eq!(once, again, "pure: the same graph gives the same fan");
        // Every edge's offset is a whole or half whole multiple of the base, which is
        // what a fan grouped any other way would fail. 0.5 is exact in binary, so this
        // is an exact statement and not a tolerance.
        for &amount in &once {
            let steps = amount / 0.5;
            assert!(
                steps == steps.round() || (steps - steps.round()).abs() == 0.5,
                "{amount} is off the spacing"
            );
        }
    }
}
