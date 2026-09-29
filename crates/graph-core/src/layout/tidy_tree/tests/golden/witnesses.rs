//! The shapes that reach `executeShifts`' `shift` at all, and the
//! `m[w] += shift` line in particular. See the module doc for the mechanism.

use super::Row;

/// The smallest shape this suite found (7 nodes) on which `executeShifts` applies a
/// **non-zero** `shift`, which is the only way its `z[w] += shift` can be seen at all.
///
/// The mechanism is not obvious from the shape. `moveSubtree(wm, wp, shift)` sets
/// `wp.s += shift` and `wp.c -= change` with `change = shift / (wp.i - wm.i)`. For
/// `wp.i - wm.i == 1` — by far the common case — `s` and `c` cancel *exactly* and the
/// row's running total stays `0`. Here `v1` is the **second** of `v0`'s three children and
/// the conflict resolves against `v0` itself, so `wp.i - wm.i == 2`: `v1` ends up with
/// `s = S` and `c = -S/2`, which do not cancel. The reverse walk adds that child's
/// `s + change` into a total that is still `0`, and the *next* child along (`v3`) is
/// shifted by the residue. Every other golden here has `idxdiff == 1` and would pass
/// against a completely broken `execute_shifts`.
///
/// `v0` parents `v1, v3, v4`; `v1` parents `v2, v5`; `v4` parents `v6`.
pub(crate) const EXECUTE_SHIFTS_WITNESS: [Row; 7] = [
    ("v0", 0x3f0c_cccd, 0x0000_0000),
    ("v1", 0x3e99_999a, 0x3f00_0000),
    ("v2", 0x3e4c_cccd, 0x3f80_0000),
    ("v3", 0x3f0c_cccd, 0x3f00_0000),
    ("v4", 0x3f4c_cccd, 0x3f00_0000),
    ("v5", 0x3ecc_cccd, 0x3f80_0000),
    ("v6", 0x3f4c_cccd, 0x3f80_0000),
];

/// The companion to the above, and the only shape this suite found on which
/// `m[w] += shift` is observable **on its own**. The 7-node witness shifts leaves, whose
/// `z` is what carries; here the shifted child is *internal* (`v2`, which parents
/// `v5, v7, v8`), and an internal node's `z` is overwritten by its own `firstWalk`
/// (`v.z = w.z + separation`, or the midpoint) while its `m` is not — `m` only
/// accumulates, and is summed later in `secondWalk` and in `apportion`'s running
/// `sim`/`sip`. A leaf's `m` reaches nothing at all.
///
/// Edges (parent, child), admission order = id order: `0->1, 0->2, 0->3, 0->4, 2->5,
/// 0->6, 2->7, 2->8, 7->9, 6->10, 5->11`.
pub(crate) const M_SHIFT_WITNESS: [Row; 12] = [
    ("v0", 0x3f09_2492, 0x0000_0000),
    ("v1", 0x3e5b_6db7, 0x3eaa_aaab),
    ("v2", 0x3eb6_db6e, 0x3eaa_aaab),
    ("v3", 0x3f06_1862, 0x3eaa_aaab),
    ("v4", 0x3f30_c30c, 0x3eaa_aaab),
    ("v5", 0x3e12_4925, 0x3f2a_aaab),
    ("v6", 0x3f5b_6db7, 0x3eaa_aaab),
    ("v7", 0x3edb_6db7, 0x3f2a_aaab),
    ("v8", 0x3f12_4925, 0x3f2a_aaab),
    ("v9", 0x3edb_6db7, 0x3f80_0000),
    ("v10", 0x3f5b_6db7, 0x3f2a_aaab),
    ("v11", 0x3e12_4925, 0x3f80_0000),
];

