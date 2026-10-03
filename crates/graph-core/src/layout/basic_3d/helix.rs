//! `_helix_layout` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:65-81`): a
//! double helix, two nodes per turn, ported formula for formula. Closed form, no stream,
//! no graph.
//!
//! The construction, as the reference writes it:
//!
//! ```text
//! levels = (n + 1) // 2                  how many pairs of turns
//! t      = (i // 2) / (levels - 1)       which turn node i sits on
//! angle  = t*4*pi + (i % 2)*pi          the strand: even i on one, odd on the other
//! radius = scale * 0.3                  fixed, so the strands never approach
//! x      = radius*cos(angle)
//! y      = radius*sin(angle)
//! z      = t*scale*2 - scale            -scale at t=0, +scale at t=1
//! ```
//!
//! **The `levels == 1` branch is the one a port gets wrong silently**, so it is stated
//! here and pinned in the tests. `t` divides by `levels - 1`, which is zero for a one-node
//! graph — the reference guards it (`basic.py:71-74`) and returns `t = 0.5` for every
//! node, which is the **midpoint** of the helix, not its foot. So a one-node helix is
//! `z = 0`, and a two-node helix (levels = 1 as well) is two antipodal points, both at
//! `z = 0`. `0.5` is not an arbitrary filler: it is the only value that keeps a
//! degenerate helix centred rather than pinned to the `-scale` end, and a port that
//! returned `t = 0` instead would draw one node at the bottom and two at the bottom,
//! silently, with every formula still "right". **The differential compares this case
//! explicitly** — it is the row a `if levels > 1` inverted will fail.
//!
//! **The odd node count is the reference's own asymmetry**: `(n + 1) // 2` puts the extra
//! node on strand 0, because node `n-1` is even when `n` is odd and `(i % 2)` is its
//! strand (`basic.py:66-68`). Nothing rounds to make the strands even; the strand
//! assignment is `i % 2` and nothing else.
//!
//! **Why `4*pi` and not `2*pi`.** Two nodes per turn means consecutive nodes are
//! `pi` apart in angle, and `4*pi` over one turn puts node `2k` and node `2k+1` a half
//! turn apart on opposite strands. That is the whole of the double helix; there is no
//! second radius and no second offset in the reference.

use super::{SCALE, in_space};
use crate::layout::Geometry;
use crate::stage::StageError;

/// The strand offset: `(i % 2) * pi` (`basic.py:76`), a half turn between the strands.
const STRAND: f64 = core::f64::consts::PI;

/// The fixed radius, `scale * 0.3` (`basic.py:77`). Not a parameter: the reference has
/// one `scale` and this coefficient is part of its shape, not of its tuning.
const RADIUS_RATIO: f64 = 0.3;

/// `HELIX`'s capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.basic3d.helix";

/// `_helix_layout(n, scale)` (`basic.py:65-81`) over the node count, at [`SCALE`].
pub(super) fn run(n: u32) -> Result<Geometry, StageError> {
    let (x, y, z) = columns(n);
    Ok(in_space(&x, &y, &z))
}

/// [`run`] at the `scale` the caller asks for. Both uses of it are the reference's own
/// (`radius = scale * 0.3` and `z = t*scale*2 - scale`), so
/// `run_scaled(n, SCALE) == run(n)` bit for bit.
pub(super) fn run_scaled(n: u32, scale: f64) -> Result<Geometry, StageError> {
    let (x, y, z) = columns_scaled(n, scale);
    Ok(in_space(&x, &y, &z))
}

/// The three `f64` columns at `n`, before narrowing, at [`SCALE`].
///
/// The `levels` guard is [`turn`], and it is a function of the whole count rather than a
/// branch inside the loop, so the degenerate case is decided once rather than per node.
pub(super) fn columns(n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    columns_scaled(n, SCALE)
}

/// [`columns`] at an explicit `scale` (`basic.py:71-81`).
pub(super) fn columns_scaled(n: u32, scale: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    // `(n + 1) // 2` (`basic.py:68`), which is `n.div_ceil(2)` — the same integer, and the
    // reading that says what it is: the number of *pairs* of nodes a helix of `n` holds.
    let levels = n.div_ceil(2);
    let mut columns = (
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
    );
    let radius = scale * RADIUS_RATIO;
    for i in 0..n {
        let t = turn(i, levels);
        let angle = t * 4.0 * core::f64::consts::PI + f64::from(i % 2) * STRAND;
        columns.0.push(radius * libm::cos(angle));
        columns.1.push(radius * libm::sin(angle));
        columns.2.push(t * scale * 2.0 - scale);
    }
    columns
}

/// `t` for node `i` (`basic.py:71-74`): which turn of the helix it sits on.
///
/// **The degenerate case is the point of this function.** `levels - 1` is zero exactly
/// when `n < 3` (`(n + 1) // 2` is 0 at `n = 0` and 1 at both `n = 1` and `n = 2`), and
/// the reference returns `0.5` for every node then — the helix's midpoint. (This comment
/// used to say "when `n` is 0 or 1" and then list three node counts two lines later; `n = 2`
/// takes the `else` branch too.) Written as one `if` over a `t` computed by a single
/// division so the zero cannot be divided by anywhere else.
fn turn(i: u32, levels: u32) -> f64 {
    if levels > 1 {
        f64::from(i / 2) / f64::from(levels - 1)
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests;
