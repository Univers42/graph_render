//! The review's lod findings (`docs/reviews/review-core-post.md` R4, R22, M31, M33), one
//! test each; `tests.rs` is at the file limit.

use super::tests::{ring, viewport};
use super::*;

/// R4. The reference, `SciGraphs/engine/scigraphs_engine/lod.py:76-79`: "A ``budget`` of
/// 0 or less means no limit." So a budget of 0 labels every visible node.
#[test]
fn a_label_budget_of_zero_means_no_limit() {
    let (t, x, y) = ring();
    let params = LodParams {
        viewport: viewport(),
        label_budget: 0,
        ..LodParams::default()
    };
    let hints = hints(&t, &x, &y, &params);
    assert_eq!(hints.tier, Tier::Full);
    assert_eq!(
        hints.labelled, hints.visible,
        "every visible node is labelled"
    );
}

/// R22. The rank reads each visible node's degree once: a comparison sort that re-reads
/// it per comparison costs `O(k log k)` reads against the row's `O(n + m)`.
#[test]
fn the_label_rank_reads_each_degree_once_and_orders_by_degree_then_index() {
    let n = 1_000_u32;
    let visible: Vec<u8> = (0..n).map(|i| u8::from(i % 5 != 0)).collect();
    let mut reads = 0_u32;
    let order = rank_by_degree(&visible, |i| {
        reads += 1;
        (i * 7) % 13
    });
    assert_eq!(reads, 800, "one read per visible node");
    let mut want: Vec<u32> = (0..n).filter(|i| i % 5 != 0).collect();
    want.sort_by_key(|&i| (std::cmp::Reverse((i * 7) % 13), i));
    assert_eq!(order, want);
}

/// M33. A NaN viewport field is no bound on its side, the same mask as an infinite one,
/// and a NaN radius culls nothing: the hints are advisory, so a corrupt viewport may
/// over-draw but never hides a node. Before, every NaN comparison was false and the mask
/// was all zero.
#[test]
fn a_nan_viewport_field_culls_nothing_on_its_side() {
    let (t, x, y) = ring();
    let mask = |viewport: Viewport| {
        let params = LodParams {
            viewport,
            ..LodParams::default()
        };
        hints(&t, &x, &y, &params).visible
    };
    let open_left = mask(Viewport {
        x0: f64::NEG_INFINITY,
        ..viewport()
    });
    assert_eq!(open_left, vec![0, 0, 1, 1, 1, 1, 1, 0]);
    let nan_left = mask(Viewport {
        x0: f64::NAN,
        ..viewport()
    });
    assert_eq!(nan_left, open_left, "NaN left edge reads as no left edge");
    let nan_radius = mask(Viewport {
        radius: f64::NAN,
        ..viewport()
    });
    assert_eq!(nan_radius, vec![1; 8], "NaN radius culls nothing");
}

/// M31, recorded false. The rank's degree counts a self-loop twice, as the topology's own
/// `incident` does (`index/view.rs:116-117`) and as networkx does
/// (`networkx/classes/reportviews.py:526`, `len(nbrs) + (n in nbrs)`). `Simple::degree`
/// counts distinct neighbours for the simplification passes: another quantity.
#[test]
fn the_rank_degree_counts_a_self_loop_twice_like_the_incident_list() {
    use crate::index::index_model;
    use crate::records::build::{edge, node};
    let nodes = [node("a", "db"), node("b", "db"), node("c", "db")];
    let edges = [
        edge("loop", "a", "a"),
        edge("e0", "b", "c"),
        edge("e1", "b", "c"),
    ];
    let t = index_model(&nodes, &edges).expect("fits");
    for v in 0..3 {
        assert_eq!(degree_of(&t, v) as usize, t.incident(v).count(), "node {v}");
    }
    assert_eq!(
        degree_of(&t, 0),
        2,
        "networkx: 1 neighbour + 1 for the loop"
    );
}
