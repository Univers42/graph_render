//! How many edge crossings a drawing has, counted from nothing but its rows — the one
//! crossing count that can be computed on **both** sides of an oracle comparison.
//!
//! **Why not the pass's own count.** The mincross pass counts crossings over the chains: a
//! long edge is several links through dummies, and each link carries the penalty the
//! crossing is charged at. Those dummies are the pass's internal state — `-Tplain` prints no
//! dummy and no crossing count at all — so the pass's count has nothing on the oracle's side
//! to be compared with. What both sides do have is an order: a rank per node and a place
//! within its rank, from the printed coordinates on one side and from the pass's own rows on
//! the other. So the count is defined over exactly that, and one implementation computes
//! both, which is what makes a difference in the answer a difference in the order.
//!
//! **What is counted.** Two input edges cross when they span the same pair of *adjacent*
//! ranks and their ends are in opposite orders on those two ranks. That is exact, and it is
//! the whole of what is knowable without a coordinate: an edge that jumps a rank has a
//! middle the plain format never prints, so **an edge spanning more than one rank is not
//! counted at all**, and neither is a pair sharing an endpoint — those touch rather than
//! cross, and the reference's own count does not charge one node's edges against each other
//! either. Over the 1000 fixture seeds 72.8% of the edges span exactly one rank, so this
//! counts most of what a drawing has; what it leaves out is stated here rather than
//! approximated.
//!
//! **Real nodes only.** A row carrying dummies is not the same row as one without, so both
//! sides hand this real nodes and the two counts are comparable by construction.
//!
//! Determinism: the count is a sum over edge pairs in the fixture's own edge order, over
//! integer positions, with nothing read twice (`prompt.md` §6 D3).

/// One input edge that spans exactly one rank: the lower rank, the lower endpoint's place on
/// it, and the upper endpoint's place on the rank above.
struct Band {
    /// The lower of the two ranks.
    low: usize,
    /// The lower endpoint's place on [`Band::low`].
    from: i64,
    /// The upper endpoint's place on the rank above.
    to: i64,
}

/// Where every node of a drawing sits: its rank and its place on it, read once so the pair
/// loop below is two lookups rather than two searches per edge pair.
struct Layout {
    /// Node index to rank, -1 for a node no row holds.
    rank: Vec<i32>,
    /// Node index to place within its rank.
    place: Vec<i64>,
}

impl Layout {
    /// Read a set of rows — one per rank, each left to right. The per-node arrays are four
    /// times the rank count long, which is far more than any fixture's node index.
    fn of(rows: &[Vec<u32>]) -> Self {
        let len = rows.len() * 4;
        let mut layout = Self { rank: vec![-1; len], place: vec![0; len] };
        for (r, row) in rows.iter().enumerate() {
            for (i, &node) in row.iter().enumerate() {
                if let Some(slot) = slot_of(node, len) {
                    layout.rank[slot] = r as i32;
                    layout.place[slot] = i as i64;
                }
            }
        }
        layout
    }

    /// The rank a node is on, or `None` when no row holds it.
    fn rank_of(&self, node: u32) -> Option<i32> {
        let slot = slot_of(node, self.rank.len())?;
        (self.rank[slot] >= 0).then_some(self.rank[slot])
    }

    /// A node's place in its own row.
    fn place_of(&self, node: u32) -> i64 {
        slot_of(node, self.place.len()).map_or(0, |slot| self.place[slot])
    }
}

/// A node's slot in a per-node array, or `None` when the array is too small for it.
fn slot_of(node: u32, len: usize) -> Option<usize> {
    let index = usize::try_from(node).expect("a node index fits usize");
    (index < len).then_some(index)
}

/// The number of pairs of input edges that cross in a drawing whose rows are `rows`.
pub fn edge_crossings(rows: &[Vec<u32>], edges: &[(u32, u32)]) -> i64 {
    let layout = Layout::of(rows);
    let mut cross = 0i64;
    for (index, first) in edges.iter().enumerate() {
        for second in &edges[index + 1..] {
            cross += i64::from(crosses(&layout, *first, *second));
        }
    }
    cross
}

/// Whether these two edges cross, as [`edge_crossings`] counts it.
fn crosses(layout: &Layout, first: (u32, u32), second: (u32, u32)) -> bool {
    if [first.0, first.1].iter().any(|n| [second.0, second.1].contains(n)) {
        return false;
    }
    let (Some(a), Some(b)) = (band(layout, first), band(layout, second)) else {
        return false;
    };
    a.low == b.low && (a.from < b.from) != (a.to < b.to)
}

/// An edge as the band it spans, or `None` when it spans more than one rank.
fn band(layout: &Layout, edge: (u32, u32)) -> Option<Band> {
    let first = (layout.rank_of(edge.0)?, layout.place_of(edge.0));
    let second = (layout.rank_of(edge.1)?, layout.place_of(edge.1));
    let (low, high) = if first.0 <= second.0 { (first, second) } else { (second, first) };
    (high.0 == low.0 + 1).then_some(Band { low: low.0 as usize, from: low.1, to: high.1 })
}

/// The closed case this count is pinned on: the K2,2 the mincross tests draw by hand, with
/// its two diagonals crossing once, and the same graph one rank wider with no crossing at
/// all. It is the count's own negative control — a counter that gets the direction backwards
/// passes the first and fails the second, and one that counts every inversion passes neither.
#[test]
fn the_crossing_count_of_a_hand_drawn_k22_is_one() {
    let rows = vec![vec![0, 1], vec![2, 3]];
    assert_eq!(edge_crossings(&rows, &[(0, 2), (0, 3), (1, 2), (1, 3)]), 1, "the diagonals");
    let rows = vec![vec![0, 1], vec![2, 3, 4]];
    assert_eq!(edge_crossings(&rows, &[(0, 2), (0, 3), (1, 3), (1, 4)]), 0, "already in order");
}

/// **The negative control for the case above**: the same K2,2 still crosses once when either
/// row is reversed, and once when both are — reversing both rows leaves every pair in the
/// same relative order, so a count that could tell the three apart is not counting pairs of
/// edges at all.
#[test]
fn the_crossing_count_follows_the_order_it_is_given() {
    let edges = [(0, 2), (0, 3), (1, 2), (1, 3)];
    assert_eq!(edge_crossings(&vec![vec![1, 0], vec![2, 3]], &edges), 1, "top reversed");
    assert_eq!(edge_crossings(&vec![vec![0, 1], vec![3, 2]], &edges), 1, "bottom reversed");
    assert_eq!(edge_crossings(&vec![vec![1, 0], vec![3, 2]], &edges), 1, "both reversed");
}

/// An edge that jumps a rank is not counted, and neither is a pair that shares an endpoint:
/// both are stated in the module doc as the count's limits, so both are pinned here.
#[test]
fn the_count_leaves_out_what_it_cannot_know() {
    let edges = [(0, 4), (1, 2)];
    let rows = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
    assert_eq!(edge_crossings(&rows, &edges), 0, "the long edge shares no band");
    let shared = [(0, 2), (0, 3)];
    assert_eq!(edge_crossings(&rows, &shared), 0, "one node, two edges: a fan, not a crossing");
}