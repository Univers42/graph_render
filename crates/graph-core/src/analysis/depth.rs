//! BFS depth (`prompts/phase-07-analysis.md` step 6): the depth of every node below the
//! root of a hierarchy, by breadth first. Exact — no `Ponytail` owed.
//!
//! **The interface is deliberately the smallest thing that could be re-pointed.** Step 6
//! says "reusing Phase 3's `hierarchy.rs` root/forest logic. One convention across the
//! codebase, not two". The re-point has happened: p3's
//! [`layout::hierarchy::Hierarchy`](crate::layout::hierarchy::Hierarchy) implements
//! [`Roots`] below by delegation and nothing else, so the two are one convention with
//! two names. This module derives no roots, cycles or parents of its own: it reads the
//! convention through [`Roots`], whose four methods are p3's `Hierarchy` accessors
//! verbatim — `node_count`, `roots`, `virtual_root`, `children`. What it does own is a
//! check of that convention ([`bfs_depth`] refuses an out-of-range root and two roots
//! with no virtual root) and the root offset, read with p3's own `roots.len() >= 2`.
//!
//! **The re-point has happened.** p3's [`Hierarchy`](crate::layout::hierarchy::Hierarchy)
//! implements [`Roots`] below, by delegation and nothing else, so the two are one
//! convention with two names:
//!
//! ```ignore
//! let hierarchy = crate::layout::hierarchy::Hierarchy::of(&topology)?;
//! let d = depth::bfs_depth(&hierarchy);
//! ```
//!
//! (`ignore`, not a compiled example: a doctest is an external crate, so `crate::` does
//! not resolve into this one.)
//!
//! Re-deriving roots, breaking cycles or dropping extra parents here would be the second
//! convention step 6 forbids, and it would be a *silent* one: the two would disagree on
//! the exact inputs where p3's repair records a note. That call is not evidence, so
//! neither is the example: `depth/hierarchy.rs` compares this walk against p3's own
//! `depth` column node by node over p3's four fixtures, and graph-cli's
//! `capabilities::tests::depth` writes the same call out as a type bound, so undoing the
//! re-point breaks a build rather than leaving a row that describes a function which no
//! longer exists.
//!
//! # The convention, as p3 states it (`docs/decisions/hierarchy-repair.md`, D-H)
//!
//! - One root: that root is at depth 0 and the tree counts out from it.
//! - Two or more roots: they hang, ascending, off one hidden **virtual root** at dense
//!   index `n`, so every real root is at depth **1**. The virtual root is an index, never
//!   a node, so it gets no column entry.
//! - No nodes: no levels, and a [`Depth::max`] of 0.
//! - A node no root reaches is [`UNREACHED`], which is deliberately *not* 0 — p3's own
//!   `Hierarchy::depth` leaves an unreachable node at 0, but that column is only ever read
//!   on a repaired tree where nothing is unreachable, while this one is an analysis
//!   result that a frontend may colour by, and 0 would render an orphan as a root.
//!
//! [`Depth::max`] is the deepest level **reached**, so a forest whose only reached node is
//! a bare root reports 0.
//!
//! Determinism: a FIFO queue seeded in the given root order, children in the order the
//! source lists them, every node written once on the first (hence shortest, hence
//! uniquely determined) visit. No `HashMap`, no tie-break to arbitrate — every output
//! entry has exactly one value that satisfies "shortest path from a root", so the
//! traversal order cannot change the answer (`Depth` is `Eq` and the tests pin it).

use crate::layout::hierarchy::Hierarchy;

/// The depth of a node no root reaches. `u32::MAX` leaves room for any real depth and
/// cannot be confused with one.
pub const UNREACHED: u32 = u32::MAX;

/// [`bfs_depth`]'s analysis id — the one graph-wasm registers it under and the hash gate
/// hashes it as. Lives here, not in the registry, for `components::WEAK`'s reason: a
/// caller naming `analysis.depth.bfs` is asking for *this* walk over `Roots`.
pub const BFS: &str = "analysis.depth.bfs";

/// The root/forest convention [`bfs_depth`] reads, and nothing else. A [`Roots`] must
/// satisfy: `roots` is every node with no parent, ascending; `virtual_root` is `Some(n)`
/// — the virtual root's dense index — exactly when there are two or more roots, and
/// `None` when there is at most one. `node_count` never counts the virtual root, and
/// `children` is asked only for real nodes `0..n`, so a source never has to be the
/// virtual root. `children` is ascending within a node.
///
/// A child index `>= node_count` is a caller bug and panics, exactly as
/// [`crate::Csr::from_pairs`] does for a row out of range.
pub trait Roots {
    /// Real nodes, `n`. The virtual root, if any, is index `n` and is not one of them.
    fn node_count(&self) -> u32;

