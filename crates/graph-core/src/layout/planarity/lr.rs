//! The Left-Right planarity test (Brandes 2009), ported from networkx 3.6's
//! `LRPlanarity` (`algorithms/planarity.py`): orient the graph by DFS
//! ([`orient`](self::orient)), test the orientation for a valid left-right partition
//! ([`testing`](self::testing)), then resolve each edge's absolute side
//! ([`sign`](self::sign)) so [`super::embed`] can lay out the rotation. [`test`] returns
//! `None` the moment any of that fails — the graph is not planar.
//!
//! Every recursive method in the reference has an iterative twin using an explicit
//! stack (`dfs_orientation` vs. `_recursive`); only the iterative ones are ported, so a
//! 100k-node path does not recurse 100k deep. An edge is addressed by its **slot** (an
//! [`Adjacency`] flat index) instead of the reference's `(v, w)` tuple, so every
//! per-edge table here is a plain `Vec` indexed by slot, never a hash map — the ports of
//! `self.lowpt`, `self.side`, and friends.

mod orient;
mod sign;
mod testing;

use super::adjacency::Adjacency;

/// One node's two half-edges (`Interval::empty()` when neither is set): the return
/// edges of one side of a conflict, `high` the topmost, `low` the lowest.
#[derive(Debug, Clone, Copy, Default)]
struct Interval {
    low: Option<u32>,
    high: Option<u32>,
}

impl Interval {
    const fn edge(slot: u32) -> Self {
        Self {
            low: Some(slot),
            high: Some(slot),
        }
    }

    fn is_empty(&self) -> bool {
        self.low.is_none() && self.high.is_none()
    }

    /// Whether this interval's return edges outrank `b`'s.
    fn conflicts_with(&self, b: u32, lowpt: &[u32]) -> bool {
        !self.is_empty()
            && lowpt[self.high.expect("non-empty has a high") as usize] > lowpt[b as usize]
    }
}

/// A stack entry of the LR test: left and right intervals that must land on opposite
/// sides. `id` stands in for the reference's object identity, so [`testing`] can tell
/// whether the stack has been unwound back to a remembered point.
#[derive(Debug, Clone, Copy, Default)]
struct ConflictPair {
    id: u32,
    left: Interval,
    right: Interval,
}

impl ConflictPair {
    fn back_edge(id: u32, slot: u32) -> Self {
        Self {
            id,
            left: Interval::default(),
            right: Interval::edge(slot),
        }
    }

    fn swap(&mut self) {
        core::mem::swap(&mut self.left, &mut self.right);
    }

    /// The lower of the two intervals' lowpoints; only ever called on a pair holding at
    /// least one edge.
    fn lowest(&self, lowpt: &[u32]) -> u32 {
        match (self.left.low, self.right.low) {
            (None, Some(r)) => lowpt[r as usize],
            (Some(l), None) => lowpt[l as usize],
            (Some(l), Some(r)) => lowpt[l as usize].min(lowpt[r as usize]),
            (None, None) => unreachable!("a conflict pair always holds a return edge"),
        }
    }
}

/// What the LR test hands [`super::embed`] once the graph is confirmed planar.
#[derive(Debug)]
pub(super) struct Sides {
    /// Every node's DG-out slots, sorted by signed nesting depth — the final rotation
    /// order [`super::embed`] starts each row from.
    pub(super) ordered: Vec<Vec<u32>>,
    /// A child's parent slot (in the parent's row), `None` for a root. [`super::embed`]
    /// recovers the parent node itself from this via `adjacency.slot_neighbour`, so the
    /// node is not duplicated here.
    pub(super) parent_edge: Vec<Option<u32>>,
    /// A slot's resolved side, `1` or `-1`; meaningless off an oriented slot.
    pub(super) side: Vec<i8>,
    /// Every DFS root, ascending dense index (discovery order over `0..n`).
    pub(super) roots: Vec<u32>,
}

