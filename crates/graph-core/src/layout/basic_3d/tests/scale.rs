//! The one claim all five `*_scaled` entry points share: the reference's `scale` is a
//! parameter, not a constant.
//!
//! Split from [`super`] because that file is about what the five placements read — the node
//! count and nothing else — and this is about the second argument `basic.py` hands them
//! (`layout_scale`, `scene_properties.py:478-483`) which used to be unreachable.

use super::{bare, space};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::basic_3d::SCALE;
use crate::layout::basic_3d::{
    bipartite_3d, bipartite_3d_scaled, cube, cube_scaled, helix, helix_scaled, sphere,
    sphere_scaled, spiral, spiral_scaled,
};
use crate::stage::StageError;

/// **The `scale` argument is a parameter, and this is the test that says so.** [`SCALE`] is
/// SciGraphs' *dispatcher default* (`layouts/dispatcher.py:14`), not the only value the
/// reference can be asked for: `layout_scale` is a user-facing `FloatProperty` spanning
/// `0.1 ..= 100.0` (`properties/scene_properties.py:478-483`), handed straight to
/// `apply_graph_layout` (`ui/operators/scigraphs/layout_operators.py:315`) and from there
/// to `_sphere_layout` & co. (`layouts/dispatcher.py:101-110`). A slider dragged to `12.0`
/// is the ordinary case, so every one of the five has a `*_scaled` entry point, and
/// `sphere_scaled(&bare(9), 12.0)` is the drawing the user asked for.
#[test]
fn a_scale_other_than_five_is_reachable() {
    let topology = bare(9);
    for (id, run, scaled) in scaled_all() {
        let default = run(&topology).expect("runs");
        assert_eq!(
            scaled(&topology, SCALE).expect("the default is a legal scale"),
            default,
            "{id}: passing SCALE through the scaled entry point moved the default"
        );
        let wider = scaled(&topology, 12.0).expect("a scale the rule allows");
        assert_ne!(wider, default, "{id}: the scale never reached the drawing");
        let narrow = space(&default);
        let wide = space(&wider);
        // **Linearity is the reference's own shape**, not a convenience: every coordinate in
        // all five is a product of `scale`, so doubling the scale doubles every coordinate.
        // The tolerance is the `f32` narrowing (one rounding, ~6e-8 relative) with room to
        // spare; anything that made a coordinate non-linear would miss it by orders.
        for axis in 0..3 {
            for i in 0..9usize {
                let narrow_v = [narrow.0[i], narrow.1[i], narrow.2[i]][axis];
                let wide_v = [wide.0[i], wide.1[i], wide.2[i]][axis];
                let want = f64::from(narrow_v) * (12.0 / SCALE);
                assert!(
                    (f64::from(wide_v) - want).abs() <= 1e-5 * want.abs().max(1.0),
                    "{id} node {i} axis {axis}: {wide_v} is not {want}"
                );
            }
        }
    }
}

/// The one rule the five `*_scaled` entry points share, and the same shape
/// `grid/scaled.rs` uses: finite and above 0, so a `NaN` or a negative scale is refused at
/// the entry point rather than handed to the kernel and refused later at `snapshot` under
/// `node.x`.
#[test]
fn a_scale_that_is_not_finite_and_positive_is_refused() {
    let topology = bare(4);
    for (id, _, scaled) in scaled_all() {
        for scale in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                scaled(&topology, scale).expect_err("refused"),
                StageError::Param {
                    name: "scale",
                    rule: "finite and above 0"
                },
                "{id} at scale {scale}"
            );
        }
        assert!(scaled(&topology, f64::MIN_POSITIVE).is_ok(), "{id}");
    }
}

/// The five ids that share one `*_scaled` entry point, as `(id, run, run_scaled)`.
///
/// `scaled_all` is a separate list from [`all`] rather than a fourth tuple field, because
/// `fn(&Topology, f64) -> …` cannot sit in the same tuple as `fn(&Topology) -> …` without
/// every call site spelling the arity out.
fn scaled_all() -> [Scaled; 5] {
    [
        (sphere::ID, sphere, sphere_scaled),
        (helix::ID, helix, helix_scaled),
        (cube::ID, cube, cube_scaled),
        (spiral::ID, spiral, spiral_scaled),
        (bipartite_3d::ID, bipartite_3d, bipartite_3d_scaled),
    ]
}

/// One of the five, paired with the entry point that takes the scale the reference takes.
type Scaled = (
    &'static str,
    fn(&Topology) -> Result<Geometry, crate::stage::StageError>,
    fn(&Topology, f64) -> Result<Geometry, crate::stage::StageError>,
);
