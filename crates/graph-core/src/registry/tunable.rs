//! The parameter structs a layout publishes, and the one macro that says so
//! (`docs/decisions/layout-params.md`).
//!
//! **One invocation per struct generates both halves**: the [`ParamSpec`] table that
//! reaches a caller, and the code that writes a run's values into the struct's fields.
//! Both come from the same `field: kind, min, max, default, step, doc` list, so index `i`
//! of the wire is field `i` of the struct *by construction*. A hand-written table beside
//! a hand-written setter could disagree — swap two rows and every value stays in range,
//! every gate stays green and the drawing is simply wrong — and nothing in the compiler
//! or in any other test would see it.
//!
//! `Option` fields, nested structs and structs whose readers differ per layout are
//! deliberately absent from the list; the decision doc names each exclusion and says what
//! it costs.

use crate::layout::circle_packing::CirclePackingParams;
use crate::layout::force::davidson_harel::DhParams;
use crate::layout::force::drl::DrlParams;
use crate::layout::force::fruchterman_reingold::FrParams;
use crate::layout::force::graphopt::GraphoptParams;
use crate::layout::force::kamada_kawai::KkParams;
use crate::layout::force::lgl::LglParams;
use crate::layout::force::spring::SpringParams;
use crate::layout::forceatlas2::Fa2Params;
use crate::layout::grid::GridParams;
use crate::layout::sugiyama::SugiyamaParams;
use graph_contract::params::ParamSpec;

/// A parameter struct a registered layout can be drawn at other than its `Default`.
///
/// Implemented only through the `tunable!` macro, never by hand.
pub trait Tunable: Copy + Default {
    /// The published parameters, in schema order — which is also the order the macro
    /// invocation lists the fields in.
    const PARAMS: &'static [ParamSpec];

    /// Whether any field is an `f32`, so a published bound must survive
    /// `(bound as f32) as f64 == bound`. A caller that sends the published `max` and gets
    /// back a `f32` above it has been handed an out-of-range value the schema said did
    /// not exist, which is worse than a clamp because nothing says so.
    const NARROWED_TO_F32: bool;

    /// `Self` at `values`, written into the fields in schema order, or `None` when
    /// `values` is not one value per published parameter. `None` rather than a panic: the
    /// length is checked by the buffer's own rule before this runs, and a public path in
    /// this crate does not panic.
    fn tuned(values: &[f64]) -> Option<Self>;
}

/// One published value, as the field that receives it. Every conversion here is exact
/// after [`Tunable::tuned`]'s caller has range-checked the value: `u32` from an integral
/// `f64` in range, `bool` from `0.0`/`1.0`, `f32` by the nearest representable value.
pub trait FromParam {
    /// The field's value for a published `value`.
    fn from_param(value: f64) -> Self;
}

impl FromParam for f64 {
    fn from_param(value: f64) -> Self {
        value
    }
}

impl FromParam for f32 {
    fn from_param(value: f64) -> Self {
        value as f32
    }
}

impl FromParam for u32 {
    fn from_param(value: f64) -> Self {
        value as u32
    }
}

impl FromParam for bool {
    fn from_param(value: f64) -> Self {
        value != 0.0
    }
}

/// Declares one parameter struct's published parameters and its applier at once.
///
/// `tunable!(Ty, narrowed, { field: RustTy, Kind, min, max, default, step, "doc"; .. })`
/// — the field name *is* the published name, the `default` expression *is* what the
/// run takes, and `narrowed` is `true` when any field is an `f32`.
macro_rules! tunable {
    (
        $ty:ty, $narrow:literal,
        { $( $field:ident : $rust:ty, $kind:ident, $min:expr, $max:expr, $default:expr, $step:expr, $doc:literal );* $(;)? }
    ) => {
        impl $crate::registry::tunable::Tunable for $ty {
            const PARAMS: &'static [graph_contract::params::ParamSpec] = &[
                $( graph_contract::params::ParamSpec {
                    name: stringify!($field),
                    kind: graph_contract::params::ParamKind::$kind,
                    min: $min,
                    max: $max,
                    default: $default,
                    step: $step,
                    doc: $doc,
                } ),*
            ];

            const NARROWED_TO_F32: bool = $narrow;

            fn tuned(values: &[f64]) -> Option<Self> {
                if values.len() != <Self as $crate::registry::tunable::Tunable>::PARAMS.len() {
                    return None;
                }
                let mut out = <$ty as ::core::default::Default>::default();
                let mut at = 0usize;
                $(
                    out.$field =
                        <$rust as $crate::registry::tunable::FromParam>::from_param(values[at]);
                    at += 1;
                )*
                let _ = at;
                Some(out)
            }
        }
    };
}

// ---- The published parameters, one invocation per struct. Every bound of an `f32`
// field is a power of two or a whole number: the published `max` has to survive
// `(bound as f32) as f64 == bound`, or a caller that sends the `max` would get back a
// value above the `max` the schema published.

tunable!(FrParams, false, {
    niter: u32, Int, 1.0, 10_000.0, 500.0, 1.0,
        "sweeps of the dense repulsion pass (igraph niter)";
    seed: u32, Int, 0.0, 4_294_967_295.0, 0.0, 1.0,
        "start positions; fixed, never a clock (D5)";
});

