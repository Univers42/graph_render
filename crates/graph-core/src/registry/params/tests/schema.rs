//! The schema itself: every advertised default is its struct's `Default`, every wire
//! index reaches its own field, and every bound a value is checked against is one the
//! struct can actually hold. The claims about what a run then *draws* are in
//! [`super::drawing`].

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
use crate::layout::lanes::LanesParams;
use crate::layout::sugiyama::SugiyamaParams;
use crate::registry::{LAYOUTS, Tunable};
use graph_contract::params::{ParamKind, ParamSpec, ParamsView};

/// Every published struct, in no particular order: the two macros below are the only two
/// places that enumerate them, so a struct added to `tunable.rs` without a test fails to
/// compile here rather than passing quietly.
macro_rules! every_published {
    ($each:ident) => {
        $each!(FrParams);
        $each!(KkParams);
        $each!(LglParams);
        $each!(DrlParams);
        $each!(DhParams);
        $each!(GraphoptParams);
        $each!(SpringParams);
        $each!(Fa2Params);
        $each!(GridParams);
        $each!(SugiyamaParams);
        $each!(LanesParams);
        $each!(CirclePackingParams);
    };
}

/// Every advertised default is its struct's `Default`, bit for bit — including the ones
/// that are written as expressions (`ForceParams::velocity_decay`-style computed
/// defaults would be a different number here, which is exactly what this catches).
macro_rules! defaults_are_the_structs_own {
    ($ty:ty) => {{
        let view = ParamsView::new(<$ty as Tunable>::PARAMS);
        assert!(
            <$ty as Tunable>::tuned(&view.defaults()) == Some(<$ty>::default()),
            "{}: the published defaults are not {}::default()",
            stringify!($ty),
            stringify!($ty)
        );
    }};
}

#[test]
fn every_advertised_default_is_its_struct_default() {
    every_published!(defaults_are_the_structs_own);
}

/// Every wire index reaches its own field. This is the test that would catch a table and
/// its setter disagreeing — the failure mode where two parameters are swapped, every
/// value stays in range, every gate stays green and the drawing is simply the wrong one.
/// One macro invocation generates both halves of a layout's parameters from one list, so
/// the swap cannot be written; this says so out loud, per index.
macro_rules! each_index_reaches_its_own_field {
    ($ty:ty) => {{
        let view = ParamsView::new(<$ty as Tunable>::PARAMS);
        let base = <$ty as Tunable>::tuned(&view.defaults()).expect("the defaults apply");
        let moved: Vec<$ty> = (0..view.len())
            .map(|index| {
                let mut values = view.defaults();
                values[index] = far_end(view.specs()[index]);
                <$ty as Tunable>::tuned(&values).expect("in range applies")
            })
            .collect();
        for (index, one) in moved.iter().enumerate() {
            assert_ne!(
                *one,
                base,
                "{}: index {index} changed nothing",
                stringify!($ty)
            );
            for (other, again) in moved.iter().enumerate() {
                assert!(
                    other == index || *one != *again,
                    "{}: indexes {index} and {other} are the same field",
                    stringify!($ty)
                );
            }
        }
    }};
}

#[test]
fn each_published_index_reaches_its_own_field() {
    every_published!(each_index_reaches_its_own_field);
}

/// The bound a test perturbs an index by: whichever end of the range is further from the
/// default, which is never the default itself and is as far from every other index's as
/// the schema allows.
fn far_end(spec: ParamSpec) -> f64 {
    if spec.default - spec.min <= spec.max - spec.default {
        spec.max
    } else {
        spec.min
    }
}

/// Every bound of an `f32`-backed layout has to survive `(x as f32) as f64 == x` — the
/// published `max` is a value a caller will send, and rounding it up would hand back a
/// number above the `max` the schema published, which is worse than a clamp because
/// nothing says it happened.
macro_rules! f32_bounds_are_exact {
    ($ty:ty) => {{
        if <$ty as Tunable>::NARROWED_TO_F32 {
            for spec in <$ty as Tunable>::PARAMS {
                for (what, bound) in [
                    ("min", spec.min),
                    ("max", spec.max),
                    ("default", spec.default),
                ] {
                    assert_eq!(
                        (bound as f32) as f64,
                        bound,
                        "{}: {what} {bound} is not an f32, so the schema would publish a \\
                         bound the layout cannot hold",
                        stringify!($ty)
                    );
                }
            }
        }
    }};
}

#[test]
fn every_f32_backed_bound_survives_the_f32_round_trip() {
    every_published!(f32_bounds_are_exact);
}

/// A published integer travels in an `f64`, so a bound past `2^53` could not be checked
/// against what the struct holds. Every one today is a `u32`.
#[test]
fn every_published_integer_is_exactly_representable() {
    for layout in &LAYOUTS {
        for spec in layout.params.specs {
            if spec.kind != ParamKind::Int {
                continue;
            }
            for (what, bound) in [
                ("min", spec.min),
                ("max", spec.max),
                ("default", spec.default),
            ] {
                assert!(
                    (0.0..=9_007_199_254_740_992.0).contains(&bound) && bound.fract() == 0.0,
                    "{}: {what} {bound} is not a whole f64",
                    layout.id
                );
            }
        }
    }
}
