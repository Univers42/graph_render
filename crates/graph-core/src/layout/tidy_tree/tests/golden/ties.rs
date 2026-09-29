//! The shapes that make `normalize`'s strict `<` / `>` observable, and
//! `finish_contour`'s second trailing `if`. See the module doc.

use super::Row;

/// Witnesses for `normalize`'s tie-breaks, which are invisible on every symmetric shape.
///
/// `normalize` finds the extremes by a **strict** `<` / `>`, so a tie keeps the
/// first-visited node — d3's own rule. The chosen node is then read only through
/// `left.x`, `right.x` and `separation(left, right)`, and `separation` is `1` for
/// siblings and `2` across a subtree boundary. So a tie is observable exactly when the
/// candidates it might have picked **differ in parent**: break it the other way and `s`
/// changes, which changes `tx` and `kx`, which moves every node in the tree. A balanced
/// fixture ties symmetrically, so its tied candidates are always siblings and `s` never
/// moves — which is why the fixtures and the hand-worked tests all miss this.
///
/// `LEFT_TIE`: `v0 → v1 → v2`, and `v0 → v3`. Every node of the `v1` chain shares `v1`'s
/// `x`, and `v1` is a sibling of `v3` while `v2` is not. Strict `<` picks `v1`
/// (`separation(v1, v3) == 1`, so `s = 0.5`); `<=` would pick `v2` (`separation == 2`,
/// so `s = 1.0`).
pub(crate) const LEFT_TIE: [Row; 4] = [
    ("v0", 0x3f00_0000, 0x0000_0000),
    ("v1", 0x3e80_0000, 0x3f00_0000),
    ("v2", 0x3e80_0000, 0x3f80_0000),
    ("v3", 0x3f40_0000, 0x3f00_0000),
];

/// `RIGHT_TIE`: `v0 → v1`; `v1 → v2, v3, v4`; and `v0 → v5`. The maximum `x` is shared by
/// `v4` and `v5` — the tie has to straddle a subtree boundary, because two children of one
/// parent are always placed strictly apart and can never tie. Strict `>` takes `v4`, the
/// first in pre-order, and `v4` is a sibling of the minimum `v2`, so
/// `separation(v2, v4) == 1` and `s = 0.5`; `>=` would take `v5`, whose parent is `v0`,
/// giving `separation(v2, v5) == 2` and `s = 1.0`.
pub(crate) const RIGHT_TIE: [Row; 6] = [
    ("v0", 0x3f2a_aaab, 0x0000_0000),
    ("v1", 0x3f00_0000, 0x3f00_0000),
    ("v2", 0x3e2a_aaab, 0x3f80_0000),
    ("v3", 0x3f00_0000, 0x3f80_0000),
    ("v4", 0x3f55_5555, 0x3f80_0000),
    ("v5", 0x3f55_5555, 0x3f00_0000),
];

/// A witness for the **second** trailing `if` in `finish_contour` — the `vom` thread
/// installed when the *left* contour runs out first, and the `m[vom] += sip - som` that
/// goes with it.
///
/// That branch runs often, but on the symmetric shapes above it always finds `sip == som`,
/// so `sip - som` and `sip + som` agree and the whole line is invisible. This 6-node shape
/// runs the branch with `sip - som` large against `m[vom]`, which is the only thing that
/// tells the mutations there apart. Measured over a sweep of ~15k random trees, the branch
/// runs 21374 times and has `sip != som` on 20613 of them — it is reachable, it just needs
/// the right shape.
///
/// `v0` parents `v1, v2, v3, v4`; `v4` parents `v5`.
pub(crate) const FINISH_SECOND_BRANCH: [Row; 6] = [
    ("v0", 0x3f00_0000, 0x0000_0000),
    ("v1", 0x3e00_0000, 0x3f00_0000),
    ("v2", 0x3ec0_0000, 0x3f00_0000),
    ("v3", 0x3f20_0000, 0x3f00_0000),
    ("v4", 0x3f60_0000, 0x3f00_0000),
    ("v5", 0x3f60_0000, 0x3f80_0000),
];