/// Every per-slot and per-node table the three phases share, threaded through as one
/// value so no phase's method takes more than the house limit of parameters.
struct Lr<'a> {
    adjacency: &'a Adjacency,
    height: Vec<Option<u32>>,
    oriented: Vec<bool>,
    lowpt: Vec<u32>,
    lowpt2: Vec<u32>,
    nesting_depth: Vec<i32>,
    parent_edge: Vec<Option<u32>>,
    parent: Vec<Option<u32>>,
    roots: Vec<u32>,
    ind: Vec<u32>,
    skip_init: Vec<bool>,
    ordered: Vec<Vec<u32>>,
    stack: Vec<ConflictPair>,
    next_id: u32,
    stack_bottom: Vec<Option<u32>>,
    lowpt_edge: Vec<Option<u32>>,
    reference: Vec<Option<u32>>,
    side: Vec<i8>,
    old_reference: Vec<Option<u32>>,
    resolved: Vec<bool>,
}

impl<'a> Lr<'a> {
    fn new(adjacency: &'a Adjacency) -> Self {
        let n = adjacency.node_count() as usize;
        let slots = adjacency.total_slots() as usize;
        Self {
            adjacency,
            height: vec![None; n],
            oriented: vec![false; slots],
            lowpt: vec![0; slots],
            lowpt2: vec![0; slots],
            nesting_depth: vec![0; slots],
            parent_edge: vec![None; n],
            parent: vec![None; n],
            roots: Vec::new(),
            ind: vec![0; n],
            skip_init: vec![false; slots],
            ordered: vec![Vec::new(); n],
            stack: Vec::new(),
            next_id: 0,
            stack_bottom: vec![None; slots],
            lowpt_edge: vec![None; slots],
            reference: vec![None; slots],
            side: vec![1; slots],
            old_reference: vec![None; slots],
            resolved: vec![false; slots],
        }
    }

    fn fresh_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn top_id(&self) -> Option<u32> {
        self.stack.last().map(|p| p.id)
    }
}

/// A graph denser than the maximum a simple planar graph can have (`m > 3n - 6` for
/// `n > 2`) is rejected without running the test — a cheap, exact necessary condition,
/// not a heuristic.
fn too_dense(n: u32, m: u32) -> bool {
    n > 2 && u64::from(m) > 3 * u64::from(n) - 6
}

/// Runs the LR planarity test. `None` means `adjacency` is not planar.
pub(super) fn test(adjacency: &Adjacency) -> Option<Sides> {
    if too_dense(adjacency.node_count(), adjacency.edge_count()) {
        return None;
    }
    let mut lr = Lr::new(adjacency);
    lr.orient_all();
    lr.order_by_nesting_depth();
    if !lr.test_all() {
        return None;
    }
    lr.resolve_signs();
    lr.order_by_nesting_depth();
    Some(Sides {
        ordered: lr.ordered,
        parent_edge: lr.parent_edge,
        side: lr.side,
        roots: lr.roots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval_pair(id: u32, l: (u32, u32), r: (u32, u32)) -> ConflictPair {
        ConflictPair {
            id,
            left: Interval {
                low: Some(l.0),
                high: Some(l.1),
            },
            right: Interval {
                low: Some(r.0),
                high: Some(r.1),
            },
        }
    }

    #[test]
    fn swap_exchanges_left_and_right() {
        let mut p = interval_pair(0, (0, 1), (2, 3));
        p.swap();
        assert_eq!((p.left.low, p.right.low), (Some(2), Some(0)));
    }

    #[test]
    fn conflicts_with_compares_lowpt_of_the_high_edge() {
        let lowpt = [5u32, 1, 9];
        let full = Interval {
            low: Some(0),
            high: Some(2),
        };
        assert!(full.conflicts_with(0, &lowpt)); // lowpt[2]=9 > lowpt[0]=5
        assert!(!full.conflicts_with(2, &lowpt)); // lowpt[2]=9 > lowpt[2]=9 is false
        assert!(!Interval::default().conflicts_with(0, &lowpt));
    }

    #[test]
    fn lowest_picks_whichever_side_is_present() {
        let lowpt = [3u32, 7];
        let left_only = ConflictPair {
            id: 0,
            left: Interval::edge(0),
            right: Interval::default(),
        };
        assert_eq!(left_only.lowest(&lowpt), 3);
        let both = interval_pair(0, (0, 0), (1, 1));
        assert_eq!(both.lowest(&lowpt), 3);
    }

    #[test]
    fn too_dense_rejects_only_past_the_planar_edge_bound() {
        assert!(!too_dense(2, 100)); // n <= 2 never rejected here
        assert!(!too_dense(4, 6)); // 3*4-6 = 6, exactly at the bound
        assert!(too_dense(4, 7));
    }
}