    /// Every node with no parent, ascending dense index. Empty only when there is no
    /// node.
    fn roots(&self) -> &[u32];

    /// The virtual root's dense index, when there are two or more roots; `None` when
    /// there is at most one.
    fn virtual_root(&self) -> Option<u32>;

    /// Node `v`'s children, ascending, for a real node `v < node_count`.
    fn children(&self, v: u32) -> &[u32];
}

/// The re-point: p3's repaired [`Hierarchy`](crate::layout::hierarchy::Hierarchy) *is*
/// the root/forest convention, so it reads as a [`Roots`] by delegation. The four
/// methods are that type's own accessors called through their inherent path, never
/// through this trait's — an inherent method wins method resolution, so spelling it out
/// is what stops a future removal of one of the four from silently turning
/// `self.node_count()` into infinite recursion instead of a compile error.
impl Roots for Hierarchy {
    fn node_count(&self) -> u32 {
        Hierarchy::node_count(self)
    }

    fn roots(&self) -> &[u32] {
        Hierarchy::roots(self)
    }

    fn virtual_root(&self) -> Option<u32> {
        Hierarchy::virtual_root(self)
    }

    fn children(&self, v: u32) -> &[u32] {
        Hierarchy::children(self, v)
    }
}

/// One depth per node, indexed by dense index. The analysis column step 7 wants
/// (`depth: Vec<u32>`), still a struct so the sentinel and the maximum travel with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Depth {
    levels: Vec<u32>,
    max: u32,
}

impl Depth {
    /// One entry per node, `0..node_count`, in dense-index order.
    pub fn levels(&self) -> &[u32] {
        &self.levels
    }

    /// Node `v`'s depth, or [`UNREACHED`]. Panics on an index past the last node.
    pub fn of(&self, v: u32) -> u32 {
        self.levels[v as usize]
    }

    /// Whether no root reaches node `v`: test this before using a level as a ring or
    /// row index, since [`UNREACHED`] is not a depth. Panics as [`Depth::of`] does.
    pub fn is_unreached(&self, v: u32) -> bool {
        self.of(v) == UNREACHED
    }

    /// The deepest level reached; 0 when nothing is reached.
    pub fn max(&self) -> u32 {
        self.max
    }
}

/// Depth from the forest's **detected** roots, following p3's convention exactly: one
/// root at depth 0, two or more under the virtual root at depth 1.
pub fn bfs_depth<R: Roots + ?Sized>(forest: &R) -> Depth {
    let roots = forest.roots();
    check_roots(forest, roots);
    // A release `assert!`, like the index checks: a source that drops p3's virtual root
    // would otherwise get a column one level short on every node.
    assert!(
        roots.len() < 2 || forest.virtual_root().is_some(),
        "two or more roots must hang off a virtual root (D-H step 6)"
    );
    // The virtual root is index `n` and has no column of its own, so the walk starts
    // from the real roots at the depth the virtual root would have given them — depth 1
    // under one, depth 0 when the single root is the tree root. Same column p3's own
    // `breadth_first` produces, without indexing a column that does not exist. The
    // `roots.len() >= 2` clause is p3's own test (`layout/hierarchy.rs`, `root` and
    // `virtual_root`), so a lone root named with a stray virtual root still sits at 0.
    let under_virtual_root = forest.virtual_root().is_some() && roots.len() >= 2;
    walk(forest, roots, u32::from(under_virtual_root))
}

/// Depth from **declared** roots: each of `roots` is at depth 0 whatever the forest's
/// own roots are, and a node several of them reach takes the shortest. A declared root
/// that descends from another declared root is still 0 — declaring is absolute, not
/// relative. An index `>= node_count` is a caller bug and panics.
pub fn depth_from<R: Roots + ?Sized>(forest: &R, roots: &[u32]) -> Depth {
    check_roots(forest, roots);
    walk(forest, roots, 0)
}

/// Refuses a root `>= node_count` by name, detected or declared alike.
fn check_roots<R: Roots + ?Sized>(forest: &R, roots: &[u32]) {
    let n = forest.node_count();
    for &root in roots {
        assert!(root < n, "root {root} of {n}");
    }
}

