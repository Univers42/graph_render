//! Where the Kamada-Kawai descent starts: a circle in the plane, a sphere in space.
//!
//! Split from `kamada_kawai.rs` for the house line cap. The shape of the start is the whole
//! content, and the 3D half is not a decoration — see [`sphere`].

/// The widest point the port keeps; see [`super::MAX_DIM`].
const MAX_DIM: usize = 3;

/// Vertices on a circle of radius `0.36 * sqrt(n)` at `dim` 2, on a sphere of the same
/// radius at `dim` 3 — the spec's empirical start radius, on whichever surface that
/// dimension needs.
///
/// **The 3D sphere is required, not decorative.** A 3D start drawn in a plane leaves every
/// `zz` entry of the first Newton Hessian at zero (the springs' second derivative along `z`
/// depends on `dz`, which is zero for every pair), so the block is singular, the step is
/// zero by [`super::solve`]'s guard, and the layout never leaves the plane: the 3D arm would
/// have answered with a z column of zeros.
pub(super) fn circle_start(n: usize, dim: usize) -> Vec<[f64; MAX_DIM]> {
    let radius = 0.36 * libm::sqrt(n as f64);
    if dim == 2 {
        return ring(n, radius);
    }
    sphere(n, radius)
}

/// The 2D arm's own start, unchanged: `i`-th vertex at angle `2 pi i / n` on the circle.
///
/// The third slot is a literal `0.0` rather than a drawn value — at `dim` 2 nothing reads
/// it, and writing it as a zero says so where deriving it would say nothing.
fn ring(n: usize, radius: f64) -> Vec<[f64; MAX_DIM]> {
    (0..n)
        .map(|i| {
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            [radius * libm::cos(angle), radius * libm::sin(angle), 0.0]
        })
        .collect()
}

/// The closed-form Fibonacci sphere: `y` uniform on `[-1, 1)`, the azimuth stepping by the
/// golden angle, and `r = sqrt(1 - y^2)`.
///
/// Ponytail: igraph starts its own 3D KK from its own seeded sphere, not this one, so the
/// two pictures differ from the first move. That is cosmetic — KK's descent finds a local
/// minimum, and the differential compares stress rather than coordinates — and the escape
/// hatch is `sphere`'s formula, which is one line to change.
fn sphere(n: usize, radius: f64) -> Vec<[f64; MAX_DIM]> {
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = libm::sqrt((1.0 - y * y).max(0.0));
            let angle = golden * i as f64;
            [
                radius * r * libm::cos(angle),
                radius * r * libm::sin(angle),
                radius * y,
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every vertex of the 3D start is off the plane, and on the sphere of the spec's own
    /// radius. This is the property the Newton step depends on: a start with `z = 0`
    /// everywhere would give a singular Hessian and a layout that never leaves the plane,
    /// so a 3D arm whose z column was all zeros would still pass a "does it have a z column"
    /// check.
    #[test]
    fn the_3d_start_is_a_sphere_and_not_a_plane() {
        let n = 12;
        let start = circle_start(n, 3);
        assert_eq!(start.len(), n);
        let radius = 0.36 * libm::sqrt(n as f64);
        assert!(start.iter().any(|p| p[2] != 0.0), "z is live");
        for p in &start {
            let r = libm::sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
            assert!(
                (r - radius).abs() < 1e-12,
                "off the sphere: {r} vs {radius}"
            );
        }
    }

    /// The 2D start is untouched by the 3D arm existing: same radius, same angles, `z` a
    /// literal zero.
    #[test]
    fn the_2d_start_is_the_circle_it_always_was() {
        let n = 6;
        let start = circle_start(n, 2);
        let radius = 0.36 * libm::sqrt(n as f64);
        for (i, p) in start.iter().enumerate() {
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            assert!((p[0] - radius * libm::cos(angle)).abs() < 1e-12);
            assert!((p[1] - radius * libm::sin(angle)).abs() < 1e-12);
            assert_eq!(p[2], 0.0, "the third slot is a literal zero at dim 2");
        }
    }
}
