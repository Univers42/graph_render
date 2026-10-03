//! The four graph-free 3D placements — `layout.basic3d.sphere`, `.helix`, `.cube` and
//! `.spiral` — against **SciGraphs' own** `_sphere_layout`, `_helix_layout`, `_cube_layout`
//! and `_spiral_layout_3d` (`harness/oracle-basic-3d.py`), run in the `ge-python-oracle`
//! image with the `SciGraphs/` submodule mounted so the arm is the reference function itself
//! and not a restatement of it.
//!
//! **One differential for four layouts, because the four take the same two arguments and
//! read no graph at all** (`basic.py:22`, `:65`, `:83`, `:36` — all `(num_nodes, scale)`).
//! Five near-identical arm files would be the redundancy the library-first rule forbids, and
//! the differences between the four belong in one place: `harness/oracle-basic-3d.py`'s
//! `ARMS` table and its `--function`-shaped docstring. Each still carries its own ceiling,
//! because each is a different function whose worst case is its own.
//!
//! `SPHERE`, `HELIX` and `SPIRAL` are closed forms, so agreement is a coordinate tolerance
//! and nothing weaker. **`CUBE` is compared in two halves and the split is the point**: its
//! eight corners are exact and are what the gate compares, and its interior is compared
//! *distributionally* because the reference draws it from a module-level **global**
//! `np.random.RandomState` (`common.py:43-52`) — so no interior coordinate of ours can
//! equal SciGraphs' for any seed, and the reference's own interior depends on every earlier
//! layout in the process that drew from it. The corners are what the layout is.
//!
//! **`SPIRAL` is compared the way a closed form is compared, coordinates within a tolerance,
//! and it is the only one of the four whose parameters are NOT closed form.** Every other
//! coordinate here is a direct transcription of one line of `basic.py`; the spiral's `t` is
//! an arc-length INVERSION (`basic.py:45-56`): a 65 536-point `linspace`, a sequential
//! `cumsum` of trapezoid speeds, and `np.interp` per node. So its comparison is the one that
//! earns the tolerance rather than merely resting on it — a port that gets `linspace`,
//! `cumsum`'s order or `interp`'s slope formula right agrees to the narrowing, and one that
//! gets any of the three wrong is off by orders of magnitude on `t` and therefore on every
//! coordinate, because `t` multiplies `omega` straight into `cos`/`sin`. Its node count also
//! sets the curve: `turns = max(2, round(sqrt(n / (0.75*pi))))` (`basic.py:41`), so the sweep
//! over counts is what exercises the integer turning count, not a detail of the placement.
//!
//! **Every ceiling in this file is MEASURED on this tree**, over 1000 seeds of the gate's own
//! model, and the measured numbers are on each constant. They are one host and one image; a
//! different libm could move the last digit of a coordinate gap, which is what the ceiling's
//! power of ten is for.

use super::{Differential, columns_3d};
use graph_core::layout::basic_3d::{cube, helix, sphere, spiral};
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const BASIC_3D: Differential = Differential {
    name: "basic-3d",
    ceilings: &[
        (sphere::ID, "sphere", CEILING),
        (helix::ID, "helix", CEILING),
        // **The corners, not the interior** — and the harness says so in its own
        // `compared` field, because `worst` does not mean the same thing in all four rows
        // and the judge's gate is `worst <= ceiling`. See the module doc.
        (cube::ID, "cube", CORNER_CEILING),
        // A coordinate comparison like the first two, and its own ceiling because its
        // `t` is an inversion rather than a transcription. See [`SPIRAL_CEILING`].
        (spiral::ID, "spiral", SPIRAL_CEILING),
    ],
    line,
};

/// The coordinate ceiling for the two closed forms. **Measured** over 1000 seeds
/// (`emit-basic-3d-fixtures --seeds 1000`, then this arm in `ge-python-oracle`): sphere's
/// worst `max |ours - theirs|` **2.384e-7**, helix's **2.376e-7**, and every one of the 1000
/// seeds bit-identical on both arms after the snapshot's `f32` narrowing. The gap is that
/// narrowing and `np.cos` against `libm::cos`, nothing else, and the ceiling is the next power
/// of ten above it.
pub(crate) const CEILING: f64 = 1e-6;