/// A third witness, and the only one this suite found where the **sign flip** on that same
/// `m[w] += shift` is observable. `M_SHIFT_WITNESS` above does shift an internal node, but
/// by 5.55e-17 against an `m` of -0.5 — under one ulp, so `-=` rounds straight back and
/// the mutation survives. This 48-node shape moves an internal node by 4.5 against an `m`
/// of 3, which no rounding can absorb.
///
/// It is large, and deliberately so: an exhaustive sweep of chain and bush shapes up to
/// five siblings and depth five found no smaller tree reaching this state, and a golden
/// checked in by *running* d3 is worth more than a small one reasoned into existence.
/// Every other golden in this file is far smaller.
pub(crate) const SIGN_FLIP_WITNESS: [Row; 48] = [
    ("v0", 0x3f33a7a2, 0x00000000),
    ("v1", 0x3ee36942, 0x3de38e39),
    ("v2", 0x3d918b0c, 0x3e638e39),
    ("v3", 0x3d4feb35, 0x3eaaaaab),
    ("v4", 0x3eb5edcf, 0x3e638e39),
    ("v5", 0x3f3c6d28, 0x3e638e39),
    ("v6", 0x3f6b3547, 0x3de38e39),
    ("v7", 0x3dbb207d, 0x3eaaaaab),
    ("v8", 0x3f1e89bf, 0x3eaaaaab),
    ("v9", 0x3d2655c4, 0x3ee38e39),
    ("v10", 0x3eef1b4a, 0x3ee38e39),
    ("v11", 0x3e7980a6, 0x3f0e38e4),
    ("v12", 0x3ee9e89c, 0x3f0e38e4),
    ("v13", 0x3f660299, 0x3e638e39),
    ("v14", 0x3ec585d9, 0x3f2aaaab),
    ("v15", 0x3e118b0c, 0x3ee38e39),
    ("v16", 0x3f1bf068, 0x3f0e38e4),
    ("v17", 0x3ebb207d, 0x3f471c72),
    ("v18", 0x3f4585d9, 0x3ee38e39),
    ("v19", 0x3f30bb20, 0x3f0e38e4),
    ("v20", 0x3e4feb35, 0x3f2aaaab),
    ("v21", 0x3f46d285, 0x3e638e39),
    ("v22", 0x3ecfeb35, 0x3f471c72),
    ("v23", 0x3f759aa4, 0x3de38e39),
    ("v24", 0x3d2655c4, 0x3f0e38e4),
    ("v25", 0x3f3c6d28, 0x3eaaaaab),
    ("v26", 0x3f0725af, 0x3f2aaaab),
    ("v27", 0x3f1bf068, 0x3f2aaaab),
    ("v28", 0x3df980a6, 0x3f0e38e4),
    ("v29", 0x3ebb207d, 0x3f638e39),
    ("v30", 0x3e4feb35, 0x3f471c72),
    ("v31", 0x3f30bb20, 0x3f2aaaab),
    ("v32", 0x3e918b0c, 0x3f2aaaab),
    ("v33", 0x3f5137e1, 0x3e638e39),
    ("v34", 0x3ef980a6, 0x3f471c72),
    ("v35", 0x3f5a5092, 0x3eaaaaab),
    ("v36", 0x3eb5edcf, 0x3eaaaaab),
    ("v37", 0x3f5a5092, 0x3ee38e39),
    ("v38", 0x3f4585d9, 0x3f0e38e4),
    ("v39", 0x3e918b0c, 0x3f471c72),
    ("v40", 0x3ebb207d, 0x3f800000),
    ("v41", 0x3f0725af, 0x3f471c72),
    ("v42", 0x3d2655c4, 0x3f2aaaab),
    ("v43", 0x3f7067f6, 0x3e638e39),
    ("v44", 0x3e2655c4, 0x3f0e38e4),
    ("v45", 0x3f4585d9, 0x3f2aaaab),
    ("v46", 0x3f118b0c, 0x3f471c72),
    ("v47", 0x3f7067f6, 0x3eaaaaab),
];
