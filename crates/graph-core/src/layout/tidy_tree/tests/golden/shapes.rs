//! The ordinary golden shapes: a chain, a fan, an unbalanced tree, a
//! caterpillar, and the two single-root fixtures. See the module doc for why each is here.

use super::Row;

/// The deep chain: one column, `ky = 1/7`, every `x` the same. The simplest shape that
/// still walks the full depth, so it pins `ky`'s `1.0 / depth(bottom)` and the `y` column
/// alone.
pub(crate) const CHAIN8: [Row; 8] = [
    ("n0", 0x3f00_0000, 0x0000_0000),
    ("n1", 0x3f00_0000, 0x3e12_4925),
    ("n2", 0x3f00_0000, 0x3e92_4925),
    ("n3", 0x3f00_0000, 0x3edb_6db7),
    ("n4", 0x3f00_0000, 0x3f12_4925),
    ("n5", 0x3f00_0000, 0x3f36_db6e),
    ("n6", 0x3f00_0000, 0x3f5b_6db7),
    ("n7", 0x3f00_0000, 0x3f80_0000),
];

/// Ten leaves off one root: a single row, ten evenly spaced `x`, all at `y == 1`. Pins
/// the sibling separation of `1.0` across the whole row and the single `kx` rescale.
pub(crate) const FAN10: [Row; 11] = [
    ("r", 0x3f00_0000, 0x0000_0000),
    ("l0", 0x3d4c_cccd, 0x3f80_0000),
    ("l1", 0x3e19_999a, 0x3f80_0000),
    ("l2", 0x3e80_0000, 0x3f80_0000),
    ("l3", 0x3eb3_3333, 0x3f80_0000),
    ("l4", 0x3ee6_6666, 0x3f80_0000),
    ("l5", 0x3f0c_cccd, 0x3f80_0000),
    ("l6", 0x3f26_6666, 0x3f80_0000),
    ("l7", 0x3f40_0000, 0x3f80_0000),
    ("l8", 0x3f59_999a, 0x3f80_0000),
    ("l9", 0x3f73_3333, 0x3f80_0000),
];

/// A 6-deep left chain against a 2-deep right bush. This is the shape that makes
/// `apportion` step the contour across a subtree boundary — `separation` is `2.0` there
/// against `1.0` between siblings — so it exercises `move_subtree`, `next_ancestor` and
/// the `thread` bookkeeping in `finish_contour`. Every `x` in `b` and `b0..b4` is
/// load-bearing: a wrong contour step moves all of them.
pub(crate) const UNBALANCED: [Row; 14] = [
    ("r", 0x3ec0_0000, 0x0000_0000),
    ("a", 0x3e00_0000, 0x3e12_4925),
    ("a0", 0x3e00_0000, 0x3e92_4925),
    ("a1", 0x3e00_0000, 0x3edb_6db7),
    ("a2", 0x3e00_0000, 0x3f12_4925),
    ("a3", 0x3e00_0000, 0x3f36_db6e),
    ("a4", 0x3e00_0000, 0x3f5b_6db7),
    ("a5", 0x3e00_0000, 0x3f80_0000),
    ("b", 0x3f20_0000, 0x3e12_4925),
    ("b0", 0x3ec0_0000, 0x3e92_4925),
    ("b1", 0x3f00_0000, 0x3e92_4925),
    ("b2", 0x3f20_0000, 0x3e92_4925),
    ("b3", 0x3f40_0000, 0x3e92_4925),
    ("b4", 0x3f60_0000, 0x3e92_4925),
];

/// A caterpillar: a spine where each node carries one leaf, nine deep. The contour walk's
/// `vim` and `vip` descend together on both sides here, and one side always runs out
/// first, so both trailing `if`s in `finish_contour` have work to do.
pub(crate) const CATERPILLAR9: [Row; 17] = [
    ("c0", 0x3f00_0000, 0x0000_0000),
    ("c1", 0x3f00_0000, 0x3d80_0000),
    ("s1", 0x3f00_0000, 0x3e00_0000),
    ("c2", 0x3f00_0000, 0x3e40_0000),
    ("s2", 0x3f00_0000, 0x3e80_0000),
    ("c3", 0x3f00_0000, 0x3ea0_0000),
    ("s3", 0x3f00_0000, 0x3ec0_0000),
    ("c4", 0x3f00_0000, 0x3ee0_0000),
    ("s4", 0x3f00_0000, 0x3f00_0000),
    ("c5", 0x3f00_0000, 0x3f10_0000),
    ("s5", 0x3f00_0000, 0x3f20_0000),
    ("c6", 0x3f00_0000, 0x3f30_0000),
    ("s6", 0x3f00_0000, 0x3f40_0000),
    ("c7", 0x3f00_0000, 0x3f50_0000),
    ("s7", 0x3f00_0000, 0x3f60_0000),
    ("c8", 0x3f00_0000, 0x3f70_0000),
    ("s8", 0x3f00_0000, 0x3f80_0000),
];

/// The `tree-balanced` fixture (15 nodes, depth 3) replayed through the same D-H repair
/// the crate documents, so the golden covers the real ingest path — every parent-first
/// spelling plus `child_of`, in an order that is deliberately not the node order.
pub(crate) const TREE_BALANCED: [Row; 15] = [
    ("r", 0x3f00_0000, 0x0000_0000),
    ("a", 0x3e80_0000, 0x3eaa_aaab),
    ("a1", 0x3e00_0000, 0x3f2a_aaab),
    ("a1x", 0x3daa_aaab, 0x3f80_0000),
    ("a1y", 0x3e2a_aaab, 0x3f80_0000),
    ("a2", 0x3ec0_0000, 0x3f2a_aaab),
    ("a2x", 0x3eaa_aaab, 0x3f80_0000),
    ("a2y", 0x3ed5_5555, 0x3f80_0000),
    ("b", 0x3f40_0000, 0x3eaa_aaab),
    ("b1", 0x3f20_0000, 0x3f2a_aaab),
    ("b1x", 0x3f15_5555, 0x3f80_0000),
    ("b1y", 0x3f2a_aaab, 0x3f80_0000),
    ("b2", 0x3f60_0000, 0x3f2a_aaab),
    ("b2x", 0x3f55_5555, 0x3f80_0000),
    ("b2y", 0x3f6a_aaab, 0x3f80_0000),
];

/// `tree-degenerate`, the 8-node chain listed deepest-first (so the root has the *highest*
/// dense index) with every edge `child_of` and two non-positive weights. Dense order is
/// `n7, n6, ..., n0`, which is the reverse of the chain.
pub(crate) const TREE_DEGENERATE: [Row; 8] = [
    ("n7", 0x3f00_0000, 0x3f80_0000),
    ("n6", 0x3f00_0000, 0x3f5b_6db7),
    ("n5", 0x3f00_0000, 0x3f36_db6e),
    ("n4", 0x3f00_0000, 0x3f12_4925),
    ("n3", 0x3f00_0000, 0x3edb_6db7),
    ("n2", 0x3f00_0000, 0x3e92_4925),
    ("n1", 0x3f00_0000, 0x3e12_4925),
    ("n0", 0x3f00_0000, 0x0000_0000),
];
