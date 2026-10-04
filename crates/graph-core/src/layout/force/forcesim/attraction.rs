//! `_attraction` (`simulation.py:694-724`), the `model='FA2'` branch.
//!
//! ```text
//! diff   = pos[dst] - pos[src]                                  f32
//! factor = ones(E, f32) * attraction                            f32   (Python float, weak)
//! factor = factor / (f32(0.5) * (mass[src] + mass[dst]))          f32
//! pull   = diff * factor[:, None]                               f32
//! acc    = bincount(src, weights=pull[:, ax])                   f64
//! acc   -= bincount(dst, weights=pull[:, ax])                   f64
//! force[:, ax] = acc.astype(f32)                                f32
//! ```
//!
//! **The reduction is `np.bincount`, not `np.add.at` and not a `sum`.** It is sequential in
//! edge index, and it accumulates in `f64` whatever the weights' dtype — numpy's `bincount`
//! declares `double*` for the weights — which is why `pull` is computed in `f32` and still
//! lands in an `f64` accumulator before the `astype(DTYPE)` at `simulation.py:723`. This
//! port reproduces it exactly: one sequential `f64` pass adding into `src`, then a second
//! subtracting into `dst`, in the same edge order.
//!
//! **`LINLOG` and the `FR`/`YIFAN_HU` branches are not ported** (`simulation.py:702-707`):
//! `_forceatlas2_forcesim` constructs the model as `'FA2'` (`forceatlas.py:131`) and only
//! switches to `LINLOG` when `lin_log_mode` is set (`simulation.py:310-311`), which this
//! layout does not expose. The module doc's `Ponytail (params)` says so.

/// `self.attraction` as `_forceatlas2_forcesim` leaves it: `ForceSim.__init__`'s default
/// of `1.0` (`simulation.py:265`), never passed by `forceatlas.py:129-140`.
///
/// `float(attraction)` is a Python float, so `factor * self.attraction` keeps the `f32`
/// array `f32` (NEP 50) — the multiplication is a no-op at 1.0 and is not written out.
const ATTRACTION: f32 = 1.0;

/// The attraction field, added into `force` by the caller.
pub(super) fn accumulate(force: &mut [f32], pos: &[f32], mass: &[f32], edges: &[(u32, u32)]) {
    let mut pull = vec![0.0f32; 3 * edges.len()];
    for (e, &(a, b)) in edges.iter().enumerate() {
        let (a, b) = (a as usize, b as usize);
        // `DTYPE(0.5) * (mass[src] + mass[dst])`: the f32 add first, then the f32 scale,
        // then one f32 division. A self-loop is impossible — `simple_graph` drops those.
        let factor = ATTRACTION / (0.5f32 * (mass[a] + mass[b]));
        for axis in 0..3 {
            pull[3 * e + axis] = (pos[3 * b + axis] - pos[3 * a + axis]) * factor;
        }
    }
    let mut acc = vec![0.0f64; mass.len()];
    for axis in 0..3 {
        acc.iter_mut().for_each(|slot| *slot = 0.0);
        for (e, &(src, _)) in edges.iter().enumerate() {
            acc[src as usize] += f64::from(pull[3 * e + axis]);
        }
        for (e, &(_, dst)) in edges.iter().enumerate() {
            acc[dst as usize] -= f64::from(pull[3 * e + axis]);
        }
        for (i, &total) in acc.iter().enumerate() {
            force[3 * i + axis] += total as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_isolated_pair_pulls_each_endpoint_towards_the_other_by_the_same_amount() {
        // Two nodes at x = 0 and x = 4, no other edge, unit mass.
        let pos = [0.0f32, 0.0, 0.0, 4.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let mut force = [0.0f32; 6];
        accumulate(&mut force, &pos, &mass, &[(0, 1)]);
        // factor = 1 / (0.5 * (1 + 1)) = 1, so pull = diff = +4 on x.
        assert_eq!(force, [4.0, 0.0, 0.0, -4.0, 0.0, 0.0]);
    }

    /// The weight splits by mass: an edge from a heavy node to a light one pulls the heavy
    /// node less, because `factor = 1 / (0.5 * (mass[src] + mass[dst]))`.
    #[test]
    fn the_pull_is_split_by_the_endpoints_masses() {
        let pos = [0.0f32, 0.0, 0.0, 4.0, 0.0, 0.0];
        let mass = [3.0f32, 1.0];
        let mut force = [0.0f32; 6];
        accumulate(&mut force, &pos, &mass, &[(0, 1)]);
        let factor = 1.0f32 / (0.5 * (3.0 + 1.0));
        assert_eq!(force[0], 4.0 * factor);
        assert_eq!(force[3], -4.0 * factor);
    }

    /// A parallel pair collapses in `simple_graph`, but if one reached here the two passes
    /// over it must compound: `acc[src] += pull` and `acc[dst] -= pull` are both run.
    #[test]
    fn a_repeated_pair_is_counted_once_per_occurrence() {
        let pos = [0.0f32, 0.0, 0.0, 2.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let mut once = [0.0f32; 6];
        let mut twice = [0.0f32; 6];
        accumulate(&mut once, &pos, &mass, &[(0, 1)]);
        accumulate(&mut twice, &pos, &mass, &[(0, 1), (0, 1)]);
        assert_eq!(twice[0], 2.0 * once[0]);
        assert_eq!(twice[3], 2.0 * once[3]);
    }
}