tunable!(KkParams, false, {
    epsilon: f64, Float, 0.0, 1.0, 0.0, 1e-9,
        "stop once no node moves further than this; 0 runs the full 50*n budget";
});

tunable!(LglParams, false, {
    maxit: u32, Int, 1.0, 10_000.0, 150.0, 1.0,
        "iterations of the local-graph layout";
    coolexp: f64, Float, 0.01, 4.0, 1.5, 0.05,
        "cooling exponent; at or below 0 the layout substitutes 1.5";
    seed: u32, Int, 0.0, 4_294_967_295.0, 0.0, 1.0,
        "start positions; fixed, never a clock (D5)";
});

tunable!(DrlParams, false, {
    edge_cut: f64, Float, 0.0, 1.0, 0.8, 0.01,
        "share of edges kept long: an edge longer than 40000*(1-edge_cut) is cut before a \
         sweep";
    // Ponytail (edge_cut): inert until an edge is longer than `40000 * (1 - edge_cut)`, so
    // on a small drawing every value of it draws the same picture. It is read (the
    // schedule's own `cut_length`), it simply has nothing to cut at that size. Escape
    // hatch: none needed — at the sizes where it bites, the drawing is 40 000 units wide.
    seed: u32, Int, 0.0, 4_294_967_295.0, 0.0, 1.0,
        "the sweep schedule's stream; fixed, never a clock (D5)";
});

tunable!(DhParams, false, {
    maxiter: u32, Int, 1.0, 1_000.0, 10.0, 1.0,
        "coarse rounds of the annealer";
    fineiter: u32, Int, 0.0, 1_000.0, 0.0, 1.0,
        "fine rounds run at full node distances after the coarse ones";
    cool_fact: f64, Float, 0.0, 1.0, 0.95, 0.01,
        "cooling factor per candidate move; 0 freezes the round immediately";
    seed: u32, Int, 0.0, 4_294_967_295.0, 0.0, 1.0,
        "start positions; fixed, never a clock (D5)";
});

tunable!(GraphoptParams, false, {
    niter: u32, Int, 1.0, 10_000.0, 500.0, 1.0,
        "sweeps of the optimizer (igraph niter)";
    node_charge: f64, Float, 0.0, 1e6, 0.001, 0.001,
        "repulsion per node; 0 turns the pair repulsion off";
    node_mass: f64, Float, 0.0, 1e6, 30.0, 1.0,
        "the per-axis cap on a node's movement";
    spring_length: f64, Float, 0.0, 1e6, 0.0, 1.0,
        "rest length of an edge's spring";
    spring_constant: f64, Float, 0.0, 1e6, 1.0, 0.1,
        "stiffness of an edge's spring";
    max_sa_movement: f64, Float, 0.0, 1e6, 5.0, 0.5,
        "cap on one simulated-annealing step";
    seed: u32, Int, 0.0, 4_294_967_295.0, 0.0, 1.0,
        "start positions; fixed, never a clock (D5)";
});

tunable!(GridParams, true, {
    spacing: f32, Float, 0.0625, 1024.0, 1.0, 0.125,
        "distance between neighbouring cells, on both axes";
});

tunable!(SugiyamaParams, true, {
    layer_spacing: f32, Float, 0.0625, 1024.0, 1.0, 0.125,
        "y distance between adjacent layers of the layered drawing";
});

tunable!(CirclePackingParams, true, {
    iterations: u32, Int, 1.0, 10_000.0, 500.0, 1.0,
        "radius-solver sweeps and tangency-refinement steps; at least 1 is always used";
    scale: f32, Float, 0.25, 4096.0, 5.0, 0.25,
        "the finished packing's rough diameter; must be finite and above 0";
});

tunable!(SpringParams, false, {
    iterations: u32, Int, 1.0, 10_000.0, 50.0, 1.0,
        "maximum gathers of the Fruchterman-Reingold pass (networkx iterations)";
    threshold: f64, Float, 0.0, 1.0, 1e-4, 1e-6,
        "early exit once norm(delta_pos)/n is at or below this; 0 never exits early";
    scale: f64, Float, 1e-6, 1e6, 5.0, 0.5,
        "final extent of the drawing (networkx scale)";
});

tunable!(Fa2Params, false, {
    max_iter: u32, Int, 1.0, 10_000.0, 100.0, 1.0,
        "iteration ceiling (networkx default 100)";
    jitter_tolerance: f64, Float, 0.0, 1e6, 1.0, 0.1,
        "jitter tolerance (networkx default 1.0); 0 never exits early";
    scaling_ratio: f64, Float, 0.0, 1e6, 2.0, 0.1,
        "repulsion scale (networkx default 2.0)";
    gravity: f64, Float, 0.0, 1e6, 1.0, 0.1,
        "gravitational pull to the centroid (networkx default 1.0)";
    seed: u32, Int, 0.0, 4_294_967_295.0, 0.0, 1.0,
        "seeds this port's initial positions; fixed, never a clock (D5)";
});
