//! The re-point onto p3's repaired [`Hierarchy`](crate::layout::hierarchy::Hierarchy),
//! over p3's own fixtures.
//!
//! `depth/tests.rs` pins the convention against a hand-built forest, on the grounds that
//! `layout/hierarchy.rs` "is not on this branch's base". It is on the base now, so this
//! module is where the claim the re-point exists to make is checked: **`bfs_depth` over a
//! real [`Hierarchy`] and p3's own `depth` column are the same function of the same
//! tree** — one convention, not two. A second derivation would agree on the hand-built
//! forest and could still disagree on exactly the inputs the repair rewrites: the
//! two-roots virtual root, a multi-parent node, a cycle broken at its lowest index.
//!
//! So the fixtures used here are the four that exercise those: `tree-balanced`,
//! `tree-degenerate`, `forest` (two roots, virtual root) and `cyclic` (a repaired
//! cycle). Every node of every one of them is compared, node by node, and the deepest
//! level is compared too.

use super::UNREACHED;
use super::bfs_depth;
use crate::index::index_model;
use crate::layout::hierarchy::Hierarchy;
use crate::layout::hierarchy::fixture::{FIXTURES, load};

/// `Hierarchy::of`'s only refusal is `n + 1` not fitting `u32`, which `index_model` cannot
/// have produced (its own capacity check runs first), so this panic is the same
/// caller-bug discipline `Csr::from_pairs` takes for an out-of-range row.
fn repaired(fixture: &str) -> (Hierarchy, u32) {
    let (nodes, edges) = load(fixture);
    let topology = index_model(&nodes, &edges).expect("the fixtures fit");
    let hierarchy = Hierarchy::of(&topology).expect("n + 1 fits u32");
    let n = topology.node_count();
    (hierarchy, n)
}

/// Every fixture, every node: `bfs_depth`'s column is p3's own `depth` column, value for
/// value, and the maximum agrees with it.
#[test]
fn depth_over_a_repaired_hierarchy_is_p3s_own_depth_column() {
    for (name, _) in FIXTURES {
        let (hierarchy, n) = repaired(name);
        let d = bfs_depth(&hierarchy);
        assert_eq!(d.levels().len(), n as usize, "{name}: one level per node");
        for v in 0..n {
            assert_eq!(
                d.of(v),
                hierarchy.depth(v),
                "{name}: node {v} is at the depth p3's own column gives it"
            );
        }
        assert_eq!(d.max(), hierarchy.max_depth(), "{name}: the deepest level");
    }
}

/// The re-point is what makes the two-roots case reachable at all, and it is the case
/// the hand-built forest's `virtual_root` was standing in for: p3 hangs two or more
/// roots off one virtual root at index `n`, so **every** real node sits one level below
/// where a single-rooted reading would put it. Pinning the offsets, not just the
/// agreement, is what stops a `Roots` impl that reported `virtual_root() == None` from
/// passing the comparison above by accident.
#[test]
fn two_or_more_roots_hang_off_the_virtual_root_and_the_real_roots_sit_at_depth_one() {
    let (hierarchy, n) = repaired("forest");
    assert!(
        hierarchy.roots().len() >= 2,
        "the forest fixture is the two-roots case"
    );
    assert_eq!(hierarchy.virtual_root(), Some(n), "the virtual root is n");
    let d = bfs_depth(&hierarchy);
    for &root in hierarchy.roots() {
        assert_eq!(d.of(root), 1, "root {root} hangs at depth 1");
    }
}

/// A repaired hierarchy reaches **every** node, so `UNREACHED` never appears in this
/// column — which is precisely the difference from p3's own column (an unreachable node
/// is 0 there). Stating it here is what makes `UNREACHED` a sentinel for a partial
/// forest rather than a value this re-point can produce.
#[test]
fn a_repaired_hierarchy_reaches_every_node_so_unreached_never_appears() {
    for (name, _) in FIXTURES {
        let (hierarchy, n) = repaired(name);
        let d = bfs_depth(&hierarchy);
        assert!(
            !d.levels().contains(&UNREACHED),
            "{name}: the repair makes every node reachable"
        );
        assert!(
            d.max() <= n.saturating_sub(1),
            "{name}: a depth below the node count is not a depth"
        );
    }
}

/// The cycle the repair breaks is a case the hand-built forest could not have covered:
/// `depth` walks `children`, and a parent-pointer cycle is not a `children` cycle, so
/// the walk terminates here exactly as it does in p3's own breadth-first pass. If the
/// re-point had re-derived roots instead of reading p3's, this is the fixture where the
/// two would part company.
#[test]
fn the_cyclic_fixture_terminates_and_agrees_with_the_repaired_column() {
    let (hierarchy, n) = repaired("cyclic");
    assert!(
        !hierarchy.notes().is_empty(),
        "the cyclic fixture is repaired, not silently accepted"
    );
    let d = bfs_depth(&hierarchy);
    for v in 0..n {
        assert_eq!(d.of(v), hierarchy.depth(v), "node {v}");
    }
}
