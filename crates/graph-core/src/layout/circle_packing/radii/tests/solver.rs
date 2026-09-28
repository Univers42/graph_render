//! The radius solver's own per-corner angle, and the gather `angle_sums` is in after
//! D10. Two claims are pinned in this module's children. First, the solver's angle is
//! *not* `_packing_angle`: the solver clamps its denominator and takes the `arccos` of
//! whatever survives, with no `pi/3` branch (`circle_packing.py:160-161`), so it disagrees
//! with the placement helper by up to 2.1 radians once `2ab` underflows — that is
//! [`angle`]. Second, the gather reproduces the scatter bit for bit over seeded flowers,
//! which it can only do if the per-vertex fold order is the corner array's own — that is
//! [`gather`].

mod angle;
mod gather;

use super::super::solver_angle;

/// The reference's own per-sweep angle, written out of `circle_packing.py:156-161` so
/// the tests state it independently of the production function: `denom` is *clamped*
/// (`np.maximum(2 * a * b, 1e-12)`) rather than guarded, and there is no `pi/3` branch.
pub(super) fn reference_solver_angle(r_i: f64, r_j: f64, r_k: f64) -> f64 {
    let (a, b, c) = (r_i + r_j, r_i + r_k, r_j + r_k);
    let denom = (2.0 * a * b).max(1e-12);
    libm::acos(((a * a + b * b - c * c) / denom).clamp(-1.0, 1.0))
}

/// A flat corner array plus `n` vertices, the shape `np.bincount` sees: the reference
/// flattens the flowers vertex-major at `circle_packing.py:143`, so a vertex's run of the
/// array is its own flower, in flower order. The gather takes the same array split back
/// into per-vertex flowers, which is what makes the two comparable.
pub(super) struct Flat {
    pub(super) n: usize,
    pub(super) corners: Vec<(u32, u32, u32)>,
    pub(super) at: Vec<Vec<(u32, u32)>>,
}

impl Flat {
    /// `at[i]` first, then flatten it vertex-major — the reference's own order.
    pub(super) fn from_flowers(at: Vec<Vec<(u32, u32)>>) -> Self {
        let n = at.len();
        let corners = (0..n as u32)
            .zip(&at)
            .flat_map(|(i, pairs)| pairs.iter().map(move |&(l, r)| (i, l, r)))
            .collect();
        Self { n, corners, at }
    }
}

/// The scatter `angle_sums` had before D10: one loop over the flat corner array, each
/// corner writing into its own centre's accumulator (`sums[c] += ...`). Test-only
/// reference — the production path is the gather, and this exists to prove the two agree
/// bit for bit.
pub(super) fn scatter_angle_sums(flat: &Flat, radii: &[f64]) -> Vec<f64> {
    let mut sums = vec![0.0; flat.n];
    for &(c, l, r) in &flat.corners {
        sums[c as usize] += solver_angle(radii[c as usize], radii[l as usize], radii[r as usize]);
    }
    sums
}

pub(super) fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// mulberry32, the same generator `crate::synthetic` uses: a seeded, target-independent
/// stream, so these flowers are the same bits on every run and every target.
pub(super) struct Rng(pub(super) u32);

impl Rng {
    pub(super) fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        t ^ (t >> 14)
    }

    /// Seeded uniform in `[0, 1)`, the stream's raw shape.
    pub(super) fn unit(&mut self) -> f64 {
        f64::from(self.next_u32()) / 4_294_967_296.0
    }
}

/// Radii span eight decades, so a seeded flower both underflows the denominator and sits
/// well clear of it, and the gather sweep exercises both forms.
pub(super) const DECADES: [f64; 8] = [1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1.0];

/// One seeded flower: `n` vertices, `m` corner slots whose centres repeat. `seed % 3`
/// caps the centre draw, so on some seeds only the first third of the vertices carry a
/// corner at all and the rest gather to exactly zero.
pub(super) fn seeded_flower(seed: u32, seed_index: u32) -> Flat {
    let n = (4 + seed_index % 9) as usize;
    let m = seed_index as usize * 3;
    let mut rng = Rng(seed);
    let centres = n as u32 - seed_index % 3;
    let mut at = vec![Vec::new(); n];
    for _ in 0..m {
        let c = rng.next_u32() % centres;
        let l = rng.next_u32() % n as u32;
        let mut r = rng.next_u32() % n as u32;
        if r == l {
            r = (r + 1) % n as u32;
        }
        at[c as usize].push((l, r));
    }
    Flat::from_flowers(at)
}

/// The radii for a seeded flower, from its own stream. Spread across [`DECADES`] per
/// vertex, which puts some flowers wholly above the `1e-12` denominator and some wholly
/// below it.
pub(super) fn seeded_radii(flat: &Flat, seed: u32) -> Vec<f64> {
    let mut rng = Rng(seed ^ 0x51D2_C0DE);
    (0..flat.n)
        .map(|_| rng.unit() * DECADES[(rng.next_u32() % 8) as usize])
        .collect()
}
