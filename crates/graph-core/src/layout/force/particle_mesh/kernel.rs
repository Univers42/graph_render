//! The many-body law sampled on the mesh and transformed once: the spectrum the density's
//! spectrum is multiplied by.
//!
//! The law is d3's `manyBody.js`, the one Barnes-Hut ports: a node at offset `r` from
//! another gains `charge * alpha * (-r) / l` with `l = |r|²`, nothing at `l >= dmax²`,
//! and `l` raised to `sqrt(dmin² * l)` below `dmin²`. As a kernel that is
//! `G(r) = -r / l(r)`, stored packed as one complex sample, `Gx + i·Gy`: the density is
//! real, so one inverse transform returns both field components, `Ex` in the real part
//! and `Ey` in the imaginary. `G(0)` is zero, and with `G` odd the CIC deposit and the
//! CIC read cancel a node's force on itself.

use super::fft::{C, Plan};
use super::frame::Frame;

/// The squared cutoffs of the law, the only parameters the kernel depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Law {
    pub(super) dmin2: f64,
    pub(super) dmax2: f64,
}

/// The kernel's spectrum, in [`Plan::fft2`]'s transposed layout and pre-scaled by `1/P²`,
/// and the inputs it was built for.
///
/// Rebuilt only when the rung, the reach or the law changes, so a run pays one extra
/// forward transform per rung its span crosses rather than one per tick.
pub(super) struct Kernel {
    built_for: Option<(i32, usize, u64, u64)>,
    pub(super) spectrum: Vec<C>,
}

impl Kernel {
    pub(super) fn new(side: usize) -> Kernel {
        Kernel {
            built_for: None,
            spectrum: vec![C::default(); side * side],
        }
    }

    /// Makes the spectrum the one for `frame` and `law`; `scratch` is overwritten.
    pub(super) fn refresh(&mut self, plan: &Plan, frame: &Frame, law: Law, scratch: &mut [C]) {
        let key = (frame.step, frame.reach, law.dmin2.to_bits(), law.dmax2.to_bits());
        if self.built_for == Some(key) {
            return;
        }
        sample(scratch, plan.side(), frame, law);
        plan.fft2(scratch, &mut self.spectrum, false);
        self.built_for = Some(key);
    }
}

/// `g` zeroed, then the law at every cell offset within the frame's reach, wrapped modulo
/// `side` and scaled by `1/side²` (a power of two, so the scaling is exact).
pub(super) fn sample(g: &mut [C], side: usize, frame: &Frame, law: Law) {
    g.fill(C::default());
    let scale = 1.0 / (side * side) as f64;
    let reach = frame.reach as isize;
    for dy in -reach..=reach {
        let row = dy.rem_euclid(side as isize) as usize * side;
        for dx in -reach..=reach {
            let offset = (dx as f64 * frame.h, dy as f64 * frame.h);
            g[row + dx.rem_euclid(side as isize) as usize] = green(offset, law, scale);
        }
    }
}

/// `G(r) * scale`.
fn green((rx, ry): (f64, f64), law: Law, scale: f64) -> C {
    let mut l = rx * rx + ry * ry;
    if l == 0.0 || l >= law.dmax2 {
        return C::default();
    }
    if l < law.dmin2 {
        l = libm::sqrt(law.dmin2 * l);
    }
    C {
        re: -rx / l * scale,
        im: -ry / l * scale,
    }
}