/// `CUBE`'s ceiling, over the **corners only**: measured worst `max |ours - theirs|` **0.0**,
/// exact on 1000 of 1000 seeds, which is what a literal array multiplied by an `f32`-exact
/// `scale = 5.0` must give.
///
/// It is one power of ten above that rather than the `0.0` itself, because a tolerance
/// sitting exactly on the number it was measured from is not one, and because a future
/// non-representable `scale` would make an exact ceiling refuse a correct port.
///
/// **The interior is not gated, and could not be.** Its coordinates are drawn from this
/// crate's own `Mulberry32`, against the reference's module-level global
/// `np.random.RandomState`, so the two disagree by design: measured `interior_gap_not_gated`
/// is **7.909** at seed 52 and only **7 of 100** seeds are bit-identical interior-wide, both of
/// which are the correct behaviour and are reported rather than hidden. What *is* gated of the
/// interior is its distribution — `interior_variance_ratio` **1.000**, i.e. uniform on
/// `[-0.8*scale, 0.8*scale]` — and the report carries those numbers beside the ones it gates.
pub(crate) const CORNER_CEILING: f64 = 1e-7;

/// `SPIRAL`'s ceiling, over **all three columns of every node** — the whole layout, because
/// nothing in it is drawn from a stream and nothing in it is a literal array.
///
/// **Measured** over 1000 seeds (`emit-basic-3d-fixtures --seeds 1000`, then this arm in
/// `ge-python-oracle`): worst `max |ours - theirs|` **2.384e-7** at seed **563**, and
/// **1000 of 1000** seeds bit-identical on both arms after the snapshot's `f32` narrowing —
/// the same 1000-of-1000 the two pure transcriptions reach, which is the result worth
/// reading twice: the arc-length inversion agrees to the narrowing too, so `linspace`, the
/// sequential `cumsum` and `interp`'s slope formula are all bit right, not merely close.
///
/// The ceiling is the same power of ten as [`CEILING`] and the worst is the same number to
/// four digits, which is what the *narrowing* costs and not the inversion: at `scale = 5.0`
/// every spiral coordinate is a handful of units, so one `f32` ulp there is the 2.4e-7 the
/// three closed forms all report.
///
/// Ponytail: what this tolerates is **one `f32` ulp of a coordinate, and nothing else** — it
/// does not check `turns` against a fixed count, it lets the reference's own `turns = max(2,
/// round(sqrt(n / (0.75*pi))))` (`basic.py:41`) stand and sweeps node counts so the integer
/// varies under it, which is the only way the floor and the half-to-even rounding are
/// reached at all (both bite at `n <= 14`, where `turns` is 2 because `max` lifted it, and
/// the first count that rounds to 3 is `n = 15`). A port that hard-codes `turns = 2` and gets
/// everything else right stays inside this ceiling on every `n < 15` and misses the row
/// entirely; the sweep is the control for that, not the tolerance.
pub(crate) const SPIRAL_CEILING: f64 = 1e-6;

/// SciGraphs' `apply_graph_layout(scale=5.0)` default (`dispatcher.py:14`), restated in the
/// fixture so the arm is handed the same `scale` the module hard-codes. All four take one
/// `scale` and no iteration budget, so `--max-iter` is ignored, as `closed_form.rs`'s is.
const SCALE: f64 = 5.0;

/// One seed's line.
///
/// The graph is emitted because it is the gate's own model and the arm is told the node
/// **count** — these four read nothing else, which is the reference's own behaviour and the
/// reason the fixture carries no edge data the arm would use.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out = json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target,
                "scale": SCALE });
    for &(id, key, _) in BASIC_3D.ceilings {
        let layout = graph_core::registry::find(id).ok_or_else(|| format!("{id}: gone"))?;
        let run =
            graph_core::run_with(&nodes, &edges, id, layout.run).map_err(|e| e.to_string())?;
        out[key] = columns_3d(
            id,
            &run.snapshot.parts().nodes,
            run.snapshot.parts().z.as_deref(),
        )?;
    }
    Ok(out)
}