/// The breadth-first walk itself, from `sources`, all seeded at `seed`.
fn walk<R: Roots + ?Sized>(forest: &R, sources: &[u32], seed: u32) -> Depth {
    let n = forest.node_count();
    let mut levels = vec![UNREACHED; n as usize];
    let mut queue: Vec<u32> = Vec::with_capacity(n as usize);
    for &source in sources {
        claim(&mut levels, &mut queue, source, seed);
    }
    let mut at = 0;
    while let Some(&v) = queue.get(at) {
        for &child in forest.children(v) {
            assert!(child < n, "child {child} of node {v} is past node {n}");
            let below = levels[v as usize] + 1;
            claim(&mut levels, &mut queue, child, below);
        }
        at += 1;
    }
    let max = levels
        .iter()
        .filter(|&&level| level != UNREACHED)
        .copied()
        .max()
        .unwrap_or(0);
    Depth { levels, max }
}

/// Writes `level` for `v` and queues it, unless it already carries a depth — the first
/// visit in a FIFO breadth-first walk is the shortest one, so a later visit can only be
/// equal or worse. This is also what makes a cyclic `children` terminate.
fn claim(levels: &mut [u32], queue: &mut Vec<u32>, v: u32, level: u32) {
    let slot = &mut levels[v as usize];
    if *slot == UNREACHED {
        *slot = level;
        queue.push(v);
    }
}

// The ABI-face test of the `impl Roots for Hierarchy` above, so it sits here and not in
// `tests.rs`, whose `Forest` double pins the convention over structures carrying no
// repair at all (a cycle to break, a second parent to drop) rather than over a real one.
#[cfg(test)]
mod repointed {
    use super::*;
    use crate::edgekind::{EdgeKind, child_first_from_type};
    use crate::index::index_model;
    use crate::records::EdgeRecord;
    use crate::records::build::{edge, node};

    /// BFS depth of a **real** [`Hierarchy`], with no adapter in between: this pins the
    /// type graph-wasm now hands to `bfs_depth` directly. That is the re-point step 6 of
    /// `prompts/phase-07-analysis.md`, and the impl it asks for is the one written above.
    ///
    /// The fixture is the two-root forest the ABI face pins — `a` above `b` above `c`,
    /// plus an isolated `z` — and the expected column is the one that face pins, `[1, 2, 3,
    /// 1]`, restated rather than recomputed. That is the point: the value does not change,
    /// the code producing it does, and a test that moved its expectation along with the code
    /// would agree with anything.
    #[test]
    fn a_real_hierarchy_is_the_roots_depth_reads() {
        let nodes: Vec<_> = ["a", "b", "c", "z"].iter().map(|id| node(id, "")).collect();
        let edges = [tree("t0", "a", "b"), tree("t1", "b", "c")];
        let topology = index_model(&nodes, &edges).expect("four nodes fit");
        let hierarchy = Hierarchy::of(&topology).expect("n + 1 fits");
        assert_eq!(
            hierarchy.roots(),
            [0, 3],
            "two roots: `a` and the isolated `z`"
        );
        let d = bfs_depth(&hierarchy);
        assert_eq!(d.levels(), [1, 2, 3, 1], "the pinned column, unchanged");
        assert_eq!(d.max(), 3, "the deepest level reached");
    }

    /// A `hierarchy` edge with the parent as source, which is what `Hierarchy`'s CSR
    /// reads.
    fn tree(id: &str, source: &str, target: &str) -> EdgeRecord {
        EdgeRecord {
            kind: EdgeKind::Hierarchy,
            child_first: child_first_from_type(Some("parent_of")),
            label: "parent_of".into(),
            ..edge(id, source, target)
        }
    }
}

// House limit: <=300 lines per file. Split the same way `centrality.rs` is
// (`analysis/centrality/tests.rs`), rather than let a growing test module push this
// file over.
#[cfg(test)]
mod tests;

/// The re-point onto p3's repaired `layout::hierarchy::Hierarchy`, which *is* on the
/// base now: the merge step this module's own doc has been waiting for, and the one
/// convention p3 and this stage must share rather than derive twice.
#[cfg(test)]
mod hierarchy;

/// The refusals and the sentinel predicate, split out because `tests.rs` is at the
/// house's 300-line limit.
#[cfg(test)]
mod guards;
